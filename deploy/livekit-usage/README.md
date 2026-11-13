# Private SFU usage collector

This extends LiveKit v1.13.11, commit
`385aa40f9292fe1db8caad87301e4b285b7c198a`, with a small MIT-licensed collector.
LiveKit and its dependencies retain their own licenses (LiveKit: Apache-2.0).
The final image retains the official runtime and its notices. This is a server
image, not an additional Windows application dependency.

Build from the repository root:

```sh
docker build -f deploy/livekit-usage/Dockerfile -t freedomstream/livekit-usage:COMMIT .
```

Use the immutable resulting image tag as `LIVEKIT_USAGE_IMAGE` in the production
`shared/release.env`, alongside `APP_IMAGE`. Regenerate the private SFU
configuration using the reviewed release's `prepare-production.mjs`; preserve its
existing `shared/secrets.json`, room database and invitations. The generated SFU
configuration enables Prometheus on port 6789. The application derives
`http://livekit:6789/metrics` from its private SFU URL; `USAGE_METRICS_URL` can
override it for a local test. Do not publish 6789 or proxy `/metrics` publicly.
Docker Compose local builds this image automatically. Production requires an
explicit image, rather than silently running stock LiveKit without user metrics.

`stats.patch` invokes the collector after the existing upstream/downstream byte
accounting and before the participant worker receives the same statistics. It
includes primary, padding and retransmission bytes once. The worker supplies the
admitted participant identity. No invitations, JWTs, display names or keys enter
metric labels. Upstream totals become uploads to the server; downstream totals
become downloads by that subscriber, including every actual subscription.

Counters live for one hour after their last increment, allowing a poll to observe
short streams after departure. The collector holds at most 4096 directional
series; inactive oldest entries are evicted at capacity. It also exports a process
generation so restarting the SFU cannot produce negative or huge traffic deltas.
This bound exceeds the expected friends deployment; saturation is a collection
limitation, not billing-grade accounting.

The app samples every five seconds, with one in-flight bounded request, a
three-second deadline, a 4 MiB response ceiling and no redirects. Deltas are
stored in SQLite minute buckets and retained for 400 days. Fine-grained period
boundaries round out to minutes. A delta crossing a minute boundary is allocated
proportionally; SFU telemetry delivery adds its own latency. Graph rates are
averages over measured time, not encoder requests or instantaneous peaks.
Collection gaps over 15 seconds and restarts establish fresh baselines. New
participants admitted during continuous observation can start at zero; existing
participants with no trustworthy baseline start at their first observed counter.
Unobserved data cannot be reconstructed and is excluded, never invented.

Totals cover **recorded SFU media/data traffic**, not every byte on the VPS NIC.
They exclude HTTPS downloads/speed tests, signaling, IP/UDP/TCP/TLS framing and
extra TURN relay legs. The UI states this scope. Do not present them as provider
bandwidth billing or whole-host usage. SFU counters remain keyed by distinct
participant sessions. The administrator report combines sessions carrying the
same PC identifier across rooms, names and app instances; older/unidentified
sessions stay separate. There are no permanent friend accounts. See
[PC usage identity](../../docs/device-usage-identity.md) for persistence, privacy,
compatibility and verification limits.

Do not activate this release until its base is coordinated with the parallel
friends-beta/fullscreen work. In particular, preserve that release's negotiated
100/200 Mb/s network tests and 48 MiB upload proxy limit; this PR does not edit
`network-test.ts` or `prepare-production.mjs`.
