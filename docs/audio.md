# Windows application audio

Choose **Off**, an application, or **Desktop application mix** in the publisher. Refresh sources to list applications with existing audio sessions. Application selection follows the same executable path across process restarts. Stop sharing before changing audio mode or exclusions. The microphone is never captured.

Desktop mix combines audio from discoverable Windows application sessions, with a maximum of 16 captured process trees. Discord, Discord PTB/Canary and their descendants are always excluded. The publisher's own process tree, including its Watch friends window, is excluded too. Exclude more applications with the checkboxes; these exclusions follow the selected executable path across restarts. Exclusions take priority over inclusion. System sounds without an attributable process and inaccessible/protected processes are omitted.

Use **Watch friends** to open the room in a separate Chrome window owned by the publisher. It uses a new temporary profile and incognito mode, so game/browser audio and room playback occupy separate process trees. Chrome must be installed for this button; it is not bundled. Regular browser invitations still work. When a publisher watches in their normal browser instead, exclude that browser in the audio settings: Windows cannot reliably separate two tabs sharing an audio process. Publishing game/application audio does not automatically exclude every browser on the computer.

Application loopback needs Windows build 20348 or newer (normally Windows 11 on desktop). Video capture remains separate from that requirement. There is no fallback to an unfiltered endpoint/system mix on older Windows, discovery failures or capture errors.

## Capture and lifecycle

Each approved process tree gets its own WASAPI process loopback through GStreamer. The pipeline converts to 48 kHz, two-channel PCM and mixes in a common live clock. LiveKit encodes Opus with a 192 kbps target and DTX disabled; the SFU forwards it without decoding. Echo cancellation, voice noise suppression and automatic gain control remain off so they do not alter game/music audio.

The application discovers sessions across active render endpoints and deduplicates ancestor/descendant captures. A PID plus creation time identifies a capture, and an owned process handle prevents that PID from being recycled while its loopback is alive. Unknown identities are never eligible. A protected descendant makes the whole ancestor tree ineligible, since Windows' process-tree capture cannot subtract that descendant.

Discovery refreshes every 500 ms. A changed set is rebuilt after closing the old captures and clearing queued PCM; a short audio gap can occur on application or routing changes. Discord restarts do not become eligible, including when its version directory changes. Newly appearing applications can enter Desktop mix; an application selection remains restricted to its executable path. An application moving to a different executable path needs a new selection. Capture/discovery failure stops the sharing group with an error instead of publishing an unfiltered mix.

Audio and video share a cancellation lease. Stop, Leave, closing the publisher, a terminal disconnect, or failure to recover either publication ends both pumps. Stop waits for native capture release before network cleanup. Queues and the SDK audio buffer are bounded. No raw PCM or desktop recording is written to disk or sent to the app server.

The lease disables retained RTC tracks on cancellation, and late publication/republication of a disabled track is removed. Stop resolves the current IDs from the stable track handles, because full reconnects replace publication IDs. Stopping during recovery, or finding an owned publication left after cleanup, closes the media identity to prevent a delayed republish; joining again is then required.

## Isolated viewer

Chrome starts suspended, enters a Windows job with kill-on-close, then resumes. All early browser children inherit ownership. Closing/leaving the publisher terminates that job; closing the viewer window also ends its playback. The temporary profile is removed after process teardown, with a bounded retry for Windows file-mapping/scanner races.

The invitation is validated first and passed by a local DevTools message after Chrome starts at `about:blank`. It never appears in process arguments. DevTools listens on an automatically selected loopback port, and its address is validated before use. This port is for local native control and is not published to the LAN or VPS. The room UI strips invitation authority from the address bar as usual. Normal browser profiles and cookies are not accessed. Treat processes already running as your Windows user as trusted; protecting process memory from a hostile local user is outside this application model.

## Verification

`npm run native:test` exercises selection, exclusions, restarts, PID reuse and protected descendant rules. With Chrome, the native runtime and a working audio endpoint, `npm run native:e2e -- audio.spec.ts` checks actual process loopbacks using quiet generated tones: game 440 Hz, Discord 880 Hz, and the owned viewer 1320 Hz. The game and Discord fixture processes restart during capture. A second capture excludes the game explicitly. Only bounded spectral summaries are returned; no recording is retained or uploaded. Restart clicks have broadband energy, so rejection is measured in steady windows before and after the restart.

`audio-stream.spec.ts` publishes only the generated game fixture through the actual SFU. Its left/right tones differ (440/660 Hz), so the browser check verifies channel separation as well as Discord rejection, explicit A/V subscription traffic stopping, rewatch and native release. These fixtures do not establish real-game, headset-switching or an evening of friends' usage; those remain field qualification work.

The explicit diagnostic commands `native:run -- --audio-apps` and `native:run -- --qualify-audio` can reveal application names/paths or capture local audio into a transient analysis buffer. Do not paste application inventories into public logs. The audio qualification command accepts bounded JSON on stdin, runs at most 20 seconds and emits aggregate counters/spectral measurements only. It never uploads PCM.

Primary references: [Microsoft process loopback](https://learn.microsoft.com/en-us/samples/microsoft/windows-classic-samples/applicationloopbackaudio-sample/), [GStreamer WASAPI process modes](https://gstreamer.freedesktop.org/documentation/wasapi2/wasapi2src.html), [GStreamer live audio mixing](https://gstreamer.freedesktop.org/documentation/audiomixer/audiomixer.html).
