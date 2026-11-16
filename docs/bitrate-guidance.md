# Bitrate guidance

The [quality guide](quality.md) documents the implemented Automatic targets and
watching policy. Targets are starting points for testing, not universal quality
or hardware guarantees.

Resolution, frame rate, motion, text size, codec and encoder generation all affect
the bitrate needed. Start with Automatic, then compare visible quality and actual
delivery on the intended hardware. If a connection cannot sustain the selected
mode, choose fewer watches, a smaller resolution or a lower frame rate. Raising
a cap cannot repair an overloaded link.

PodeTelar sends one encoding per publication. The lowest active watcher budget
can reduce that encoding for everyone watching it. Per-viewer resolutions would
require additional encodings or layers; the server does not transcode.

## Planning bandwidth

Count each subscription separately. Five publishers each watched by the other
four participants produce twenty outgoing copies at the SFU. At 35 Mbps per
stream that is about 700 Mbps before audio, transport overhead and retransmissions;
at 100 Mbps it exceeds a 1 Gbps port. Leave capacity for competing traffic and TURN.
Per-publication caps do not provide a server-wide bandwidth shaper.

Measure requested, encoded, sent and received rates separately. Include loss,
jitter, latency and hardware load when evaluating quality. Above-60-FPS targets
are extrapolated; they do not establish physical 4K120 capture or playback.

For background, see [NVIDIA's broadcasting guide](https://www.nvidia.com/en-us/geforce/guides/broadcasting-guide/)
and [LiveKit's SFU architecture](https://docs.livekit.io/reference/internals/livekit-sfu/).
Broadcast settings can inform experiments, but do not qualify this application's
low-latency pipeline or another GPU vendor.
