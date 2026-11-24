# Production deployment

## Upgrading an existing deployment

This release renames service paths, the Compose project, database filename and SFU
metrics. Migrate an existing deployment as one coordinated change before enabling
automatic deployment from this source. Clients and server must be upgraded together;
renamed cookies require fresh admission. Do not start a second stack with empty state.

Take and verify an online database backup, then stop the existing app, SFU, relay
and its backup/renewal timers using the previous release's configuration. Preserve
the entire shared directory, credentials, certificates and database. Save the old
download executable and manifest separately for rollback. Stage a newly built
`podetelar-windows-x64-*.exe` with its matching `publisher.json` in `shared/publisher`;
renaming an old executable is insufficient because it cannot handle the new links.
Move the deployment to `/opt/podetelar` and rename the stopped database
to `shared/data/podetelar.db`, keeping any WAL/SHM companions with it. Update
`DATABASE_PATH` in private environment files and retain the backup for rollback.
Retarget `current` to the new release under `/opt/podetelar/releases/`; existing
release links use absolute paths and do not follow a moved deployment root.

Regenerate configuration with the existing secrets and deploy both the new app
and usage-enabled SFU together. Update the restricted SSH command, nginx snippet
paths and systemd units to their new names; disable and remove the previous units
and snippets so only one set is active. Validate nginx, verify stored rooms and
usage history, and exercise backup, renewal and restore before enabling CD.
Local Compose installations must likewise migrate their existing data volume
explicitly; changing a project name creates a separate default volume.

## Automatic server updates

`Deploy VPS` deploys successful `Checks` push runs on `main`. It checks that the
commit is still the latest main before starting; old commits are skipped. Manual
dispatch also requires successful main checks. Deployments serialize and never
cancel an update in progress. Only a small Ubuntu SSH job runs on Actions; the
VPS builds the app with one CPU/768 MiB and the usage SFU with one CPU/1536 MiB.
The SFU image is reused by source hash until its collector/build files change.
Before activation, the new app image validates the staged publisher filename,
manifest, size and hash. Invalid or outdated download metadata aborts deployment
before running services or state change. Windows packaging remains a separate
operation; CD preserves the already qualified download during ordinary updates.

Before transferring source or building, the workflow reads the active release's
commit through the restricted SSH `current` command. It compares all changes from
that deployed version to checked main. Only API, served website/dashboard, shared
contracts, server build inputs, production/SFU/relay files and the CD workflow
trigger deployment. Native publisher/portable, assets, Cargo and documentation-only
changes skip source transfer, builds and container restart. Comparing to the active
release also catches an earlier pending server update followed by a native-only
merge. A short GitHub preflight job still evaluates this condition after Checks.

The `production` environment holds `VPS_DEPLOY_KEY` and `VPS_KNOWN_HOSTS`; its
variables are `VPS_HOST` and `VPS_USER`. Allow deployments only from main. The key
is dedicated to this repository and has a forced SSH command with forwarding/PTY
disabled. Install `cd-receive.py` at `/opt/podetelar/cd/receive.py` and authorize
its public key as `restrict,command="/usr/bin/python3 /opt/podetelar/cd/receive.py"`.
Use the previously verified server host keys, never an unverified keyscan in CI.

The receiver accepts only `current` (the active SHA) or `deploy <40-character-SHA>` plus a bounded source
archive. It rejects links/traversal/private state and stages immutable releases.
The release script builds before activation, creates an integrity-checked SQLite
online backup (including committed WAL pages), and preserves the original database
inode, credentials and publisher manifest. It checks every saved traffic bucket
and coverage row within the existing 400-day retention for deletion/reduction,
and durable rooms/participants with retained traffic for loss. Expired users
without retained traffic may undergo normal application cleanup.
Traffic writes may increase totals across the update. First activation adds the
private metrics listener and enables the usage SFU without regenerating secrets.
If no traffic tables existed previously, they initialize without fabricating past
traffic; the former stock SFU cannot reconstruct historical participant usage.

