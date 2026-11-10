# Repeatable wired A/V timing source

`npm run native:av-fixture` generates `.local/av-sync/flash-click.avi` using only
Node. Run development tasks through hidden processes as usual. The file is
deterministic and existing different contents are never overwritten.

| Property       | Value                                                              |
| -------------- | ------------------------------------------------------------------ |
| Duration       | 8 seconds, repeatable in a local media player                      |
| Video          | 320×180, 30 FPS, uncompressed RGB                                  |
| Audio          | 48 kHz, 16-bit stereo PCM; no microphone                           |
| Events         | White flash and 20 ms click at 0.5, 1.5, …, 7.5 seconds            |
| Flash duration | 100 ms                                                             |
| File size      | 43,019,852 bytes                                                   |
| SHA-256        | `646829d47951a4c6236909b130c401bdd1d7caed0e4ffa123bf85df560b9fa9d` |

Uncompressed video and PCM avoid codec priming in the **test source**. FreedomStream
still captures and encodes the player window normally, including source audio.
This does not establish the player's or physical output device's synchronization.

## Source validation performed

GStreamer discovery reported an eight-second AVI containing the expected video and
PCM formats. Actual demux/decode of both streams to files completed successfully.
The decoded output contained 240 video frames and 384,000 stereo sample frames.
White transitions were exactly at the eight specified frame times. First nonzero
audio samples occurred one 48 kHz sample after each transition (the click envelope
starts at zero). Demuxed audio and video both had a 0.500000000-second timestamp
for the first event. No accumulating offset was present in this decoded source.

This validates a useful input fixture, **not** FreedomStream's end-to-end A/V
offset or physical speaker/headphone timing.

## Measurement to perform

Felipe confirmed the reported lag used wired devices; whether it began immediately
or grew over time is unknown. Keep both possibilities open.

1. On the watching PC, play the file locally with the same wired output that will
   be used for watching. Record several flash/click pairs at the actual display
   and output. Use the same external observation/recording setup throughout.
2. Stop local playback. On the publishing PC, play the same file and share that
   player window and its source audio. Observe the watcher with the same wired
   device. Keep the app microphone off; the fixture never needs it.
3. Compare the relative flash/click offset against local playback. Record the
   method's timing resolution and uncertainty; a recording device can have its
   own A/V offset. Do not equate callback or queue timestamps with physical output.
4. Sample initial playback, then repeat after sustained looping. Exercise H.264
   and HEVC, reconnect, source changes and multiple watched streams. Record offset
   and variation, including whether delay grows. This distinguishes a stable
   offset from accumulating backlog before selecting a fix.

Physical measurements and a validated correction are still pending. This fixture
does not change the deployed app or constitute an audio-delay fix.
