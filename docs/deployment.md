# Production deployment

This deployment owns `/opt/freedomstream`, its Compose project, two nginx snippets,
two timers and explicitly named firewall rules. Existing sites on 80/443, databases
and other Docker projects must be preserved. Commands below run as root unless
noted. Inspect ports, Docker subnets and existing nginx port-80 behavior first.
The selected subnet `172.30.77.0/28` must be free. State is UID/GID 1000; change the
Compose users and provisioned ownership together on a different host.

## Layout and release

Keep immutable source releases at `/opt/freedomstream/releases/<40-character-commit>`;
`current` points to one release. Shared state stays at `/opt/freedomstream/shared`.
Build the image on the server from that exact release:

```sh
docker build -t freedomstream/app:COMMIT /opt/freedomstream/releases/COMMIT
```

Do not deploy from an unreviewed third-party image or ship `.env`, `.local`, room
data, browser profiles or build caches in the source archive. Generate private
state using Node 24 (or its container) as UID 1000:

```sh
node /opt/freedomstream/releases/COMMIT/scripts/prepare-production.mjs /opt/freedomstream PUBLIC_IPV4 ROOM_HOST TURN_HOST
```

This preserves existing random secrets. `ROOM_HOST` may be the public IP;
`TURN_HOST` must be a DNS name resolving directly to the VPS. It prepares `.env`, `secrets.json`, private SFU/relay config,
and nginx snippets, without printing secrets. Copy a committed Windows ZIP and
its `publisher.json` into `shared/publisher`, then make the files read-only to the
app user. Keep all state directories mode 700 and credential files mode 600.

Write `APP_IMAGE=freedomstream/app:COMMIT` to `shared/release.env`, mode 600.
Use the release's `deploy/production/compose.yaml` with **both** env files. Never
print `docker compose config` or Docker environment inspection into shared logs.

## HTTPS without a domain

The current layout uses `https://PUBLIC_IPV4:8443`, native media TCP 7881 / UDP
7882, and TURN/TLS 5349. SFU HTTP 7880 is private inside Docker. App HTTP 18080
binds host loopback only. All public signaling uses the app's `/media` admission
gateway. Do not expose SFU HTTP through nginx or another port.

