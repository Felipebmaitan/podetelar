import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
const root = resolve(import.meta.dirname, "..");
const output = resolve(root, ".local/experiments/pipelines");
mkdirSync(output, { recursive: true });
const cases = [];
const add = (label, description, extra = {}) =>
  cases.push({ label, request: { description, ...extra } });
for (const width of [1920, 3840])
  for (const last of [true, false]) {
    add(
      `last-sample-${width}-${last}`,
      `videotestsrc num-buffers=180 pattern=ball ! video/x-raw,format=I420,width=${width},height=${(width * 9) / 16},framerate=60/1 ! appsink name=out max-buffers=1 drop=false sync=false enable-last-sample=${last}`,
      { appsink: "out", expectedFrames: 180 },
    );
  }
const tone = (frequency, extra = "") =>
  `audiotestsrc num-buffers=300 is-live=true samplesperbuffer=480 freq=${frequency} volume=0.02 ! audio/x-raw,format=S16LE,rate=48000,channels=2 ${extra}`;
const device = "wasapi2sink low-latency=true sync=false";
add(
  "audio-four-output-devices",
  [440, 880, 1320, 1760]
    .map(
      (f, i) =>
        `${tone(f)} ! queue max-size-buffers=5 ! volume volume=${i === 2 ? 0 : 1} ! ${device}`,
    )
    .join(" "),
);
add(
  "audio-one-shared-output",
  `audiomixer name=mix ignore-inactive-pads=true latency=30000000 output-buffer-duration=10000000 ! audioconvert ! audioresample ! ${device} ` +
    [440, 880, 1320, 1760]
      .map(
        (f, i) =>
          `${tone(f)} ! queue max-size-buffers=5 ! volume volume=${i === 2 ? 0 : 1} ! mix.`,
      )
      .join(" "),
);
for (const mix of [true, false]) {
  const source = `${tone(440)} ! ${mix ? "mix." : "appsink name=out max-buffers=4 drop=false sync=false enable-last-sample=false"}`;
  const mixer = mix
    ? " audiotestsrc wave=silence num-buffers=300 is-live=true samplesperbuffer=480 ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! mix. audiomixer name=mix ignore-inactive-pads=true latency=30000000 output-buffer-duration=10000000 ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! appsink name=out max-buffers=4 drop=false sync=false enable-last-sample=false"
    : "";
  add(`audio-single-root-${mix ? "mixer" : "direct"}`, source + mixer, {
    appsink: "out",
    pcm: true,
  });
}
for (const sync of [false, true]) {
  add(
    `audio-shared-clock-${sync}`,
    `audiomixer name=mix ignore-inactive-pads=true latency=30000000 output-buffer-duration=10000000 ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! tee name=pcm pcm. ! queue ! ${device.replace("sync=false", `sync=${sync}`)} pcm. ! queue ! appsink name=out max-buffers=4 drop=true sync=false enable-last-sample=false ${tone(440)} ! volume volume=0.5 ! mix. ${tone(880)} ! volume volume=0 ! mix.`,
    { appsink: "out", pcm: true },
  );
}
for (const preset of ["p1", "p4", "p7"]) {
  add(
    `nvidia-hevc-${preset}`,
    `d3d11testsrc is-live=true pattern=ball num-buffers=180 ! video/x-raw(memory:D3D11Memory),width=1920,height=1080,framerate=60/1 ! d3d11convert ! video/x-raw(memory:D3D11Memory),format=NV12 ! nvd3d11h265enc preset=${preset} tune=ultra-low-latency rc-mode=cbr bitrate=6000 gop-size=60 bframes=0 zerolatency=true ! h265parse ! identity name=count ! fakesink sync=false`,
    {},
  );
}
for (const pacing of [true, false]) {
  add(
    `nvidia-pacing-${pacing}`,
    `d3d11testsrc is-live=true pattern=ball num-buffers=180 ! video/x-raw(memory:D3D11Memory),width=1920,height=1080,framerate=60/1 ! d3d11convert ! video/x-raw(memory:D3D11Memory),format=NV12 ! ${pacing ? "clocksync ! " : ""}nvd3d11h265enc preset=p4 tune=ultra-low-latency rc-mode=cbr bitrate=6000 gop-size=60 bframes=0 zerolatency=true ! h265parse ! identity name=count ! fakesink sync=false`,
    {},
  );
}
for (const api of ["dxgi", "wgc"]) {
  // Capture stays in GPU memory and ends at a local discard sink. No desktop
  // pixels, encoded media or audio are saved or transmitted by these cases.
  add(
    `nvidia-capture-${api}`,
    `d3d11screencapturesrc capture-api=${api} show-cursor=false ! queue max-size-buffers=2 leaky=downstream ! ${api === "wgc" ? "imagefreeze is-live=true allow-replace=true ! " : ""}videorate drop-only=true max-rate=60 ! d3d11convert ! video/x-raw(memory:D3D11Memory),format=NV12,width=1920,height=1080,framerate=60/1 ! clocksync ! nvd3d11h265enc preset=p4 tune=ultra-low-latency rc-mode=cbr bitrate=6000 gop-size=60 bframes=0 zerolatency=true ! h265parse ! identity name=count ! fakesink sync=false`,
    { seconds: 3 },
  );
}
const file = resolve(output, "cases.json");
writeFileSync(file, JSON.stringify(cases, null, 2));
console.log(JSON.stringify({ cases: cases.length, file }));
