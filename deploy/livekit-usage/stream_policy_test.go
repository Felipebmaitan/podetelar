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

func TestPodeTelarStreamPolicySeparatedBursts(t *testing.T) {
	now := time.Now()
	guard := &StreamPolicyGuard{}
	overage := &guard.bitrateViolation
	guard.noteViolation(now, overage)
	guard.noteViolation(now.Add(policyViolationWindow+time.Second), overage)
	if guard.closed {
		t.Fatal("separate overflows must not be treated as a sustained violation")
	}
}

func TestPodeTelarStreamPolicyChangesLive(t *testing.T) {
	start := time.Now()
	video := &rtp.Packet{Payload: []byte{1}}
	buffer := &streamPolicyBuffer{geometry: streamPolicyGeometry{long: 2560, short: 1440, known: true}}
	active := &StreamPolicyGuard{}
	previous := streamCaps{MaxFPS: 60, MaxResolution: 1440, MaxBitrate: 10_000_000}
	before := (policySnapshot{}).refresh(previous, start.Add(-time.Second))
	if !active.accept(buffer, video, 1, start, before.forPublication(start, start)) {
		t.Fatal("existing caps should allow matching media")
	}
	tighter := streamCaps{MaxFPS: 60, MaxResolution: 1080, MaxBitrate: 12_000_000}
	updated := start.Add(time.Second)
	after := before.refresh(tighter, updated)
	grace := after.forPublication(start, updated)
	if grace != (streamCaps{MaxFPS: 60, MaxResolution: 1440, MaxBitrate: 12_000_000}) {
		t.Fatalf("grace must keep the previous resolution and the enforceable FPS and bitrate limits: %+v", grace)
	}
	if !active.accept(buffer, video, 2, updated, grace) {
		t.Fatal("existing streams have a bounded reconfiguration window")
	}
	refresh := after.refresh(tighter, updated.Add(2*time.Second))
	if active.accept(buffer, video, 3, updated.Add(policyChangeGrace), refresh.forPublication(start, updated.Add(policyChangeGrace))) {
		t.Fatal("oversized active stream exceeded its reconfiguration window")
	}
	if refresh.forPublication(start, updated.Add(time.Minute)) != tighter {
		t.Fatal("a paused track must not start a new grace window after resuming")
	}
	newTrack := &StreamPolicyGuard{}
	if newTrack.accept(buffer, video, 1, updated, refresh.forPublication(updated, updated)) {
		t.Fatal("a new oversized stream must be blocked immediately")
	}
}
