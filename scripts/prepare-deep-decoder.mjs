// MIT. Generate an isolated exact-current decoder plus explicit experimental hooks.
import { readFileSync, writeFileSync, mkdirSync, copyFileSync } from "node:fs";
import { resolve } from "node:path";
import { createHash } from "node:crypto";
const root = resolve(import.meta.dirname, ".."),
  out = resolve(root, ".local/experiments/deep-pass/decoder");
mkdirSync(out, { recursive: true });
const hash = (s) => createHash("sha256").update(s).digest("hex");
const replace = (s, a, b) => {
  if (s.split(a).length !== 2) throw Error(`Expected one exact marker: ${a}`);
  return s.replace(a, b);
};
const originals = {};
for (const name of [
  "freedomstream_decoder.h",
  "deferred_i420.h",
  "bounded_readback.h",
]) {
  const source = readFileSync(
    resolve(root, "apps/publisher/decoder", name),
    "utf8",
  ).replaceAll("\r\n", "\n");
  originals[name] = hash(source);
  let s = source;
  s = replace(
    s,
    "#pragma once",
    '#pragma once\n#include "deep-decoder-support.h"',
  );
  if (name === "freedomstream_decoder.h") {
    s = replace(
      s,
      "    Stop();\n    failed_generation_",
      "    Stop();\n    deep::InstallTrace();\n    failed_generation_",
    );
    for (const codec of ["h264", "h265", "av1"])
      s = replace(
        s,
        `! d3d11${codec}dec !`,
        `! d3d11${codec}dec name=fsdeepdecoder !`,
      );
    s = replace(
      s,
      "    if (!pipeline_) return false;",
      "    if (!pipeline_) return false;\n    deep_device_ = deep::Setup(pipeline_);",
    );
    s = replace(
      s,
      "    readback_.reset();",
      "    readback_.reset();\n    deep_device_.reset();",
    );
    s = replace(
      s,
      "    if (!sample) return GST_FLOW_EOS;",
      "    if (!sample) return GST_FLOW_EOS;\n    deep::Observe(sample);",
    );
    s = replace(
      s,
      "  const bool hevc_;",
      "  std::shared_ptr<deep::Device> deep_device_;\n  const bool hevc_;",
    );
  }
  if (name === "deferred_i420.h") {
    s = replace(
      s,
      "      if (readback_) { converted = readback_->Copy(sample_, info_, pixels.get()); }",
      "      if (readback_) { converted = readback_->Copy(sample_, info_, pixels.get()); if(converted)deep::bounded_success++;else deep::generic_fallback++; }",
    );
    s = replace(
      s,
      "readback_(std::move(readback)) {}",
      "readback_(std::move(readback)) { deep::deferred_live++; }",
    );
    s = replace(
      s,
      "    if (sample_) gst_sample_unref(sample_);",
      "    if (sample_) gst_sample_unref(sample_);\n    deep::deferred_live--;\n    if(pixels_) deep::planes_live_bytes -= static_cast<uint64_t>(StrideY())*height()+static_cast<uint64_t>(StrideU()+StrideV())*((height()+1)/2);",
    );
    s = replace(
      s,
      "      pixels_ = std::move(pixels);",
      "      deep::materialized++;\n      if(!converted)deep::failures++;\n      deep::planes_live_bytes += static_cast<uint64_t>(StrideY())*height()+static_cast<uint64_t>(StrideU()+StrideV())*((height()+1)/2);\n      pixels_ = std::move(pixels);",
    );
  }
  if (name === "bounded_readback.h") {
    s = replace(
      s,
      "  ~BoundedReadback() {\n    staging_.Reset();",
      "  ~BoundedReadback() {\n    if(staging_)deep::staging_live--;\n    staging_.Reset();",
    );
    // There is a second reset on size/device change, independently balanced.
    s = replace(
      s,
      "      staging_.Reset();",
      "      if(staging_)deep::staging_live--;\n      staging_.Reset();",
    );
    s = replace(
      s,
      "    std::lock_guard<std::mutex> guard(mutex_);",
      "    const auto lock_start=deep::Clock::now();\n    std::lock_guard<std::mutex> guard(mutex_);\n    deep::readback_lock_ns+=deep::Ns(deep::Clock::now()-lock_start);\n    deep::readbacks++;",
    );
    s = replace(
      s,
      "{ gst_d3d11_device_lock(device); }",
      "{ const auto start=deep::Clock::now();gst_d3d11_device_lock(device);deep::device_lock_ns+=deep::Ns(deep::Clock::now()-start); }",
    );
    s = replace(
      s,
      "        return false;\n    }\n    context->CopySubresourceRegion",
      "        return false;\n      deep::staging_live++;\n    }\n    const auto copy_start=deep::Clock::now();\n    context->CopySubresourceRegion",
    );
    s = replace(
      s,
      "    if (FAILED(context->Map(staging_.Get(), 0, D3D11_MAP_READ, 0, &mapped))) return false;",
      "    deep::copy_ns+=deep::Ns(deep::Clock::now()-copy_start);\n    if (FAILED(deep::Map(context,staging_.Get(),dmem->device,&mapped))) return false;",
    );
    s = replace(
      s,
      "    bool converted = false;",
      "    const auto convert_start=deep::Clock::now();\n    bool converted = false;",
    );
    s = replace(
      s,
      "    {\n      DeviceLock locked(device_);",
      "    deep::convert_ns+=deep::Ns(deep::Clock::now()-convert_start);\n    {\n      DeviceLock locked(device_);",
    );
  }
  writeFileSync(resolve(out, name), s);
  if (name === "freedomstream_decoder.h")
    writeFileSync(
      resolve(out, "deep-adapter.h"),
      s.slice(0, s.indexOf("\nclass DecoderFactory final")) +
        "\n} // namespace freedomstream\n",
    );
}
copyFileSync(
  resolve(root, "scripts/experiments/deep-decoder-support.h"),
  resolve(out, "deep-decoder-support.h"),
);
writeFileSync(
  resolve(out, "source.json"),
  JSON.stringify(
    {
      schema: 1,
      base: "e93b71d437810192bd1648e65088f1a6b9196754",
      originalNormalizedSha256: originals,
      generated: Object.fromEntries(
        [...Object.keys(originals), "deep-decoder-support.h"].map((name) => [
          name,
          hash(readFileSync(resolve(out, name))),
        ]),
      ),
    },
    null,
    2,
  ),
);
console.log(JSON.stringify({ prepared: out }));
