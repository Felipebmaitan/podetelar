# Native resource footprint: matched comparison

Measured on 2026-11-09/10 on Windows, Ryzen 7 9800X3D / RTX 5080. Branch
`perf/native-resource-footprint` starts from `origin/main` at `c90410d` in a
separate checkout; the original checkout's ongoing feature edits were preserved.

The retained change is intentionally small: release the verified compressed
portable payload before waiting for the native child, and convert received
pictures directly into the final owned `ColorImage`. No dependency, runtime,
codec, capture, audio, subscription, queue or quality-policy change is included.

## Results

Five matched rounds alternated before/after order. Values below are medians of
the five per-run measurements. The exact per-run values, executable/payload
hashes and measurement timestamp are in
[the compact measurement data](native-footprint-results.json).

| Measurement                                     |    Before |     After |          Change |
| ----------------------------------------------- | --------: | --------: | --------------: |
| Waiting portable wrapper, private bytes         | 78.78 MiB |  1.25 MiB |  77.53 MiB less |
| Waiting portable wrapper, working set           | 84.68 MiB |  7.30 MiB |  77.37 MiB less |
| 1080p conversion p50, one worker                |  2.138 ms |  0.975 ms | 54.4% less time |
| 1080p conversion p50, four workers              |  3.924 ms |  1.930 ms | 50.8% less time |
| 4K conversion p50, one worker                   |  8.549 ms |  3.796 ms | 55.6% less time |
| 4K conversion p50, four workers                 | 24.290 ms | 14.510 ms | 40.3% less time |
| Rust pixel allocation traffic per 1080p picture | 15.82 MiB |  7.91 MiB |        50% less |
| Rust pixel allocation traffic per 4K picture    | 63.28 MiB | 31.64 MiB |        50% less |

Conversion p95 also decreased in all four workloads: 2.805→1.161,
5.032→2.340, 11.336→4.266 and 27.537→18.042 ms respectively. Median process
CPU milliseconds per converted picture, including identical setup/warm-up, fell
from 2.292→1.042, 4.210→2.105, 9.505→4.167 and 27.197→16.260 respectively.
The checksum was identical before/after in each workload. It samples one fixed
pixel per worker and is only a consumption/sanity check, not full-image
equivalence evidence; the separate color/stride fixtures compare all pixels.

The wrapper's private-byte medians ranged from 78.76–78.84 MiB before and
1.23–1.27 MiB after. Both wrappers used zero measurable CPU time during their
three-second waiting samples; this change claims memory savings, not an idle
CPU improvement. The allocation counts fall from two pixel allocations to one
per picture; the benchmark also counts a small fixed results-vector allocation.

These are useful, consistent improvements in the changed paths. They do not
establish whole-app CPU percentages, multi-stream decoder capacity, gaming FPS,
physical A/V synchronization or an RX 7600 stutter fix.

## Reproduction and controls

1. Check out `c90410d` and prepare the checksum-pinned SDK/runtime. Use Node 24,
   Rust 1.95, the unchanged lockfile and workspace release profile (thin LTO,
   one codegen unit, static MSVC runtime). Build with
   `cargo build --locked --release --workspace` and preserve both executables in
   `.local/footprint/before/`.
2. Commit `b549915` adds the benchmark and a helper containing the existing
   conversion algorithm, without changing production behavior. Build
   `cargo build --locked --release --example frame-conversion` there, and preserve
   `target/release/examples/frame-conversion.exe` beside the baseline executables.
3. Build candidate `4584191` with the same compiler, dependencies, target path
   and flags using
   `cargo build --locked --release --workspace --bins --examples`. Preserve
   the two production executables and example in `.local/footprint/after/`.
   Later documentation/tooling commits do not change the measured app code.
