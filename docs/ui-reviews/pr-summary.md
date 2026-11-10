# Native app redesign PR

The review branch `feat/stream-first-ui-pr` starts at current `main` and contains
the native UI work from `feat/stream-first-ui`, including its source-picker and
graceful-close fixes. Already merged media, icon, administrator and deployment
work remains on the base; unrelated local decoder experiments are excluded.

## Screenshots

The Before images are Felipe's supplied application screenshots. After images
are captures of the implemented native renderer with explicit DEMO fixtures.
Game imagery, participants, codecs and bitrate readouts in those demo captures
illustrate layout; they do not establish live media performance. No private
invitations, session credentials or unrelated desktop/window inventories are
included. The older pass reports and capture directories record design history.

| View                | Before                                                     | After                                                     |
| ------------------- | ---------------------------------------------------------- | --------------------------------------------------------- |
| Streams             | [Original app](images/pr-comparison/before-streams.png)    | [New stream grid](images/settings-source/grid.png)        |
| Advanced            | [Dense settings](images/pr-comparison/before-advanced.png) | [Grouped settings](images/settings-source/advanced.png)   |
| Source confirmation | Source selection had no local confirmation                 | [Local source preview](images/settings-source/source.png) |

## Behavior

- Larger stream surfaces, Inter typography, compact room/member sidebar and
  custom right-click controls for watching, sound, focus and fullscreen.
- Received codec appears immediately left of measured bitrate on hovered watched
  streams. Unsubscribed previews never show receive metrics.
- Advanced settings use Video, Reception and Connection groups with diagnostics
  collapsed. The redundant microphone-off row and route selector are removed.
  Automatic watching allowance defaults to 30 Mbps without a measured download.
- The local source popover captures a debounced still without publishing, audio,
  encoding or upload. A bounded retry recovers transient capture failures;
  generation cancellation discards obsolete work. Pointer and keyboard navigation
  remain distinct. Uploaded preview timing remains start/once per minute.
- While open, source discovery refreshes quietly and excludes hidden, minimized,
  cloaked, tool and nonactivating windows. Existing publication validation keeps
  its HWND/PID ownership checks so minimizing does not revoke a shared source.
- Window close cancels pending join/capture, prioritizes shutdown over the bounded
  command queue and waits for stream unpublication, room departure and worker
  teardown. Repeated X clicks cannot skip cleanup. Force close is available after
  ten seconds if cleanup stalls.

The detailed implementation and earlier scoped verification are recorded in
[settings and source preview](settings-source.md),
[typography and menu polish](stream-first-polish.md) and
[the initial design pass](stream-first.md). Live watcher notification timing on
window close remains unmeasured; screenshots do not establish that behavior.

## Validation on the PR branch

With current `main` as the base and its pinned decoder SDK, Node 24
`npm run check`, native formatting/Clippy and all 51 native publisher tests pass,
alongside the portable tests. The earlier scoped reviews approved the UI,
source-picker fixes and graceful shutdown. This PR does not merge or deploy the
application; hardware qualification and live watcher shutdown timing retain
their documented limits.
