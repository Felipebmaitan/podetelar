// Compare the packaged SDK's actual capabilities with this Mac's VideoToolbox.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { join } from "node:path";
const app = process.argv[2];
assert.ok(app, "Pass a packaged application path.");
const clean = {
  ...process.env,
  DYLD_LIBRARY_PATH: "",
  DYLD_FALLBACK_LIBRARY_PATH: "",
};
const hardware = JSON.parse(
  execFileSync(
    "xcrun",
    [
      "swift",
      "-e",
      `import VideoToolbox
import Foundation
VTRegisterSupplementalVideoDecoderIfAvailable(kCMVideoCodecType_AV1)
let codecs: [(String, CMVideoCodecType)] = [("video/h264", kCMVideoCodecType_H264), ("video/h265", kCMVideoCodecType_HEVC), ("video/av1", kCMVideoCodecType_AV1)]
print(String(data: try! JSONSerialization.data(withJSONObject: codecs.filter { VTIsHardwareDecodeSupported($0.1) }.map { $0.0 }), encoding: .utf8)!)`,
    ],
    { env: clean, encoding: "utf8", timeout: 60000 },
  ),
);
const probe = JSON.parse(
  execFileSync(
    join(app, "Contents/Helpers/PodeTelar.app/Contents/MacOS/FreedomStream"),
    ["--probe"],
    {
      env: {
        ...clean,
        GST_PLUGIN_SYSTEM_PATH_1_0: join(app, "Contents/PlugIns"),
        GST_PLUGIN_PATH_1_0: "",
        GST_PLUGIN_SCANNER_1_0: join(app, "Contents/MacOS/gst-plugin-scanner"),
      },
      encoding: "utf8",
      timeout: 15000,
    },
  ),
);
assert.deepEqual(
  [...new Set(probe.decoders)].sort(),
  hardware.sort(),
  "Packaged decoders differ from available hardware; no software fallback is allowed.",
);
assert.ok(
  !probe.encoders.some((encoder) => encoder.codec === "av1"),
  "macOS must not advertise AV1 encoding.",
);
console.log(
  JSON.stringify({ verifiedHardwareDecoders: hardware, av1Encoding: false }),
);
