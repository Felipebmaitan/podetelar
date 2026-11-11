# Admin room rework verification

Verified locally on 2026-11-11, using Node 24.22.2 on Windows and the real
Docker application/SFU stack. Scope: the browser administrator UI and room
authority/lifecycle. This does not qualify native capture, codecs, audio,
hardware, portable distribution or remote deployment.

- `npm run check` passed: formatting, workspace/tool types, 49 server tests,
  two web tests, four deployment/relay configuration tests, and builds.
- `docker compose -p freedomstream-admin-rework up --build -d --wait` passed.
- `COMPOSE_PROJECT_NAME=freedomstream-admin-rework npm run smoke -- --restart-app`
  passed: actual HTTP admission, scoped sessions, app restart/persistence, logout,
  built web assets and authenticated SFU administration.
- `npm run test:e2e` passed all eight Playwright tests against that stack.
  The new tests exercise New room dialog focus, inline expansion/collapse by
  keyboard, actual OS clipboard copy, search, reload and invitation recovery,
  no browser storage, cancel/confirm closure, revoked access, and 390px layout.
- A test participant connected using the real LiveKit browser SDK with native
  grants obtained by the test HTTP client. Its connected identity appeared in
  the admin member list. Disconnect/reconnect restored Active presence. Final
  disconnect, real server time and SFU absence expired the room; it disappeared
  and both its session and invitation were denied. No HTTP/presence mock or
  clock fast-forward is used in this browser test. This is a presence test,
  without a publication/subscription or native media qualification claim.
- Deterministic lifecycle/API tests additionally cover cleanup without an admin
  request, pending first join, stale identity filtering, continuous idle presence,
  reconnect/outage grace reset, restart/migration, room-bound AES-GCM ciphertext,
  credential/invitation rotation, same-origin/admin enforcement, immediate
  authority denial and pending SFU disconnect retries.
- Review found a stale-empty-response race at the reconnect grace boundary.
  Successful upstream signaling now resets the grace for an SDK reconnect that
  reuses its session. A deferred-response regression covers the 58s read / 59s
  reconnect / 60s response ordering, and the actual WebSocket gateway test
  confirms upstream connections reset the persisted empty interval.

The approved layout uses one expanded room rather than a separate detail pane
or a bottom creation/share walkthrough. Members show their confirmed presence;
the UI does not infer watching or publishing activity from a connection alone.

Schema 4 adds lifecycle metadata and encrypted invitations while retaining
admission hashes. The encryption key derives from ADMIN_TOKEN. Existing legacy
rooms enter the presence check on migration, including rooms whose sessions were
already removed. Empty legacy rooms expire after successful absence observations;
new managed rooms await their first admission. Outages and restart reset the
grace. The existing deployment backup must precede migration, and rollback to
older code requires restoring its matching schema-3 backup. Rotating ADMIN_TOKEN
preserves admission hashes but requires explicitly replacing invitations before
they can be copied again. History remains available in Usage after room closure.
