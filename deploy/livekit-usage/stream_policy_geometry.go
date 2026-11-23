// Copyright PodeTelar contributors. MIT licensed.
// This file extends the separately Apache-2.0-licensed LiveKit server.
package buffer

import (
	"encoding/binary"

	"github.com/livekit/mediatransportutil/pkg/codec"
	"github.com/livekit/protocol/codecs/mime"
)

type streamPolicyGeometry struct {
	mime        mime.MimeType
	long, short uint32
	known       bool
	av1Sequence []byte
}

func (g *streamPolicyGeometry) fits(maxResolution int) bool {
	long := map[int]uint32{720: 1280, 1080: 1920, 1440: 2560, 2160: 3840}[maxResolution]
	return g.known && g.long <= long && g.short <= uint32(maxResolution)
}

func (g *streamPolicyGeometry) update(size codec.VideoSize) {
	g.known = size.Width > 0 && size.Height > 0
	g.long = max(g.long, size.Width, size.Height)
	g.short = max(g.short, min(size.Width, size.Height))
}

func (g *streamPolicyGeometry) observe(m mime.MimeType, payload []byte) {
	if g.mime != m {
		g.mime, g.known, g.av1Sequence = m, false, nil
	}
	switch m {
	case mime.MimeTypeH264:
		switch payload[0] & 31 {
		case 7:
			g.update(codec.ExtractH264VideoSize(payload))
		case 24: // STAP-A
			g.observeNALs(payload[1:], false)
		case 25, 26, 27, 29: // Interleaved packetization does not provide supported size evidence.
			g.known = false
		case 28: // FU-A start carrying an SPS
			if len(payload) > 2 && payload[1]&0x9f == 0x87 {
				nal := append([]byte{payload[0]&0xe0 | 7}, payload[2:]...)
				g.update(codec.ExtractH264VideoSize(nal))
			}
		}
	case mime.MimeTypeH265:
		if len(payload) < 2 {
			return
		}
		switch (payload[0] >> 1) & 63 {
		case 33:
			g.update(codec.ExtractH265VideoSize(payload))
		case 48: // Aggregation packet
			g.observeNALs(payload[2:], true)
		case 49: // Fragmentation unit start carrying an SPS
			if len(payload) > 3 && payload[2]&0xbf == 0xa1 {
				nal := append([]byte{payload[0]&0x81 | (33 << 1), payload[1]}, payload[3:]...)
				g.update(codec.ExtractH265VideoSize(nal))
			}
		}
	case mime.MimeTypeAV1:
		g.observeAV1(payload)
	case mime.MimeTypeVP8:
		var vp8 codec.VP8
		if vp8.Unmarshal(payload) == nil && vp8.IsKeyFrame {
			g.update(codec.ExtractVP8VideoSize(&vp8, payload))
		}
	}
}

func (g *streamPolicyGeometry) observeNALs(elements []byte, hevc bool) {
	for len(elements) >= 2 {
		size := int(binary.BigEndian.Uint16(elements))
		elements = elements[2:]
		if size == 0 || size > len(elements) {
			g.known = false
			return
		}
		nal := elements[:size]
		if hevc && len(nal) >= 2 && (nal[0]>>1)&63 == 33 {
			g.update(codec.ExtractH265VideoSize(nal))
		} else if !hevc && nal[0]&31 == 7 {
			g.update(codec.ExtractH264VideoSize(nal))
		}
		elements = elements[size:]
	}
}

// See https://aomediacodec.github.io/av1-rtp-spec/ for the aggregation header
// and https://aomediacodec.github.io/av1-spec/#sequence-header-obu-syntax for
// the sequence fields. The sequence maximum bounds any encoded frame.
func (g *streamPolicyGeometry) observeAV1(payload []byte) {
	header := payload[0]
	remaining := payload[1:]
	if header&8 != 0 { // N, start of a coded video sequence
		g.known = false
	}
	count := int(header>>4) & 3
	for index := 0; len(remaining) > 0; index++ {
		size := len(remaining)
		if count == 0 || index+1 < count {
			length, read := binary.Uvarint(remaining)
			if read <= 0 || length == 0 || length > uint64(len(remaining)-read) {
				g.known = false
				return
			}
			remaining = remaining[read:]
			size = int(length)
		}
		element := remaining[:size]
		remaining = remaining[size:]
		continuation := index == 0 && header&0x80 != 0
		incomplete := len(remaining) == 0 && header&0x40 != 0
		switch {
		case continuation:
			if g.av1Sequence != nil && len(g.av1Sequence)+len(element) <= 4096 {
				g.av1Sequence = append(g.av1Sequence, element...)
				g.update(av1VideoSize(g.av1Sequence))
			} else {
				g.av1Sequence = nil
			}
		case element[0]>>3&15 == 1: // OBU_SEQUENCE_HEADER
			g.update(av1VideoSize(element))
			if incomplete {
				g.av1Sequence = append(g.av1Sequence[:0], element...)
			}
		}
		if !incomplete {
			g.av1Sequence = nil
		}
	}
}

type policyBits struct {
	data []byte
	bit  int
	ok   bool
}

func (b *policyBits) read(count int) uint32 {
	if b.bit+count > len(b.data)*8 {
		b.ok = false
		return 0
	}
	var value uint32
	for i := 0; i < count; i++ {
		value = value<<1 | uint32(b.data[b.bit/8]>>(7-b.bit%8)&1)
		b.bit++
	}
	return value
}

func av1VideoSize(obu []byte) codec.VideoSize {
	if len(obu) < 2 || obu[0]&0xf9 != 8 {
		return codec.VideoSize{}
	}
	data := obu[1:]
	if obu[0]&4 != 0 {
		data = data[1:]
	}
	if obu[0]&2 != 0 {
		_, read := binary.Uvarint(data)
		if read <= 0 {
			return codec.VideoSize{}
		}
		data = data[read:]
	}
	bits := policyBits{data: data, ok: true}
	bits.read(4) // profile and still_picture
	if bits.read(1) != 0 {
		bits.read(5) // seq_level_idx
	} else {
		bufferDelayBits := 0
		if bits.read(1) != 0 { // timing_info_present_flag
			bits.read(32)
			bits.read(32)
			if bits.read(1) != 0 { // equal_picture_interval
				zeros := 0
				for zeros < 32 && bits.ok && bits.read(1) == 0 {
					zeros++
				}
				if zeros < 32 {
					bits.read(zeros)
				}
			}
			if bits.read(1) != 0 { // decoder_model_info_present_flag
				bufferDelayBits = int(bits.read(5)) + 1
				bits.read(32)
				bits.read(10)
			}
		}
		delayPresent := bits.read(1) != 0
		points := int(bits.read(5)) + 1
		for i := 0; i < points; i++ {
			bits.read(12)
			if bits.read(5) > 7 {
				bits.read(1)
			}
			if bufferDelayBits != 0 && bits.read(1) != 0 {
				bits.read(2*bufferDelayBits + 1)
			}
			if delayPresent && bits.read(1) != 0 {
				bits.read(4)
			}
		}
	}
	widthBits := int(bits.read(4)) + 1
	heightBits := int(bits.read(4)) + 1
	width := bits.read(widthBits) + 1
	height := bits.read(heightBits) + 1
	if !bits.ok {
		return codec.VideoSize{}
	}
	return codec.VideoSize{Width: width, Height: height}
}
