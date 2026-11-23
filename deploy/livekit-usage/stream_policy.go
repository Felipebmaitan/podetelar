// Copyright PodeTelar contributors. MIT licensed.
// This file extends the separately Apache-2.0-licensed LiveKit server.
package buffer

import (
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"os"
	"sync"
	"sync/atomic"
	"time"

	"github.com/pion/rtp"
)

const (
	policyRefresh         = 2 * time.Second
	policyTTL             = 10 * time.Second
	policyChangeGrace     = 4 * time.Second
	policyViolationWindow = 5 * time.Second
	policyBitrateBurst    = 2.0 // seconds of video RTP payload
	policyFrameBurst      = 0.5 // seconds of frames
)

type streamCaps struct {
	MaxBitrate    int `json:"maxBitrateBps"`
	MaxResolution int `json:"maxResolution"`
	MaxFPS        int `json:"maxFps"`
}

func (c streamCaps) valid() bool {
	return (c.MaxBitrate == 0 || c.MaxBitrate >= 1_000_000 && c.MaxBitrate <= 100_000_000) &&
		(c.MaxResolution == 0 || c.MaxResolution == 720 || c.MaxResolution == 1080 || c.MaxResolution == 1440 || c.MaxResolution == 2160) &&
		(c.MaxFPS == 0 || c.MaxFPS == 15 || c.MaxFPS == 24 || c.MaxFPS == 30 || c.MaxFPS == 60 || c.MaxFPS == 90 || c.MaxFPS == 120)
}

type policySnapshot struct {
	caps    streamCaps
	expires time.Time
}

var (
	policyURL         = os.Getenv("PODETELAR_STREAM_POLICY_URL")
	currentStreamCaps atomic.Pointer[policySnapshot]
	errStreamPolicy   = errors.New("publisher exceeded stream policy")
)

func init() {
	if policyURL == "" {
		return
	}
	token := os.Getenv("PODETELAR_STREAM_POLICY_TOKEN")
	if token == "" {
		// An enabled guard with no credentials keeps all video blocked.
		return
	}
	client := &http.Client{
		Timeout:       policyRefresh,
		CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse },
	}
	go func() {
		for {
			if caps, err := readStreamCaps(client, policyURL, token); err == nil {
				currentStreamCaps.Store(&policySnapshot{caps: caps, expires: time.Now().Add(policyTTL)})
			}
			time.Sleep(policyRefresh)
		}
	}()
}

func readStreamCaps(client *http.Client, endpoint, token string) (streamCaps, error) {
	var caps streamCaps
	request, err := http.NewRequest(http.MethodGet, endpoint, nil)
	if err != nil {
		return caps, err
	}
	request.Header.Set("Authorization", "Bearer "+token)
	response, err := client.Do(request)
	if err != nil {
		return caps, err
	}
	defer response.Body.Close()
	var payload struct {
		*streamCaps
		Revision *uint64 `json:"revision"`
	}
	payload.streamCaps = &caps
	decoder := json.NewDecoder(io.LimitReader(response.Body, 1024))
	if response.StatusCode != http.StatusOK || decoder.Decode(&payload) != nil || payload.Revision == nil || !caps.valid() {
		return caps, errStreamPolicy
	}
	return caps, nil
}

// StreamPolicyGuard owns one publication's bitrate and frame budgets across its RTP layers.
// The caller supplies the publication's existing close path for sustained violations.
type StreamPolicyGuard struct {
	sync.Mutex
	close                            func()
	closed                           bool
	started                          bool
	caps                             streamCaps
	graceUntil                       time.Time
	updated                          time.Time
	bits, frames                     float64
	bitrateViolation, frameViolation time.Time
}

func NewStreamPolicyGuard(closeTrack func()) *StreamPolicyGuard {
	if policyURL == "" {
		return nil
	}
	return &StreamPolicyGuard{close: closeTrack}
}

// SetStreamPolicy is called once on the publisher buffer before it is bound.
func (b *BufferBase) SetStreamPolicy(guard *StreamPolicyGuard) {
	b.Lock()
	defer b.Unlock()
	b.streamPolicy = guard
}

