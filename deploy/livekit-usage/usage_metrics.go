// Copyright FreedomStream contributors. MIT licensed.
// This file extends the separately Apache-2.0-licensed LiveKit server.
package telemetry

import (
	"sync"
	"time"

	prom "github.com/prometheus/client_golang/prometheus"
)

type usageKey struct{ room, participant, direction string }
type usageValue struct {
	bytes   uint64
	updated time.Time
}
type usageCollector struct {
	sync.Mutex
	values                              map[usageKey]usageValue
	started                             float64
	bytesDesc, enabledDesc, startedDesc *prom.Desc
}

var participantUsage = &usageCollector{
	values:      make(map[usageKey]usageValue),
	started:     float64(time.Now().UnixNano()) / 1e9,
	bytesDesc:   prom.NewDesc("freedomstream_participant_bytes_total", "SFU media/data bytes by admitted participant, including retransmissions", []string{"room", "participant", "direction"}, nil),
	enabledDesc: prom.NewDesc("freedomstream_usage_enabled", "FreedomStream collector version", nil, nil),
	startedDesc: prom.NewDesc("freedomstream_usage_started_seconds", "Collector process start time", nil, nil),
}

func init() { prom.MustRegister(participantUsage) }
func (c *usageCollector) Describe(ch chan<- *prom.Desc) {
	ch <- c.bytesDesc
	ch <- c.enabledDesc
	ch <- c.startedDesc
}
func (c *usageCollector) Collect(ch chan<- prom.Metric) {
	c.Lock()
	defer c.Unlock()
	ch <- prom.MustNewConstMetric(c.enabledDesc, prom.GaugeValue, 1)
	ch <- prom.MustNewConstMetric(c.startedDesc, prom.GaugeValue, c.started)
	for key, value := range c.values {
		if time.Since(value.updated) > time.Hour {
			delete(c.values, key)
			continue
		}
		ch <- prom.MustNewConstMetric(c.bytesDesc, prom.CounterValue, float64(value.bytes), key.room, key.participant, key.direction)
	}
}
func recordParticipantUsage(room, participant, direction string, bytes uint64) {
	if bytes == 0 {
		return
	}
	c := participantUsage
	c.Lock()
	defer c.Unlock()
	key := usageKey{room, participant, direction}
	if _, exists := c.values[key]; !exists && len(c.values) >= 4096 {
		var oldest usageKey
		var at time.Time
		for k, v := range c.values {
			if at.IsZero() || v.updated.Before(at) {
				oldest = k
				at = v.updated
			}
		}
		delete(c.values, oldest)
	}
	value := c.values[key]
	value.bytes += bytes
	value.updated = time.Now()
	c.values[key] = value
}
