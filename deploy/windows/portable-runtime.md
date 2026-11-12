# Bundled runtime

Original PodeTelar code is MIT licensed. Dependency notices remain under
their respective licenses; this file does not relicense any library or codec.

The media payload is an explicit allowlist of the official GStreamer 1.28.8
Windows runtime's shared libraries and LGPL plugins. It includes native D3D11
capture/conversion, NVIDIA NVENC and AMD AMF wrappers, video parsers, preview JPEG,
audio conversion/mixing/resampling and WASAPI process loopback. Plugin licenses,
file hashes and PE import closure are checked when packaging. GPL codec plugins
such as x264, x265 and libav, network/protocol plugins and unrelated frameworks
are not bundled. Hardware-driver DLLs remain provided by the installed GPU driver.

GStreamer/GLib/proxy-libintl license texts, additional upstream source notices and
complete corresponding library sources/build recipes are included in this folder.
See `sources/README.md`. Shared libraries remain separately replaceable.
Microsoft's unmodified release CRT is bundled in a private `crt` directory, under
the installed Visual Studio toolchain's redistribution terms. A complete installed
CRT trio at least as new is preferred, because an older app-local CRT can break
newer Windows audio/COM components; otherwise the launcher selects the bundled
directory for itself and its children. It is not installed
system-wide. Its actual version and hashes are recorded in `Microsoft-CRT.json`.

Normal launch performs no dependency downloads, invokes no installer or PowerShell,
changes no registry/system PATH settings and requests no elevation. It creates a
versioned private runtime cache. Room admission and streaming naturally use the
network after you choose to join. Native HEVC/AMD/4K120 qualification limits still
apply; distributing a codec wrapper does not establish codec patent rights.
