# Friends feedback beta delivery — 9 November 2026

Felipe explicitly requested rebuilding the shareable EXE and deploying these
changes to the existing VPS. This is an updated friends beta, not completion of
the physical A/V, AMD, fresh-PC, 4K120 or group/game qualification gates. The PRs
remain separate drafts; this delivery does not merge them.

## Delivered build

- Source, executable and server image: `39aaffe48b53b794bb34f4f9df06dcad4afebae7`.
- Versioned EXE: `freedomstream-windows-x64-0.1.0-39aaffe48b53.exe`.
- Size: **81,808,351 bytes**.
- SHA-256: `070f23232e76f0598ebbe92746e1169053b45f614717d11dd896f24a9cfc58e8`.
- An identical local copy is named `FreedomStream.exe` for sharing.
- Portable, bundled runtime/notices/sources, GUI subsystem; no installation or
  shared media-runtime download is required. This is not a fresh-PC acceptance.

The existing Friends invitation remains valid. Admitted room downloads now serve
this build; anonymous downloads remain denied. No invitation, session or server
secret is published here.

The delivered UI includes the room-member menu, own-source cached preview, simpler
room actions, live encoded FPS/sent bitrate and PT-BR/English selection. Automatic
connection measurement and upload/viewer/transport caps are included. **The slight
audio-delay issue is not fixed by this delivery**. The app includes timing
diagnostics; a repeatable test-source generator is available in the repository
for the remaining investigation.

## Package qualification

Portable qualification passed isolated/concurrent extraction, GUI subsystem,
runtime probes, payload/corresponding-source integrity, completed-cache
preservation, explicit extraction and damaged-package rejection.

All **eight packaged native media tests passed** in 3.2 minutes using the actual
shipped runtime: desktop audio exclusions/restart, HEVC and H.264 decoding,
previews, explicit unsubscribe/rewatch, compatibility fallback, stereo game audio,
full reconnect, unwatched preview recovery and aggregate watcher caps. These
results do not measure physical A/V synchronization.

## Deployment

Built an immutable application image with bounded build CPU/RAM before activation.
Created a verified WAL-aware database backup and retained the previous release,
manifest and proxy configuration. No credential/TLS files were regenerated.

Only the FreedomStream app container was replaced. The existing SFU/relay and
unrelated containers retained their IDs. Hash comparisons confirmed unrelated
nginx site files were unchanged. The dedicated upload-test route permits 24 MiB
and disables request buffering; the normal 256 KiB preview ceiling remains.
`nginx -t`, local/public health, artifact checksum/size and state-file preservation
checks passed.

A fresh operations review caught two rollback defects before activation:
ordinary copying could not replace a mode-400 manifest, and failures could be
followed by a false success message. Rollback now restores the manifest with a
sibling temporary file and atomic rename, accumulates failures, and verifies the
old image, symlink, metadata, environment, proxy and health before reporting
success. An actual non-root dummy-file test verified mode-400 atomic restoration.
A second fresh GPT-6.1-Sol high review approved the corrected beta operations.
The uploaded script passed shell syntax checking. A full live rollback was not
triggered or claimed.

## Public HTTPS checks

- Existing invitation accepted; native publishing/subscribing grants work and
  browser media publishing/subscribing grants remain denied.
- Full admitted EXE download matched the complete size and SHA-256 above.
- Anonymous EXE download returned 401. A 100,000-byte anonymous preview reached
  API admission (401), while 262,145 bytes returned 413 at the proxy boundary.
- Packaged native H.264 decoded **1,067 frames**, HEVC **1,213 frames**, and a
  relay-policy H.264 publisher with an automatic-route watcher **1,085 frames**.
  All reported stopped capture and closed receivers. This is not deliberately
  blocked-UDP or watcher-forced-relay qualification.
- A two-client concurrent native bandwidth run completed through production
  HTTPS in 11.64 seconds: download approximately 95.0/100.0 Mbps and upload
  93.6/83.3 Mbps. Derived allowances and upload ceilings retained headroom.
- **Measurement reliability is still a beta limitation:** the first concurrent
  run reported failure without retaining individual result payloads. A later run
  overlapping production media produced one unavailable measurement and one
  successful 100.0/82.9 Mbps result. The cause is not established. Do not claim
  that every automatic test, or measurement under media load, is qualified.
  The app reports unavailable rather than fabricating capacity; Advanced offers
  rerun. Last available measurements are retained after incomplete reruns.
- A post-check app sample was healthy at approximately 70 MiB RAM; it is a sample,
  not a peak-resource or full-evening load qualification.

Owned media clients stopped and temporary verification invitations were disabled
with session revocation. The original Friends room and invitation were preserved.
Ignored operator logs retain the package, rollback, deployment, download and
individual measurement observations. See [phase 12](phase-12.md) for implementation
and remaining acceptance, and [the timing source](av-sync-fixture.md) for the
unresolved wired A/V measurement.