Failures restore the previous code/config and restart its services. Rollback never
restores a snapshot over the live database, so new traffic is retained. The verified
backup and prior code/config remain under `shared/backups/cd-*` for operator recovery.
No `compose down`, volume removal, database replacement or pruning runs during CD.
The app's documented 400-day traffic retention remains a separate application policy.

Run the backup/history regression tests with
`python3 -m unittest discover -s deploy/production -p 'test_cd_*.py'`.

This deployment owns `/opt/podetelar`, its Compose project, two nginx snippets,
two timers and explicitly named firewall rules. Existing sites on 80/443, databases
and other Docker projects must be preserved. Commands below run as root unless
noted. Inspect ports, Docker subnets and existing nginx port-80 behavior first.
The selected subnet `172.30.77.0/28` must be free. State is UID/GID 1000; change the
Compose users and provisioned ownership together on a different host.

## Layout and release

Keep immutable source releases at `/opt/podetelar/releases/<40-character-commit>`;
`current` points to one release. Shared state stays at `/opt/podetelar/shared`.
Build the image on the server from that exact release:

```sh
docker build -t podetelar/app:COMMIT /opt/podetelar/releases/COMMIT
```

Do not deploy from an unreviewed third-party image or ship `.env`, `.local`, room
data, browser profiles or build caches in the source archive. Generate private
state using Node 24 (or its container) as UID 1000:

```sh
node /opt/podetelar/releases/COMMIT/scripts/prepare-production.mjs /opt/podetelar PUBLIC_IPV4 ROOM_HOST TURN_HOST
```

This preserves existing random secrets. `ROOM_HOST` may be the public IP;
`TURN_HOST` must be a DNS name resolving directly to the VPS. It prepares `.env`, `secrets.json`, private SFU/relay config,
and nginx snippets, without printing secrets. Copy a committed portable Windows EXE and
its `publisher.json` into `shared/publisher`, then make the files read-only to the
app user. Keep all state directories mode 700 and credential files mode 600.

Write `APP_IMAGE=podetelar/app:COMMIT` to `shared/release.env`, mode 600.
The administrator dashboard also requires `LIVEKIT_USAGE_IMAGE` set to the
immutable usage-enabled SFU image. Build and configure it using
[the private collector runbook](../deploy/livekit-usage/README.md). Preserve existing secrets, invitations, network-test negotiation and proxy limits
when activating a new release.
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
Use a DNS-only TURN hostname that resolves directly to the VPS and include it
in the certificate alongside the room IP. A DNS SAN is required even when the
website uses an IP origin. Keep certificate verification enabled.

1. Create `/var/www/podetelar-acme/.well-known/acme-challenge`, owned 1000:1000,
   directories 755. Only ACME challenge files belong here.
2. Install `shared/nginx-acme.conf` as `/etc/nginx/conf.d/podetelar-acme.conf`;
   run `nginx -t` and reload. The IP vhost serves only challenges and otherwise 404.
3. Allow public IPv4 TCP 80 for ACME **only after verifying every existing port-80
   vhost serves challenges, redirects or refuses requests**. UFW is port based,
   not URL based. Keep existing Cloudflare restrictions on business HTTPS 443.
4. Run the release's `certificates.sh staging`, then `certificates.sh issue`.
   Staging has independent config and is never installed. The IP and TURN DNS name
   both use this owned HTTP-01 vhost. The trusted certificate
   and key are checked for expiry, IP/hostname match and matching public keys,
   then copied into a private versioned directory and switched atomically.
5. Install `shared/nginx-https.conf` as `/etc/nginx/conf.d/podetelar-https.conf`;
   validate/reload nginx. Allow IPv4 TCP 8443 and 5349. Media ports published by
   Docker are intentionally public; Docker's forwarding can bypass UFW INPUT.
6. Allow relay backend UDP 30000:30127 **from 172.30.77.0/28 only**. Coturn's peer
   ACL permits only the SFU's public IP, its credentials expire after 300 seconds,
   and plain/anonymous TURN is disabled. Do not open an anonymous generic proxy.
