# Friends beta delivery — 9 November 2026

This delivery packages the final reviewed native UI and updates the existing VPS
room to its compatible native-app server. It is a friends beta, not AMD,
fresh-machine or real-game/group qualification. The existing room invitation is
kept in ignored operator files and supplied privately, never in this report.

## Packaged audio check and correction

The initial portable package of `095e680` passed isolated runtime/extraction and
integrity checks and six of eight native media checks. Two audio checks failed:
Discord/owned-playback tones reappeared in the automatic desktop mix.

Controlled diagnosis confirmed that direct Game-only process capture was clean,
and Discord was already absent from allowed application roots. NVIDIA Broadcast
was re-rendering a processed copy of the desktop mix under its own process.
Excluding that one proxy image path reduced the steady 880Hz/1320Hz tones from
approximately 0.02 to roughly 0.000001–0.000003 while preserving the 440Hz Game
tone at approximately 0.02. No microphone capture was opened.

The correction excludes `NVIDIA Broadcast.exe` and its descendants using the
existing protected-tree policy. Original application audio is captured directly;
the processed duplicate must not reintroduce Discord or owned playback. A
regression verifies desktop roots, a selected proxy window, and the allowed-app
inventory. Native formatting/Clippy and **7 portable + 29 native tests** passed.
UI source is unchanged from the final all-seven-9/10 Gemini review.

The corrected portable package, final media tests, deployment and download
verification are recorded below once actually completed. No unperformed hardware
or interactive checks are replaced by the compiled/tested results.
