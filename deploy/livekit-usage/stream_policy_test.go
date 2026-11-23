// Copyright PodeTelar contributors. MIT licensed.
package buffer

import (
	"encoding/hex"
	"testing"
	"time"

	"github.com/livekit/protocol/codecs/mime"
	"github.com/pion/rtp"
)

func TestPodeTelarStreamPolicyGeometry(t *testing.T) {
	// Reduced AV1 sequence headers for 1080x1920 and 2560x1440.
	portrait, _ := hex.DecodeString("1808182aa1bf7f")
	large, _ := hex.DecodeString("1808182ea7fecf80")
	var geometry streamPolicyGeometry
	geometry.observe(mime.MimeTypeAV1, portrait)
	if !geometry.fits(1080) || geometry.fits(720) {
		t.Fatalf("unexpected portrait dimensions: %#v", geometry)
	}
	geometry.observe(mime.MimeTypeAV1, large)
	geometry.observe(mime.MimeTypeAV1, portrait)
	if geometry.fits(1080) || !geometry.fits(1440) {
		t.Fatalf("oversized sequence was not retained: %#v", geometry)
	}
	geometry.observe(mime.MimeTypeAV1, []byte{0x18, 0x08})
	if geometry.fits(2160) {
		t.Fatal("malformed new sequence retained old evidence")
	}
}

func TestPodeTelarStreamPolicyChargesPerPublication(t *testing.T) {
	start := time.Now()
	video := &rtp.Packet{Payload: make([]byte, 1000)}
	caps := streamCaps{MaxBitrate: 1_000_000, MaxFPS: 30}
	a, b := &streamPolicyBuffer{}, &streamPolicyBuffer{}
	guard := &StreamPolicyGuard{}
	for frame := uint64(1); frame <= 15; frame++ {
		layer := a
		if frame%2 == 0 {
			layer = b
		}
		if !guard.accept(layer, video, frame, start, caps) ||
			!guard.accept(layer, video, frame, start, caps) {
			t.Fatal("each frame should be counted once across all buffers")
		}
	}
	if guard.accept(b, video, 16, start, caps) {
		t.Fatal("frame burst allowance is per publication")
	}
	if guard.accept(b, video, 16, start.Add(20*time.Millisecond), caps) {
		t.Fatal("remaining packets of an over-budget frame should be dropped")
	}
	caps = streamCaps{MaxBitrate: 1_000_000}
	guard = &StreamPolicyGuard{}
	for packet := range 250 {
		if !guard.accept(a, video, uint64(packet), start, caps) {
			t.Fatal("two seconds of video data should fit in the burst allowance")
		}
	}
	if guard.accept(b, video, 251, start, caps) {
		t.Fatal("bitrate burst allowance is per publication")
	}
	for second := 1; second <= 7; second++ {
		for packet := range 126 {
			guard.accept(a, video, uint64(packet), start.Add(time.Duration(second)*time.Second), caps)
		}
	}
	if !guard.closed {
		t.Fatal("a sustained violation must stop the publication")
	}
}

func TestPodeTelarStreamPolicyChangesLive(t *testing.T) {
	start := time.Now()
	video := &rtp.Packet{Payload: []byte{1}}
	buffer := &streamPolicyBuffer{geometry: streamPolicyGeometry{long: 2560, short: 1440, known: true}}
	active := &StreamPolicyGuard{}
	if !active.accept(buffer, video, 1, start, streamCaps{}) {
		t.Fatal("the default policy allows existing clients")
	}
	tighter := streamCaps{MaxResolution: 1080}
	if !active.accept(buffer, video, 2, start.Add(time.Second), tighter) {
		t.Fatal("existing streams have a bounded reconfiguration window")
	}
	if active.accept(buffer, video, 3, start.Add(5*time.Second), tighter) {
		t.Fatal("oversized active stream exceeded its reconfiguration window")
	}
	newTrack := &StreamPolicyGuard{}
	if newTrack.accept(buffer, video, 1, start, tighter) {
		t.Fatal("a new oversized stream must be blocked immediately")
	}
}