4. Stop other controlled builds/media tests before measuring. In a hidden,
   noninteractive PowerShell process with the qualified GStreamer `bin` on PATH:

   ```powershell
   & scripts/measure-native-footprint.ps1 `
     -Before .local/footprint/before `
     -After .local/footprint/after `
     -Payload <identical-verified-portable-payload.zip> `
     -Output .local/footprint/comparison.json -Runs 5
   node scripts/summarize-native-footprint.mjs `
     .local/footprint/comparison.json <new-summary.json>
   ```

The conversion example imports the **same helper as production playback**; no
alternate optimized algorithm is hidden in the benchmark. It uses deterministic
I420 planes, ten warm-up pictures per worker, then 300/180 pictures per worker
at 1080p and 120/80 at 4K for one/four workers. Timings include destination
allocation, conversion and release. A counting allocator measures Rust
allocation traffic, not C++ allocations, resident RAM or GPU memory. Four
conversion workers are not four actual network subscriptions. No decode,
rendering, texture upload or network work runs in this stage benchmark.

The wrapper test appends the identical existing, verified
`freedomstream-payload-0.1.0-39aaffe48b53.zip` to both freshly built launchers.
Its full payload hash and 81,131,943-byte size are recorded in the data. The
real bundled native child initializes its runtime and blocks on stdin in
`--watch-fixture` before any capture/room activity. A private LocalAppData cache
prevents saved-room reuse. Each run waits for the child, settles for one second,
then samples wrapper private bytes/working set every 50 ms for three seconds.
The wrapper is terminated and its lifetime job must remove that exact child.
Both versions retain the same payload verification/extraction behavior; cold
startup peak memory is unchanged by the early drop.

Raw samples, immutable before/after binaries and baseline/candidate SFU reports
are retained under `.local/footprint/` and are not committed. The compact data
contains only synthetic aggregates and binary hashes. There is no process
command-line, invitation, session/media token or captured desktop content.
This local desktop was not a controlled laboratory: alternating order and five
runs reduce temporal bias but do not eliminate background scheduling variance.

## Correctness and acceptance

- Node 24 `npm run check`: formatting, types, API/unit tests and production builds
  passed before and after the application edits.
- `cargo fmt --all -- --check` and release Clippy for the workspace/all targets
  passed. The pinned upstream SDK's existing warnings remain upstream.
- Release tests passed: seven portable tests, 36 publisher tests and two example
  tests. These include child lifetime/quoting/environment, payload/path/cache
  defenses, Windows DPAPI and new padded/odd-dimension pixel-equivalence and
  opaque-red/channel-order fixtures.
- **Both before and after** passed four real local-SFU cases: NVIDIA H.264/HEVC,
  AV1 D3D11 hardware decode and forced AV1 CPU decode. Each exercised real
  publication, preview, Watch/Stop traffic and rewatch; AV1 also exercised
  reconnect. This verifies media correctness, not an A/B throughput benchmark.
- The local qualification package
  `freedomstream-windows-x64-0.1.0-458419194930.exe` passed isolated-path first
  launch, concurrent extraction, installed/bundled CRT probes, GUI/icon/metadata
  checks, payload/source integrity, modified-cache preservation, explicit
  extraction, redirected-path rejection and damaged-payload rejection.
  This does not certify a fresh PC or a disconnected network.
- The same four codec/preview/Stop/rewatch cases also passed using the actual
  packaged EXE and its isolated bundled runtime (four passed in two minutes).
  The 81,869,962-byte package has SHA-256
  `7c6ba8fa2befc97d35983c55410e69eff68891f5297ec1e1858fd5d5828019e3`.
- A fresh GPT-6.1-Sol high-reasoning read-only review approved
  `c90410d..69f0b24` with no blocking correctness or benchmark-claim findings.
  Its nonblocking P3 noted the checksum's limited pixel coverage; that limit is
  explicit above. Whole-frame hashing can strengthen a future benchmark version
  without changing this version's recorded measurements.

AMD, physical-game performance, group field usage, fresh-PC/offline operation
and physical A/V timing remain separate acceptance gates. This PR does not claim
that any of them was completed by the conversion benchmark or compilation.
