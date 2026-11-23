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
	policyViolationGap    = 1500 * time.Millisecond
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

type policyLimit struct {
	previous, current int
	changedAt         time.Time
}

func (l policyLimit) update(value int, now time.Time) policyLimit {
	if l.current != value {
		return policyLimit{previous: l.current, current: value, changedAt: now}
	}
	return l
}

func (l policyLimit) forPublication(started, now time.Time) int {
	tightened := l.current > 0 && (l.previous == 0 || l.current < l.previous)
	if tightened && started.Before(l.changedAt) && now.Before(l.changedAt.Add(policyChangeGrace)) {
		return l.previous
	}
	return l.current
}

type policySnapshot struct {
	bitrate, resolution, fps policyLimit
	expires                  time.Time
}

func (p *policySnapshot) refresh(caps streamCaps, now time.Time) *policySnapshot {
	if p == nil {
		return &policySnapshot{
			bitrate:    policyLimit{current: caps.MaxBitrate},
			resolution: policyLimit{current: caps.MaxResolution},
			fps:        policyLimit{current: caps.MaxFPS},
			expires:    now.Add(policyTTL),
		}
	}
	return &policySnapshot{
		bitrate:    p.bitrate.update(caps.MaxBitrate, now),
		resolution: p.resolution.update(caps.MaxResolution, now),
		fps:        p.fps.update(caps.MaxFPS, now),
		expires:    now.Add(policyTTL),
	}
}

func (p *policySnapshot) forPublication(started, now time.Time) streamCaps {
	return streamCaps{
		MaxBitrate:    p.bitrate.forPublication(started, now),
		MaxResolution: p.resolution.forPublication(started, now),
		MaxFPS:        p.fps.forPublication(started, now),
	}
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
				currentStreamCaps.Store(currentStreamCaps.Load().refresh(caps, time.Now()))
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
	created                          time.Time
	updated                          time.Time
	bits, frames                     float64
	bitrateViolation, frameViolation policyOverage
}

func NewStreamPolicyGuard(closeTrack func()) *StreamPolicyGuard {
	if policyURL == "" {
		return nil
	}
	return &StreamPolicyGuard{close: closeTrack, created: time.Now()}
}

// SetStreamPolicy is called once on the publisher buffer before it is bound.
func (b *BufferBase) SetStreamPolicy(guard *StreamPolicyGuard) {
	b.Lock()
	defer b.Unlock()
	b.streamPolicy = guard
}

type streamPolicyBuffer struct {
	geometry       streamPolicyGeometry
	unverified     policyOverage
	frameDecisions [256]streamFrameDecision
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

func (g *StreamPolicyGuard) acceptCurrent(p *streamPolicyBuffer, packet *rtp.Packet, timestamp uint64) bool {
	g.Lock()
	defer g.Unlock()
	now := time.Now()
	policy := currentStreamCaps.Load()
	if policy == nil || !now.Before(policy.expires) {
		return false
	}
	return g.accept(p, packet, timestamp, now, policy.forPublication(g.created, now))
}

func (g *StreamPolicyGuard) accept(p *streamPolicyBuffer, packet *rtp.Packet, timestamp uint64, now time.Time, caps streamCaps) bool {
	if g.closed {
		return false
	}
	if !g.started {
		g.updated = now
	}
	if !g.started || g.caps.MaxBitrate == 0 {
		g.bits = float64(caps.MaxBitrate) * policyBitrateBurst
	}
	if !g.started || g.caps.MaxFPS == 0 {
		g.frames = float64(caps.MaxFPS) * policyFrameBurst
	}
	if g.caps.MaxBitrate != caps.MaxBitrate {
		g.bitrateViolation = policyOverage{}
	}
	if g.caps.MaxFPS != caps.MaxFPS {
		g.frameViolation = policyOverage{}
	}
	g.started, g.caps = true, caps
	if caps.MaxResolution > 0 && !p.geometry.fits(caps.MaxResolution) {
		g.noteViolation(now, &p.unverified)
		return false
	}
	p.unverified = policyOverage{}

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
	}
	if caps.MaxBitrate > 0 {
		charge := float64(len(packet.Payload) * 8)
		if g.bits < charge {
			g.noteViolation(now, &g.bitrateViolation)
			return false
		}
		g.bits -= charge
	}
	return true
}

type policyOverage struct {
	first, last time.Time
}

func (g *StreamPolicyGuard) noteViolation(now time.Time, overage *policyOverage) {
	if overage.last.IsZero() || now.Sub(overage.last) > policyViolationGap {
		overage.first = now
	}
	overage.last = now
	if now.Sub(overage.first) >= policyViolationWindow {
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
	if b.policyBuffer == nil {
		b.policyBuffer = &streamPolicyBuffer{}
	}
	b.policyBuffer.geometry.observe(b.mime, packet.Payload)
	return b.streamPolicy.acceptCurrent(b.policyBuffer, packet, timestamp)
}