7. Install/enable `podetelar-renew.service/.timer` and
   `podetelar-backup.service/.timer` from the release. Renewal checks every
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
docker compose --env-file /opt/podetelar/shared/.env \
  --env-file /opt/podetelar/shared/release.env \
  -f /opt/podetelar/current/deploy/production/compose.yaml up -d --wait
/opt/podetelar/current/deploy/production/certificates.sh renew
/opt/podetelar/current/deploy/production/ops.sh status
```

The startup renewal command records that nginx and the running relay have applied
the certificate. A failed validation/reload leaves
this application marker pending, so subsequent timer runs retry it even when the
certificate bytes have not changed. The `check` action is read-only and fails for
an unapplied certificate.

When a domain is available, use the split-domain configuration below or retain a
single DNS-only hostname. Plan invitation URL changes with the group. Regenerate
with the existing secrets, issue certificates, then restart only this stack.

## Cloudflare website and direct service domains

The optional `DIRECT_URL` defaults to `PUBLIC_URL`, preserving existing installs.
For example, use these roles with your own domain:

- `stream.example.com`: Cloudflare **Proxied**, public HTTPS website on 443.
- `stream-direct.example.com`: Cloudflare **DNS only**, direct HTTPS on 8443.
- `turn.example.com`: Cloudflare **DNS only**, existing TURN/TLS on 5349.

Point all three A records to the VPS IPv4. Add AAAA only after IPv6 routing,
listeners and firewall policy are separately verified. DNS-only hostnames expose
the VPS address; media protection remains the hosting provider's responsibility.
Keep media UDP 7882 and TCP 7881 direct. The SFU must advertise the public VPS IP,
never Cloudflare's proxy IPs. Changing DNS does not turn Cloudflare into a relay.

Generate the split layout, preserving the existing `shared/secrets.json`:

```sh
node scripts/prepare-production.mjs /opt/podetelar PUBLIC_IPV4 stream.example.com turn.example.com stream-direct.example.com
```

This writes `PUBLIC_URL=https://stream.example.com` and
`DIRECT_URL=https://stream-direct.example.com:8443`. The generated nginx snippet adds
only this website's 443 vhost and a direct 8443 vhost. Existing business vhosts,
their Cloudflare real-IP rules and their 443 firewall restrictions stay intact.
Keep 8443 reachable directly. A future direct 443 listener needs a separately
reviewed shared-host firewall/SNI setup or a dedicated address; changing the port
in DNS alone cannot achieve that. Port-443-only networks remain an open gate with
the existing TURN 5349 layout.

Install both generated nginx snippets, run `nginx -t`, and follow the existing
staging/issue/renew procedure. The certificate now covers the website, direct and
TURN names, and renewal checks every name before installing it. Use a public CA
certificate, not a Cloudflare Origin CA certificate, for direct/native endpoints.
For first issuance keep the website DNS-only until the origin certificate is
installed; then enable proxying with **Full (strict)** TLS. Preserve HTTP-01 access
on port 80 for all names. ACME requests must bypass challenges, authentication and
cache/redirect rules if issued through Cloudflare.

Invitation creation, including Discord replies and administrator actions, always
uses `PUBLIC_URL`; invitation secrets stay in the fragment. The small app handoff
also redeems on the public website, retaining canonical public invitations for
native Invite actions. Current native clients discover
`DIRECT_URL` through `/api/connection` before joining any public invitation, so their
host-only session cookies, preview uploads, diagnostics and bandwidth probes all
use the direct origin. Discovery is a credential-free request to the selected
server; its response can delegate native admission to an operator-chosen HTTPS
origin. Existing installations without discovery retain their single-origin flow.
Old clients must be updated before the split: their public bandwidth tests return
421 rather than silently measuring the CDN. Signaling still passes `/media` room
admission and revocation; raw SFU HTTP remains private.