Let's Encrypt supports IP certificates with a six-day lifetime. Certbot 5.9.0 is
pinned; the documented IP/webroot flow needs 5.4 or newer. See the
[official Certbot IP-certificate announcement](https://letsencrypt.org/2026/03/11/shorter-certs-certbot).
Certificate state/accounts are separate from existing site renewals.

The native WebRTC library verifies TURN names with `X509_check_host`, which does
not match an IP-only subjectAltName. Our actual native test completed TLS then
closed without allocating with an IP-only name; a DNS SAN fixed it with verification
enabled. See the [upstream name checker](https://webrtc.googlesource.com/src/+/main/rtc_base/openssl_utility.cc).
Until a personal domain exists, the current TURN-only name is
`144-217-161-239.sslip.io`. This uses the public
[sslip.io DNS service](https://nip.io/), which resolves embedded public IPs without
an account. It carries DNS queries, not room invitations or media. Its availability
and CA limits are external dependencies; replace it with your own direct DNS name
when available. The certificate includes both the room IP and TURN DNS name and
keeps the six-day profile. No certificate checks are disabled, and existing
business certificates/domains are not reused.

1. Create `/var/www/freedomstream-acme/.well-known/acme-challenge`, owned 1000:1000,
   directories 755. Only ACME challenge files belong here.
2. Install `shared/nginx-acme.conf` as `/etc/nginx/conf.d/freedomstream-acme.conf`;
   run `nginx -t` and reload. The IP vhost serves only challenges and otherwise 404.
3. Allow public IPv4 TCP 80 for ACME **only after verifying every existing port-80
   vhost serves challenges, redirects or refuses requests**. UFW is port based,
   not URL based. Keep existing Cloudflare restrictions on business HTTPS 443.
4. Run the release's `certificates.sh staging`, then `certificates.sh issue`.
   Staging has independent config and is never installed. The IP and TURN DNS name
   both use this owned HTTP-01 vhost. The trusted certificate
   and key are checked for expiry, IP/hostname match and matching public keys,
   then copied into a private versioned directory and switched atomically.
5. Install `shared/nginx-https.conf` as `/etc/nginx/conf.d/freedomstream-https.conf`;
   validate/reload nginx. Allow IPv4 TCP 8443 and 5349. Media ports published by
   Docker are intentionally public; Docker's forwarding can bypass UFW INPUT.
6. Allow relay backend UDP 30000:30127 **from 172.30.77.0/28 only**. Coturn's peer
   ACL permits only the SFU's public IP, its credentials expire after 300 seconds,
   and plain/anonymous TURN is disabled. Do not open an anonymous generic proxy.
7. Install/enable `freedomstream-renew.service/.timer` and
   `freedomstream-backup.service/.timer` from the release. Renewal checks every
   six hours; a changed certificate reloads nginx and restarts only our relay.
   Existing relay allocations reconnect during this restart. Keep UDP/direct TCP
   available where possible. `systemctl --failed` must be monitored.

The proxy overwrites X-Forwarded-For using the actual TCP peer
`$realip_remote_addr`, including on hosts with global Cloudflare real-IP rules.
The app trusts only the dedicated bridge gateway `172.30.77.1`; arbitrary clients
cannot choose rate-limit identities through forwarded headers. Existing shared
site configs are not changed. HTTP access logs and relay logs are disabled to keep
JWT query parameters and relay credentials out of logs. App errors must remain
scrubbed. An authenticated `/api/diagnostics` returns aggregate signaling counts.

Start with:

```sh
docker compose --env-file /opt/freedomstream/shared/.env \
  --env-file /opt/freedomstream/shared/release.env \
  -f /opt/freedomstream/current/deploy/production/compose.yaml up -d --wait
/opt/freedomstream/current/deploy/production/ops.sh status
```

When a domain is available, point its DNS directly at the VPS (media must not be
sent through an ordinary HTTP-only CDN proxy), regenerate the host configuration
with the same secrets, issue its certificate and restart app/relay. Plan invitation
URL changes with the group. A dedicated domain could use 443 if it does not collide
with an existing listener; this release deliberately keeps 8443 separate.

## Backups, restore and rollback

`ops.sh backup` uses SQLite's online backup API from a separate bounded container
with read-only access to live data. It includes committed WAL pages, verifies
integrity/schema/foreign keys, writes mode 600, and never overwrites an existing
backup. The running app cannot access `shared/backups`. Daily backups are local
recovery copies; copy them and the private `.env`/`secrets.json` to encrypted off-host
storage. Local copies do not survive loss of the VPS. Retention/off-host automation
needs the operator's chosen storage destination; no destination is assumed.

```sh
ops.sh backup
ops.sh verify rooms-TIMESTAMP-RANDOM.db
ops.sh restore rooms-TIMESTAMP-RANDOM.db
```

Restore validates the chosen file, backs up current data, stops only our app,
quarantines the old database **and WAL/SHM together**, installs the backup, then
starts the app with its health check. Media connections close during restore;
clients rejoin through normal authority checks. Restoring historical access can
revive old invitations/sessions: rotate/revoke after an incident restore. Rehearse
restores against a dedicated test room before relying on a backup.

For application rollback, stop the app, take a verified backup, point `current`
and `shared/release.env` at the previous tested commit/image and use its Compose
file. Do not run old code against a newer unsupported database schema. Keep a
matching backup for schema rollback. Do not delete volumes or use Docker prune.

## Resource budget and acceptance

Containers cap app at 0.5 CPU/512 MiB, SFU at 1 CPU/512 MiB and relay at 0.5
CPU/128 MiB. That is 2 CPUs and 1.125 GiB of configured memory, plus transient
backup/renewal work and host nginx. These are ceilings, not reserved capacity;
leave room for existing workloads. No server decoding or thumbnail transcoding.

Unlimited monthly transfer still has a 1 Gbps port. Five streams watched by four
others means twenty outgoing copies: 35 Mbps each is roughly 700 Mbps before
protocol/retransmission overhead. The Auto recommendation caps at 35 Mbps for this
reason. Five manual 100 Mbps streams with those subscriptions exceed the port.
Manual presets remain available; congestion feedback reduces effective delivery
and the UI shows actual bitrate. There is no global bandwidth shaper or guarantee
against manual overload of this shared host. Watch only selected streams and
leave headroom for other services. TURN has allocation quotas and an 800 Mbps
aggregate payload limit, but direct SFU traffic is not globally shaped.

Before calling the service qualified, test native NVIDIA/AMD encoders, actual
browser decode, game audio exclusion, direct and forced TLS routes, session
revocation, certificate renewal/install, a restart and real restore, then a full
evening with four/five friends. Record RTT, frame rate, measured bitrate, loss/
retransmissions, CPU/RAM and network utilization. See `docs/phase8-qualification.md`
for the evidence available and the tests still requiring real hardware/friends.
