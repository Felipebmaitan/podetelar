# Windows application audio

The app derives audio from the selected video source. A window selects only that
window's process tree, including descendants using another executable. Independent
instances using the same executable are not included. Windows cannot separate
windows or browser tabs that share an audio process. Reopening an application can
replace its window/process identity; select the new window before sharing again.

A monitor selects the approved desktop application mix, with a maximum of 16
captured process trees. Discord, Discord PTB/Canary and their descendants are
always excluded, as are FreedomStream's process tree, other official publisher
instances (including different portable versions), and processes using this
instance's exact executable path. Native watched-stream audio stays out of the mix. No microphone device is acquired. Audio routed
into another application's output, such as monitored microphone audio, is still
that application's output. System sounds without an attributable process and
inaccessible/protected processes are omitted; this is not an unfiltered system mix.

NVIDIA Broadcast and its process descendants are also excluded: its processed
desktop output can otherwise reintroduce Discord and watched-stream audio as a
second mixed copy. Original application audio is captured directly. This protects
the observed proxy; arbitrary third-party applications that replay or monitor
other audio remain subject to the output-routing limitation above.

There is no audio-source selector in the UI. The lower-level executable-path and
exclusion modes remain available to diagnostic fixtures. Application loopback
requires Windows build 20348 or newer (normally Windows 11 on desktop). Discovery
and capture failures never fall back to capturing an unfiltered endpoint.

## Capture and lifecycle

Each approved process tree gets its own WASAPI process loopback through GStreamer. The pipeline converts to 48 kHz, two-channel PCM and mixes in a common live clock. LiveKit encodes Opus with a 192 kbps target and DTX disabled; the SFU forwards it without decoding. Echo cancellation, voice noise suppression and automatic gain control remain off so they do not alter game/music audio.

The application discovers sessions across active render endpoints and deduplicates ancestor/descendant captures. A PID plus creation time identifies a capture, and an owned process handle prevents that PID from being recycled while its loopback is alive. Unknown identities are never eligible. A protected descendant makes the whole ancestor tree ineligible, since Windows' process-tree capture cannot subtract that descendant.

Discovery refreshes every 500 ms. A changed set is rebuilt after closing the old captures and clearing queued PCM; a short audio gap can occur on application or routing changes. Discord restarts do not become eligible, including when its version directory changes. Newly appearing applications can enter Desktop mix; an application selection remains restricted to its executable path. An application moving to a different executable path needs a new selection. Capture/discovery failure stops the sharing group with an error instead of publishing an unfiltered mix.

Audio and video share a cancellation lease. Stop, Leave, closing the publisher, a terminal disconnect, or failure to recover either publication ends both pumps. Stop waits for native capture release before network cleanup. Queues and the SDK audio buffer are bounded. No raw PCM or desktop recording is written to disk or sent to the app server.

The lease disables retained RTC tracks on cancellation, and late publication/republication of a disabled track is removed. Stop resolves the current IDs from the stable track handles, because full reconnects replace publication IDs. Stopping during recovery, or finding an owned publication left after cleanup, closes the media identity to prevent a delayed republish; joining again is then required.

## Verification

`npm run native:test` exercises selection, exclusions, restarts, PID reuse and protected descendant rules. With the native runtime and a working audio endpoint, `npm run native:e2e -- audio.spec.ts` checks actual process loopbacks using quiet generated tones: game 440 Hz, Discord 880 Hz, and our native playback fixture 1320 Hz. The game and Discord fixture processes restart during capture. A second capture excludes the game explicitly. Only bounded spectral summaries are returned; no recording is retained or uploaded. Restart clicks have broadband energy, so rejection is measured in steady windows before and after the restart.

`native-room.spec.ts` sends the generated game fixture through the actual SFU and
native receiver. Its left/right tones differ (440/660 Hz), allowing channel
separation and Discord rejection to be measured. It also checks explicit RTP
subscription traffic stopping and rewatch. Synthetic fixtures do not establish
real-game, headset-switching or an evening of friends' usage; those remain field
qualification work.

The explicit diagnostic commands `native:run -- --audio-apps` and `native:run -- --qualify-audio` can reveal application names/paths or capture local audio into a transient analysis buffer. Do not paste application inventories into public logs. The audio qualification command accepts bounded JSON on stdin, runs at most 20 seconds and emits aggregate counters/spectral measurements only. It never uploads PCM.

Primary references: [Microsoft process loopback](https://learn.microsoft.com/en-us/samples/microsoft/windows-classic-samples/applicationloopbackaudio-sample/), [GStreamer WASAPI process modes](https://gstreamer.freedesktop.org/documentation/wasapi2/wasapi2src.html), [GStreamer live audio mixing](https://gstreamer.freedesktop.org/documentation/audiomixer/audiomixer.html).
