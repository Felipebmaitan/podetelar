# PC usage identity

The administrator Usage page groups measured SFU traffic by **PC**, across room
sessions, concurrent instances and display-name changes. It shows the latest
measured session's nickname for the selected period, a stable PC identifier,
combined upload/download totals and a combined graph. Expand a card for session
names, admission times, rooms, identifiers and individual volumes. The latest 50
measured sessions are listed; totals and session count always include all sessions
in the selected period. Pagination applies to PC groups, not their sessions.

## Persistence and scope

The native Windows app reads
[`SystemIdentification.GetSystemIdForPublisher`](https://learn.microsoft.com/en-us/uwp/api/windows.system.profile.systemidentification.getsystemidforpublisher)
on a blocking worker. It accepts TPM, UEFI and registry-backed IDs, rejects absent
or empty IDs, and derives `pc1_` plus a lowercase SHA-256 digest of:

1. The fixed `podetelar.device.v1` namespace and a zero separator.
2. The canonical invitation **public origin** and a zero separator.
3. The Windows-provided identifier bytes.

The raw Windows value never leaves this function, enters a log, or gets written to
an app-owned file. It is neither a MAC address nor an IP, computer name, Windows
username, disk serial number or hardware inventory. The public origin is selected
before direct-server discovery, so changing the direct transport hostname does
not change recognition. Changing the public origin or Windows app publisher
identity can change the derived ID; keep the namespace and derivation fixed
across releases. Ordinary Velopack installation remains an unpackaged Win32 app.

Microsoft documents Windows-reinstall persistence for TPM/UEFI sources and most
hardware changes. The registry fallback survives ordinary restarts/upgrades and
app reinstalls, but **not a clean Windows installation**. We do not invent a random
per-session fallback when the API fails: native admission reports a fixed
`PC identity unavailable` error. Fresh-Windows, TPM reset and motherboard-replacement
behavior has not been locally qualified.

All unpackaged Win32 apps receive the same underlying system ID, according to
[Microsoft's class documentation](https://learn.microsoft.com/en-us/uwp/api/windows.system.profile.systemidentification).
The local app/server derivation avoids directly exposing that common identifier.
The result remains a persistent pseudonymous identifier, not anonymous data or
proof of a person. A modified client can spoof or omit its identifier. This feature
provides usage recognition, not a hardware ban or account authentication.

The pinned windows-rs projection caches agile activation factories. Qualification
found a reproducible access violation on the second independent-thread read after
the last COM apartment was torn down. The device module keeps one process-lifetime
MTA usage reference and balances each worker's `RoInitialize`/`RoUninitialize`.
Repeated-thread and independent-process qualification pass with that correction.
[`CoIncrementMTAUsage`](https://learn.microsoft.com/en-us/windows/win32/api/combaseapi/nf-combaseapi-coincrementmtausage)
keeps MTA resources active between workers; process termination reclaims the one
process-owned reference. It does not retain a raw system ID.

## Admission, storage and compatibility

`POST /api/rooms` and `POST /api/rooms/:id/join` accept an optional `deviceId`
matching `^pc1_[0-9a-f]{64}$`. New native clients always provide it. The ID travels
only in the HTTPS request body (HTTP is allowed only on loopback), never in a URL.
An identifier alone cannot admit a participant or authorize analytics/media.
Invitations and expiring session credentials retain their existing roles.

SQLite adds a nullable `device_id` to `usage_users` and an identity lookup index.
Each new admission gets an immutable session-to-device association. Existing
session IDs, minute buckets, SFU counter baselines and authorization stay separate.
Only reporting sums these measured buckets by `COALESCE(device_id,id)`, so
independent cumulative counters cannot overwrite or double-count one another.
Collector gaps, restarts, minute rounding and the 400-day retention remain as
documented in the [collector accounting](../deploy/livekit-usage/README.md).
Session metadata expires with its retained usage and active-session requirements.

The device ID is exposed only by protected administrator usage APIs, not room
snapshots, member lists, SFU identities, metric labels or media JWTs. This change
adds no IP logging or IP database fields. Automatic request logging remains
disabled; production log settings are unchanged.

Older clients can still join without an ID and appear as **unidentified sessions**.
Existing history remains unidentified; matching nicknames is not evidence that
sessions belong to one PC. An updated native client requires the updated admission
API: older servers' strict schemas reject the additional field. Deploy the server
before distributing the updated native build. This PR does not publish an installer
or deploy production.

## Repeatable qualification

- `npm run check`: migration, grouping across rooms/names, distinct PCs with equal
  names, immutable mappings, pruning, restart, pagination, bounded drill-down,
  malformed input, participant privacy and authorization, plus workspace checks.
- `npm run native:check` and `npm run native:test`: native formatting/Clippy, scoped
  hashing and actual Windows identity reads on independent threads, plus the
  existing native/portable suite.
- `npm run smoke -- --restart-app`: real HTTP, session recovery and SFU access
  against the isolated Docker stack. Set `COMPOSE_FILE`, `COMPOSE_PROJECT_NAME`,
  `SMOKE_BASE_URL` and `SMOKE_LIVEKIT_URL` when using nondefault local ports.
- `node --env-file=.env scripts/verify-admin-usage.mjs`: actual H.264 publication
  and subscription counters for two sessions sharing one synthetic PC ID, combined
  graphs/totals, expandable session UI, desktop/mobile overflow and actual traffic
  cessation on unsubscribe. Set `E2E_BASE_URL` to the isolated API.
- `npm run test:admin`: actual desktop browser administrator/access workflows.
- `node --env-file=.env scripts/verify-device-identity.mjs`: actual Windows ID and
  HTTP admission from two copied Rust test executables, separate processes and
  empty app-data directories; verifies distinct session IDs bind the same PC ID
  in the isolated Docker database. Set `FREEDOMSTREAM_NATIVE_TEST_EXE` to the
  publisher test EXE produced by `native:test`, `FREEDOMSTREAM_DEVICE_DB_CONTAINER`
  to that local API container, and `E2E_BASE_URL` to its loopback origin. Include
  the qualified GStreamer DLL directory in the hidden process's PATH. Private
  fixture inputs/results stay under ignored `.local/device-identity-*`; IDs and
  invitations are never printed.

These tests verify actual local Windows identity reads, process/app-data
independence, native HTTP binding and actual SFU usage. They do not substitute for
fresh-OS or replacement-hardware verification. Copying a test executable is not an
installed-app uninstall/reinstall test.
