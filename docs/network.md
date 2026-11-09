# Recovery and TLS relay

Automatic permits SFU UDP/TCP and configured TURN. **Relay only** uses the public SDK's relay ICE policy. Changing the browser route stops captures/subscriptions and reconnects; the native publisher chooses before Join. No SDP rewriting supplies credentials.

Browser initial signaling/peer connection deadlines are 10/15 seconds with no hidden initial retries. A reconnect episode starts at most ten retries or 60 seconds of attempts; an in-flight SDK attempt retains its own timeout. The pinned native SDK similarly bounds reconnect starts to ten/60 seconds, with a ten-second signaling timeout and the app's 15-second initial join deadline. Terminal failure requires explicit retry.

Watched streams show aggregate route, RTT, resolution, decoded FPS and bitrate. Native delivery exposes aggregate route/RTT/RTP counters. Candidate addresses, URLs, SDP, credentials and usernames never leave the collectors. Unknown fields stay unknown. Qualified Chrome reported a peer-reflexive candidate with `relayProtocol=tls` and a `turns:` URL; route classification uses that TLS evidence. The key-protected `/api/diagnostics` exposes only aggregate signaling counters/connections.

## Linux qualification overlay

LiveKit v1.13.11 advertises its embedded TLS relay on **443**, irrespective of its configured listener port. A shared host needing another port can use its supported external TURN configuration. FreedomStream pins official Coturn **4.18.0-r0** by digest. This keeps the existing HTTPS listener separate; a network allowing only 443 still needs a suitable dedicated TURN address on 443. A UDP-blocked test does not certify that stricter network.

From the Linux repository, supply actual addresses and a valid certificate hostname/IP. TURN DNS must reach the server directly, without an HTTP-only proxy. Run as the private configuration/certificate owner; the addresses below are documentation examples:

```sh
export TURN_PUBLIC_HOST=relay.example.com
export TURN_PUBLIC_IP=192.0.2.1
export TURN_TLS_PORT=5349
export FREEDOMSTREAM_UID="$(id -u)"
export FREEDOMSTREAM_GID="$(id -g)"
export TURN_CERT_FILE=/absolute/private/path/fullchain.pem
export TURN_KEY_FILE=/absolute/private/path/privkey.pem
node scripts/setup-relay.mjs
docker compose --env-file .env -f compose.yaml -f deploy/relay/compose.yaml up -d
```

The generator validates addresses/ports, preserves an existing 32-byte secret, and writes ignored files under `.local/relay` with mode 0600. The relay reads the secret in its configuration; the SFU reads the same secret from a file. No secret is printed or placed in process arguments. Match mount ownership to the container UID/GID and keep the key/config directory private. This Linux qualification overlay retains loopback web/signaling bindings. Production HTTPS and renewal belong to Phase 8.

Coturn permits TLS client connections with participant-specific HMAC credentials from the SFU, initially valid for five minutes. Plaintext client listeners, DTLS, CLI and TCP relaying are disabled. An explicit SFU-IP exception plus IPv4/IPv6 deny ranges restricts destinations to that SFU. Allocation lifetimes, quotas, ports and container resources are bounded. Open TLS to intended clients and allow UDP between SFU and allocation ports 30000–30127, including Docker's container-to-host path. Clients need no public UDP on this route.

Limits are 1 CPU/512 MiB for the SFU and 0.5 CPU/128 MiB for TURN, with PID/read-only/tmpfs limits. The Coturn binary has a file capability, so its otherwise dropped set retains `NET_BIND_SERVICE` for execution. TURN allocation bandwidth guards do not provide a host-wide shaper or certify five-stream throughput.

## Checks

`npm run check` covers relay configuration/recovery/diagnostic policies. Browser access tests exercise real rotation, selective revocation, saved-token replay and management Lock. `FREEDOMSTREAM_RESTART_APP=1 npm run native:e2e -- native-e2e/access-recovery.spec.ts` deliberately restarts only the local Compose app, verifies native/viewer recovery, then revokes the native session and checks capture termination.

Set `FREEDOMSTREAM_RELAY_HOST`/`FREEDOMSTREAM_RELAY_IP` and run `npm run native:e2e -- native-e2e/relay.spec.ts` for TLS qualification. Its private Chrome process maps that hostname to the supplied IP, keeps certificate verification, disables non-proxied UDP, selects the app's Relay only control, verifies selected TLS media and rejects invalid relay credentials. Optional `FREEDOMSTREAM_RELAY_HOLD_SECONDS` extends continuous decode checks. No images, secrets or raw RTC reports are saved. This test-only mapping does not alter Windows DNS/hosts or qualify native DNS.

Primary references: [pinned SFU ICE/TURN generation](https://github.com/livekit/livekit/blob/v1.13.11/pkg/service/roommanager.go), [external TURN fields](https://github.com/livekit/livekit/blob/v1.13.11/pkg/config/config.go), [Coturn configuration/peer-policy precedence](https://github.com/coturn/coturn/blob/4.18.0/examples/etc/turnserver.conf), [official release](https://github.com/coturn/coturn/releases/tag/docker%2F4.18.0-r0), [RTC candidate statistics](https://www.w3.org/TR/webrtc-stats/#rtcicecandidatestats-dictionary).