Downloads retain room authority. Clicking an authenticated website download
issues a single-use 60-second ticket and redirects to the direct `/download`
page with that ticket in a fragment. The page removes the fragment and submits
the ticket in a POST body to receive the attachment. No website cookie or permanent
invitation is copied across domains, and session expiry/revocation is rechecked
before serving bytes. At most 128 pending tickets are kept in memory; expired
entries are swept every 15 seconds and restart discards all tickets. Cloudflare
can see website TLS traffic, including response bodies and redirect headers;
fragments prevent secrets appearing in request URLs, not disclosure to a service
that terminates the website's TLS.

Use Cloudflare cache bypass rules for `/api/*`, `/media/*`, `/r/*`, `/manage/*`
and `/download`, respecting the application's `Cache-Control: no-store`. Do not
enable Cache Everything for authenticated paths. Do not capture bodies, cookies,
authorization, signaling query strings or handoff redirect headers in edge
logging/Workers. Keep native and signed Discord endpoints free of interactive
browser challenges. No broad cross-origin credential sharing or wildcard CORS is
needed. Bulk probes and download redemption return 421 on the public host, and
the generated website vhost also rejects them before reading upload bodies.

The nginx proxy overwrites forwarded identities with `$realip_remote_addr`, the
actual TCP peer. Behind Cloudflare this conservatively groups website rate limits
by edge IP; do not trust arbitrary `CF-Connecting-IP` headers to change that. Any
future client-IP forwarding needs a verified Cloudflare-only ingress boundary.
Direct native clients retain their own source IP for limits.

Cloudflare's [CDN terms](https://www.cloudflare.com/service-specific-terms-application-services/#content-delivery-network-free-pro-or-business)
restrict video and large-file delivery through the ordinary CDN. This split sends
bulk media, probes and binaries outside that CDN; it does not assert a legal
approval or a supported traffic-percentage threshold. See also
[Cloudflare proxy status](https://developers.cloudflare.com/dns/proxy-status/)
and [LiveKit ports](https://docs.livekit.io/transport/self-hosting/ports-firewall/).

Before rollout, test actual DNS/proxy status, trusted certificates and renewal,
Discord invitations, native Load/Join/reconnect/revocation, direct bandwidth tests
and downloads, and forced TURN while UDP is unavailable. Confirm selected media
candidates reach the VPS and Cloudflare carries no media/bulk probe traffic.
Local tests establish routing and authority, not production TLS, CDN behavior or
restrictive-network qualification. Apply DNS and proxy configuration explicitly as part of rollout.

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
file. While the app is stopped, run
`python3 deploy/production/cd-state.py revoke-discord-voice /opt/podetelar/shared/data/podetelar.db`
from the outgoing release directory if the previous version lacks Discord voice
following. The command revokes Discord voice sessions and queues
their SFU removal while preserving independently joined guests. Keep a
matching backup for schema rollback. Do not delete volumes or use Docker prune.

Use a pre-camera-policy database backup when rolling back the app to a release
with the original stream policy schema after saving camera policy fields. That
release expects the original JSON shape in the stream policy table.

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
leave headroom for other services. TURN has allocation quotas (8 per participant,
64 total) and an 800 Mbps per-allocation limit. Neither TURN nor direct SFU traffic
has a measured aggregate bandwidth shaper. Coturn's `bps-capacity` reserves each
allocation's full maximum at allocation time; configuring it equal to one
allocation's maximum rejected additional idle ICE candidates with error 486.
It is deliberately disabled rather than misrepresented as a shared traffic
policer. See [upstream reservation accounting](https://github.com/coturn/coturn/blob/4.18.0/src/apps/relay/netengine.c).

Before calling the service qualified, test native NVIDIA/AMD encoders, actual native decode, game audio exclusion, direct and forced TLS routes, session
revocation, certificate renewal/install, a restart and real restore, then a full
evening with four/five friends. Record RTT, frame rate, measured bitrate, loss/
retransmissions, CPU/RAM and network utilization. Local development checks do not qualify WAN performance or sustained group use.