type streamPolicyBuffer struct {
	geometry        streamPolicyGeometry
	unverifiedSince time.Time
	frameDecisions  [256]streamFrameDecision
}

type streamFrameDecision struct {
	timestamp uint64
	expires   time.Time
	allowed   bool
	marked    bool
}

func (p *streamPolicyBuffer) frame(timestamp uint64) *streamFrameDecision {
	return &p.frameDecisions[timestampHash(timestamp)]
}

func timestampHash(value uint64) uint8 {
	// RTP timestamps are commonly multiples of 1500, 3000 or 6000. Mix
	// them before choosing a slot so consecutive frames seldom collide.
	value ^= value >> 16
	value *= 0x9e3779b97f4a7c15
	return uint8(value >> 56)
}

func (g *StreamPolicyGuard) accept(p *streamPolicyBuffer, packet *rtp.Packet, timestamp uint64, now time.Time, caps streamCaps) bool {
	g.Lock()
	defer g.Unlock()
	if g.closed {
		return false
	}
	if !g.started || g.caps != caps {
		if g.started {
			g.graceUntil = now.Add(policyChangeGrace)
		}
		g.started = true
		g.caps = caps
		g.updated = now
		g.bits = float64(caps.MaxBitrate) * policyBitrateBurst
		g.frames = float64(caps.MaxFPS) * policyFrameBurst
		g.bitrateViolation, g.frameViolation = time.Time{}, time.Time{}
	}
	if now.Before(g.graceUntil) {
		p.unverifiedSince = time.Time{}
		return true
	}
	if caps.MaxResolution > 0 && !p.geometry.fits(caps.MaxResolution) {
		g.noteViolation(now, &p.unverifiedSince)
		return false
	}
	p.unverifiedSince = time.Time{}

	seconds := max(0.0, now.Sub(g.updated).Seconds())
	g.updated = now
	bitrate := float64(caps.MaxBitrate)
	g.bits = min(bitrate*policyBitrateBurst, g.bits+seconds*bitrate)
	if caps.MaxFPS > 0 {
		rate := float64(caps.MaxFPS)
		g.frames = min(rate*policyFrameBurst, g.frames+seconds*rate)
		decision := p.frame(timestamp)
		if decision.timestamp != timestamp || !now.Before(decision.expires) || (packet.Marker && decision.marked) {
			*decision = streamFrameDecision{timestamp: timestamp, expires: now.Add(time.Second), allowed: g.frames >= 1}
			if decision.allowed {
				g.frames--
			} else {
				g.noteViolation(now, &g.frameViolation)
			}
		}
		decision.marked = decision.marked || packet.Marker
		if !decision.allowed {
			return false
		}
		if g.frames >= rate*policyFrameBurst/2 {
			g.frameViolation = time.Time{}
		}
	}
	if caps.MaxBitrate > 0 {
		charge := float64(len(packet.Payload) * 8)
		if g.bits < charge {
			g.noteViolation(now, &g.bitrateViolation)
			return false
		}
		g.bits -= charge
		if g.bits >= bitrate*policyBitrateBurst/2 {
			g.bitrateViolation = time.Time{}
		}
	}
	return true
}

func (g *StreamPolicyGuard) noteViolation(now time.Time, first *time.Time) {
	if first.IsZero() {
		*first = now
	} else if now.Sub(*first) >= policyViolationWindow {
		g.closed = true
		if g.close != nil {
			go g.close()
		}
	}
}

func (b *BufferBase) acceptsStreamPolicy(packet *rtp.Packet, timestamp uint64) bool {
	if b.streamPolicy == nil || len(packet.Payload) == 0 {
		return true
	}
	now := time.Now()
	policy := currentStreamCaps.Load()
	if policy == nil || !now.Before(policy.expires) {
		return false
	}
	if b.policyBuffer == nil {
		b.policyBuffer = &streamPolicyBuffer{}
	}
	b.policyBuffer.geometry.observe(b.mime, packet.Payload)
	return b.streamPolicy.accept(b.policyBuffer, packet, timestamp, now, policy.caps)
}
