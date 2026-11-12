# Public and direct domain split

Verified locally on Windows on 2026-11-12. These results cover source behavior;
the production domain/proxy configuration has not been installed.

## Results

- Node **24.11.1** `npm run check`: formatting, types, 61 server tests, two web
  tests, five relay/deployment tests and all builds passed. The split-domain test
  covers canonical public invitations/handoff, direct native admission and media
  signaling URLs, host-only cookies, foreign-Origin rejection, real HTTP probe
  host routing, and download authority, one-use/replay/expiry/revocation and
  pending-ticket resource bounds.
- `docker compose build`, `docker compose up -d --wait`, `npm run smoke`:
  built assets, real HTTP admission/session persistence/logout and authenticated
  SFU access passed against the local Docker stack.
- `npm run test:admin`: **nine browser tests passed**, including actual attachment
  download through public-to-direct redirect, fragment erasure, same-origin POST,
  byte verification and absence of website cookies on the direct download.
  Existing administrator, mobile, invitation, reconnect and empty-room closure
  workflows passed too.
- `npm run native:check`: Rust formatting and workspace/all-target Clippy passed.
  The pinned development GStreamer runtime was reused through
  `FREEDOMSTREAM_GSTREAMER`; no new runtime is bundled by this change. Existing
  upstream LiveKit warnings remain upstream.
- `npm run native:test`: workspace tests passed (73 publisher tests passed,
  four live-fixture tests ignored in the default run). The new direct-origin
  live-fixture test was explicitly enabled and passed via the command below.
- `node --import tsx scripts/verify-direct-native.ts`: the actual native
  `Admission` client joined through credential-free public discovery and completed
  bounded direct upload/download measurements against the real local API. Its
  session cookie was issued directly; no website-to-direct cookie transfer.
- `node scripts/verify-domain-proxy.mjs`: generated nginx configuration passed
  `nginx -t` in an isolated Docker container. Real requests traversed local TLS
  and nginx to the application: public probes/download redemption returned 421;
  direct authenticated probe and ticket download succeeded, preserving `:8443`
  in the forwarded Host. Ephemeral self-signed certificates are used only by
  this test fixture; this does not establish trusted production TLS.
- Independent GPT-6.1-Sol review at high reasoning approved the source changes
  after fixes for nginx Host-port preservation and canonical public native Invite
  links. The actual browser test also exposed the HTML referrer meta policy:
  the direct download page explicitly uses same-origin policy before its form
  POST, retaining strict Origin validation without putting the ticket in Referer.

The local media directory used by the new split-domain fixtures is a test
directory. Those checks establish HTTP routing, native admission, bulk measurement
and download behavior; they do not establish media transport. The existing Docker
smoke/browser checks exercise the actual local SFU separately.

## Reproduction

Run Node 24 commands from the repository root. Build the web/server output before
the standalone verification commands. Native checks need the pinned development
runtime and native SDK prepared using the documented native setup; an alternate
existing runtime location can be supplied through `FREEDOMSTREAM_GSTREAMER`.

```sh
npm run check
docker compose build
docker compose up -d --wait
npm run smoke
npm run test:admin
npm run native:check
npm run native:test
node --import tsx scripts/verify-direct-native.ts
node scripts/verify-domain-proxy.mjs
```

The proxy fixture requires Docker and downloads `nginx:stable-alpine` plus its
test-only OpenSSL package. It removes its own named container and temporary
certificate/database/artifact directory afterward. It does not execute the dummy
download or modify production certificates, DNS, firewall rules or services.

## Deployment gates

Actual `stream.maitan.me` Cloudflare proxy status and policy, DNS-only direct/TURN
resolution, CA issuance/renewal/install, native Windows activation with the deployed
names, WAN reconnect/revocation, direct selected media candidates and forced TURN
remain unverified here. The existing direct 8443/TURN 5349 layout does not qualify
networks permitting only port 443. Old native clients must be upgraded before
enabling the split. See [deployment](../deployment.md#cloudflare-website-and-direct-service-domains)
for rollout, cookie, caching, logging and shared-host constraints.
