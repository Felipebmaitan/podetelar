# Approved settings and source preview

Implemented on `feat/stream-first-ui` from the approved [Advanced](../../output/imagegen/settings-source-proposal/advanced-settings.png)
and [source preview](../../output/imagegen/settings-source-proposal/source-preview.png)
ImageGen proposals.

## Behavior

Advanced uses Vídeo, Recepção and Conexão sections, two video-setting columns,
information tooltips and collapsed Diagnóstico. The latter retains connection
rates/errors, previous-measurement qualification, playback retry, codec and live
stream diagnostics. The footer explains when publishing changes apply and keeps
Concluir visible. At 1280×720 the default content fits; 960×540 uses an internal
scroll region. The redundant microphone-off row is removed, without changing
audio acquisition policy.

Opening the source list displays a 16:9 confirmation popover on its right.
Hover/focus changes the preview; clicking changes the pending source and dismisses
the list. Selecting a source never starts or reconfigures publishing. Start/Apply
remains explicit. The popover has no publish/playback controls and intercepts
pointer hit testing so covered stream controls cannot receive clicks through it.

`source_picker.rs` owns a separate local-only worker. It has one overwriteable
request slot and one result, a 180 ms settle delay, generation cancellation,
single-source caching and a 2 s sample-wait deadline. A driver state transition
is not guaranteed by that sample deadline; cancellation never blocks the UI.
Each job validates a currently enumerated source and obtains one 480×270 RGBA
still using D3D11 capture/conversion/download. It has no audio, hardware encoder,
admission or upload path. All pipeline exits request Null. Close, hide, fullscreen
or leaving clears the local cache and cancels obsolete work. No local source
pixels are written to disk. Uploaded preview capture timing is unchanged.

## Verification

- Native formatting/Clippy, Node 24 `npm run check`, portable tests and all 44
  publisher tests passed. Worker tests check latest-hover replacement, same-source caching,
  and clearing during capture. Renderer tests check the concise Advanced groups,
  30 Mbps fallback, hover-versus-click source selection without publication, and
  blocking clicks to covered stream controls.
- The separately opt-in hardware test
  `source_picker::tests::real_gpu_still_capture_without_encoder_audio_or_publication`
  passed on this Windows machine using a real D3D11 pattern, a real monitor and a
  real visible window through WGC (0.89 s combined test execution). Those pixels stayed in test-process
  memory and were not exported. This is local source-capture evidence, not AMD,
  codec publication, audio or 4K120 qualification.
- A fresh GPT-6.1-Sol high technical review approved the implementation without
  blocking findings. Its minor previous-measurement copy suggestion was addressed
  inside Diagnóstico.
- [Native-renderer captures](images/settings-source/) cover standard and compact
  Advanced/source states, expanded diagnostics and the main grid. They are
  explicitly synthetic DEMO fixtures with synthetic source labels and supplied
  game content; they establish layout, not real publication. Temporary fixture
  hooks are removed before final checks and the ordinary build.

The prior Gemini multimodal-image transport limitation remains recorded in the
earlier visual reports. No fabricated visual score or completed field gate is
claimed. Existing media/packaging prerequisite edits remain separate local work.

## Rapid-hover and window-discovery follow-up

Pointer movement now keeps the last hovered row across gaps and the confirmation
popover; explicit keyboard navigation uses the focused row. Local captures retry
a transient failure once after 300 ms, cancelling that delay when another source
is requested. The worker, pending request and cached result remain bounded.

The source catalogue refreshes quietly at most once per second while open,
without touching operation feedback, capture authority or encoder/audio probes.
Discovery excludes hidden, minimized, DWM-cloaked, tool and nonactivating windows.
Closed windows disappear on refresh. Active HWND/PID validation is unchanged;
minimizing an already shared window does not revoke its source. Windows can keep
a cloaked window's visible style set, so visibility alone is insufficient; see
[DWMWA_CLOAKED](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute).

Native formatting/Clippy, Node 24 `npm run check`, the normal native build and
all 49 publisher tests passed. Added regression
coverage includes rapid A→B→A switching during capture, transient recovery,
bounded failure retries, cancellation during retry backoff, pointer/keyboard
arbitration and quiet refresh authority. Both opt-in desktop tests also passed:
real HWND visibility/minimize/cloak/tool-window fixtures and actual D3D11
pattern/monitor/WGC-window still capture. They exported no private source pixels
or window inventory. A fresh GPT-6.1-Sol high review approved this scoped fix
without findings. Interactive fast-hover UX remains a field check.

## Graceful native window close

The root viewport cancels OS close until the publisher worker has completed
shutdown and its Tokio runtime has been destroyed. Shutdown cancels pending join
and capture authority immediately and uses a dedicated notification with priority
over queued commands. New actions are rejected after shutdown begins. The existing
teardown stops/unpublishes the stream, closes the media room, leaves the admission
session and releases playback before completion allows window destruction.
The closing screen stays responsive; repeated X clicks do not bypass cleanup.
An explicit Force close button appears after ten seconds if cleanup stalls.

Node 24 `npm run check`, native formatting/Clippy, all 51 publisher tests and the
normal native build passed. Regressions cover a saturated command queue, capture
cancellation, rejected new work, repeated OS close requests, deferred window
destruction and completion-triggered exit. A new GPT-6.1-Sol high review approved
the scoped change. The rebuilt ordinary GUI accepted an actual Windows close
message and exited in 281 ms without forced termination. That smoke did not start
a publication; live watcher notification timing against an SFU remains unmeasured.
