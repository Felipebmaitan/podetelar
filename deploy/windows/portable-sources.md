# Corresponding library sources

These archives accompany the exact curated media binaries inside this executable.
`../runtime-inputs.json` identifies each archive's upstream URL and SHA-256, each
shipped media DLL/plugin and its component, and the GStreamer/Cerbero revisions.
`../gstreamer-versions.txt` is the upstream package inventory, including components
that PodeTelar does **not** ship. Only the allowlisted binaries are bundled.

GStreamer is revision `202e42fa1bbff4ed61e08d47202db5c744987f0c`, as recorded by
the official 1.28.8 Windows runtime. The complete monorepo source includes the
core, base/good/bad plugins, hardware API header notices and build definitions.
GLib 2.82.4 and proxy-libintl 0.5 are LGPL; their original archives and all Cerbero
patches accompany them. The other library/header archives are also included.
The Cerbero 1.28.8 archive contains the original build recipes, configurations,
patches, license declarations and toolchain/bootstrap scripts for these versions.

For rebuilding, extract Cerbero and follow its Windows README with a licensed
Visual Studio C++ toolchain. Bootstrap using `cerbero-uninstalled -c
config/win64.cbc -v visualstudio bootstrap`. The recipes in `recipes/` specify
the source checksums, patches in application order, dependencies and Meson/CMake
options. Apply those patches to the included dependency archives and build them
in dependency order. The GStreamer source here matches the binary's recorded
Git revision, rather than substituting a newer release. Its `meson.build` and
subproject build definitions can be built against those dependencies; the
corresponding `gstreamer-1.0`, `gst-plugins-base-1.0`, `gst-plugins-good-1.0` and
`gst-plugins-bad-1.0` Cerbero recipes document the Windows configuration.
Reproducing identical binary timestamps/toolchain output is not claimed.

You may modify/rebuild the LGPL libraries and replace the compatible DLLs in the
extracted runtime's `bin/` and `lib/gstreamer-1.0/` directories. PodeTelar does
not re-hash or overwrite a completed cache on subsequent launches. A separate
copy can be extracted with `FreedomStream.exe --extract C:\new\absolute-folder`.
The normal cache is `%LOCALAPPDATA%\FreedomStreamPortable\<payload-sha256>`.
Retain the notices and license terms when redistributing changed libraries.
The publisher dynamically links these libraries; original application code is MIT.

Microsoft's CRT is proprietary redistributable code, not one of these LGPL
libraries. Its provenance and redistribution terms are in `../Microsoft-CRT.json`
and `../microsoft/`. Builders must hold the applicable Visual Studio license for
redistributing those files; runtime users need no Visual Studio installation.
GPU drivers and Windows system DLLs are supplied by the user's OS/GPU vendor.
