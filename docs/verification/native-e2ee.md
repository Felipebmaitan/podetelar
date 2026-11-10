# Native invitation encryption verification

Date: 2026-11-10. Branch: `feat/native-room-e2ee`, based on
`0d16ce85a4408209a49975da1d36ddff6c652d14` (`origin/main`). This PR is explicitly
not authorized for merge or deployment.

## Scope and review gate

The pre-implementation GPT-6.1-Sol high-reasoning review approved proceeding with
conditions for invitation-holder confidentiality. The [design](../native-e2ee.md)
states the limits: no human/device identity, FS/PCS, full replay protection or
malicious-server-resistant revocation of a known root. Implementation review and
real-media results are separate gates.

An organizer creates/replaces private invitation material in the native app.
Guests keep paste-and-join. Legacy invitations remain unprotected. Only the
admission credential enters API requests. Server-side preview storage is opaque.

## Runtime changes requiring verification

The pinned SDK setup adds strict checks that an SFU TrackInfo cannot downgrade the
client's requested encryption before sender activation. Native audio/video pumps
wait until publication and GCM cryptor readiness. The SDK closes a protected full
reconnect without republishing under retained keys; native recovery must create
fresh admission and sender-session material. Source/codec replacement retains the
protection requirement. Source inspection is not packet-level nonce telemetry.

`deploy/native-sdk.json` pins upstream archive, all four patched source files and the
whole generated SDK tree. The original checkout's generated decoder differed
from this branch's pins, so this worktree generated its own SDK from the verified
archives. Builds use a short target path to avoid Windows dependency-extraction
path limits. The GStreamer runtime is the existing verified local runtime.

## Automated checks

- Node 24.11.1 `npm run check`: passed (42 server/API, 23 web and 4 configuration
  tests, plus formatting, types and builds). Includes
  protected envelope API admission, publication proof, exact MIME, bounds,
  cadence across format changes, stopped-stream cleanup and unchanged legacy JPEG
  rejection behavior.
- `node --test scripts/native-tools.test.mjs`: 3 passed, including tampered
  archive/generated source rejection.
- Native formatting, Clippy and 41 publisher + 7 portable tests passed, including
  Windows DPAPI protected invitation save/restore, root absence, tampering and
  Forget. Re-run after the final SDK attachment-order change remains required.
- Real SFU `native-e2e/e2ee.spec.ts`: pending final recording. These tests must
  verify actual native decode/audio/preview and denial paths, not only requested
  encryption settings.
- CPU measurement: pending. The opt-in `e2ee-performance.spec.ts` uses two
  alternating-order rounds with one/four independent watcher processes, a D3D11
  snow fixture, 1080p60 and a 35 Mbps target. It records actual delivery separately.
  This is not four streams decoded in one app or a gaming/latency measurement.

## External and unverified gates

Do not infer an AMD GPU result from this machine's RTX 5080. Physical game impact,
multiple independent friends publishing into one receiving app, network/device
audio timing, fresh-PC/offline distribution and 4K120 are not established by this
phase's local tests. No new portable release or deployment is implied.

The two `e2ee-faults.spec.ts` real-SFU tests passed: fault-injected outgoing
GCM-to-None negotiation was refused; a backend-minted identity and replayed public
descriptor plus forced plaintext subscription and None-to-GCM metadata delivered
RTP bytes but zero decoded video, audio or pictures. Full
packet-level key/nonce collision instrumentation and complete replay/liveness
protection are not claimed. Record which reconnect paths were actually exercised.

## Independent implementation review

The first independent implementation review requested changes to `a77ddb0` for a
plaintext receiver bypass: mutable None-to-GCM signaling could label a receiver
protected without attaching its cryptor. `6997147` added mandatory SDK receiver
cryptors and an actual-readiness gate; real malicious-signaling tests passed.

Re-review found a two-read receiver race. `c1e8621` captures the receiver once and
also refuses remote descriptors targeting the local sender key slot. A subsequent
focused review traced overlapping same-SID SDK callbacks which could still
associate readiness with another receiver. The final SDK patch serializes each
publication's attachments and exposes its receiver only after the synchronous
subscription/encryption callback completes. A dedicated regression test overlaps
two attachments while the first callback is blocked. Final re-review is pending.

Optional upstream hardening remains separate: the C++ cryptor's malformed short
frame parsing has unchecked indexing/subtraction in the supplied source. Review
established a crash/large-allocation concern, not content or key disclosure. This
is not a claim of a comprehensive upstream runtime audit. Operator denial of
service is outside the stated confidentiality model.

Review approval is evidence about the stated scope, not a guarantee that all
software vulnerabilities have been eliminated.
