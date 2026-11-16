// MIT. Native macOS capture helper and Launch Services host. No authority is logged.
import AppKit
import ScreenCaptureKit
import CoreMedia
import CoreVideo
import CoreAudio
import Darwin

struct Request: Decodable {
    let action: String
    var kind: String?; var id: UInt32?; var width: Int?; var height: Int?; var fps: Int?
    var application: String?; var pid: Int32?; var created: UInt64?; var exclusions: [String]?
}
struct Item: Encodable {
    let kind: String; let id: UInt32; let label: String; let width: Int; let height: Int; let pid: Int32
}
struct Application: Encodable { let id: String; let label: String }
func birth(_ app: NSRunningApplication) -> UInt64 {
    var info = proc_bsdinfo()
    let count = proc_pidinfo(app.processIdentifier, PROC_PIDTBSDINFO, 0, &info, Int32(MemoryLayout<proc_bsdinfo>.size))
    guard count == MemoryLayout<proc_bsdinfo>.size, info.pbi_start_tvsec > 0 else { return 0 }
    return info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec
}
func emit<T: Encodable>(_ value: T) throws { try FileHandle.standardOutput.write(contentsOf: JSONEncoder().encode(value)) }
func fail(line: UInt = #line) -> Never {
    FileHandle.standardError.write(Data("macOS capture stopped at source line \(line)\n".utf8)); exit(1)
}

final class Capture: NSObject, SCStreamOutput, SCStreamDelegate {
    var stream: SCStream?
    var audioProcesses: [Int32] = []
    let audio: Bool
    init(audio: Bool) { self.audio = audio }
    func stream(_ stream: SCStream, didStopWithError error: Error) {
        FileHandle.standardError.write(Data("macOS stream stopped with code \((error as NSError).code)\n".utf8)); fail()
    }
    func stream(_ stream: SCStream, didOutputSampleBuffer sample: CMSampleBuffer, of type: SCStreamOutputType) {
        guard sample.isValid else { return }
        var data = Data()
        if type == .screen && !audio {
            guard let attachments = CMSampleBufferGetSampleAttachmentsArray(sample, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
                  let status = attachments.first?[.status] as? Int,
                  status == SCFrameStatus.complete.rawValue,
                  let image = sample.imageBuffer else { return }
            CVPixelBufferLockBaseAddress(image, .readOnly)
            defer { CVPixelBufferUnlockBaseAddress(image, .readOnly) }
            guard CVPixelBufferGetPixelFormatType(image) == kCVPixelFormatType_32BGRA,
                  let base = CVPixelBufferGetBaseAddress(image) else { fail() }
            let width = CVPixelBufferGetWidth(image), height = CVPixelBufferGetHeight(image)
            let stride = CVPixelBufferGetBytesPerRow(image)
            for row in 0..<height { data.append(base.advanced(by: row * stride).assumingMemoryBound(to: UInt8.self), count: width * 4) }
        } else if type == .audio && audio {
            // ScreenCaptureKit supplies 48 kHz planar Float32. Interleave S16 stereo
            // for the existing transport. Never open a microphone or input device.
            guard let description = sample.formatDescription,
                  let format = CMAudioFormatDescriptionGetStreamBasicDescription(description)?.pointee,
                  format.mSampleRate == 48000, format.mChannelsPerFrame == 2,
                  format.mFormatID == kAudioFormatLinearPCM,
                  format.mFormatFlags & kAudioFormatFlagIsFloat != 0,
                  format.mBitsPerChannel == 32 else { fail() }
            var needed = 0
            var block: CMBlockBuffer?
            CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(sample, bufferListSizeNeededOut: &needed,
                bufferListOut: nil, bufferListSize: 0, blockBufferAllocator: nil, blockBufferMemoryAllocator: nil,
                flags: 0, blockBufferOut: &block)
            let storage = UnsafeMutableRawPointer.allocate(byteCount: needed, alignment: 16)
            defer { storage.deallocate() }
            let list = storage.assumingMemoryBound(to: AudioBufferList.self)
            guard CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(sample, bufferListSizeNeededOut: nil,
                bufferListOut: list, bufferListSize: needed, blockBufferAllocator: nil, blockBufferMemoryAllocator: nil,
                flags: 0, blockBufferOut: &block) == noErr else { fail() }
            let buffers = UnsafeMutableAudioBufferListPointer(list)
            let frames = CMSampleBufferGetNumSamples(sample)
            let planar = format.mFormatFlags & kAudioFormatFlagIsNonInterleaved != 0
            guard frames > 0 && frames <= 48000 && buffers.count == (planar ? 2 : 1) else { fail() }
            for frame in 0..<frames {
                for channel in 0..<2 {
                    let buffer = buffers[planar ? channel : 0]
                    guard let pointer = buffer.mData?.assumingMemoryBound(to: Float.self) else { fail() }
                    let index = planar ? frame : frame * 2 + channel
                    guard (index + 1) * 4 <= buffer.mDataByteSize else { fail() }
                    let sample = pointer[index]
                    var value = Int16(max(-32768, min(32767, (sample.isFinite ? sample : 0) * 32767))).littleEndian
                    withUnsafeBytes(of: &value) { data.append(contentsOf: $0) }
                }
            }
        } else { return }
        guard data.count <= 3840 * 2160 * 4 else { fail() }
        var size = UInt32(data.count).littleEndian
        do {
            try withUnsafeBytes(of: &size) { try FileHandle.standardOutput.write(contentsOf: Data($0)) }
            try FileHandle.standardOutput.write(contentsOf: data)
        } catch { fail() }
    }
    @MainActor func audioFilter(_ request: Request, _ content: SCShareableContent) -> (SCContentFilter, [Int32]) {
        guard let display = content.displays.first else { fail() }
        let protected = content.applications.filter { app in
            app.processID == ProcessInfo.processInfo.processIdentifier || app.processID == getppid() ||
            app.bundleIdentifier.hasPrefix("app.podetelar.") ||
            app.bundleIdentifier.lowercased().contains("discord") ||
            (request.exclusions ?? []).contains(app.bundleIdentifier)
        }
        let chosen: [SCRunningApplication]
        if let id = request.application {
            chosen = content.applications.filter { $0.bundleIdentifier == id && !protected.contains($0) }
        } else if let pid = request.pid, let created = request.created {
            guard let app = NSWorkspace.shared.runningApplications.first(where: { $0.processIdentifier == pid && birth($0) == created }),
                  let selected = content.applications.first(where: { $0.processID == pid }), !protected.contains(selected),
                  app.bundleIdentifier == selected.bundleIdentifier else { fail() }
            chosen = [selected]
        } else {
            // Positive membership prevents newly launched excluded applications
            // from leaking audio before the next inventory refresh.
            chosen = content.applications.filter { !protected.contains($0) }
        }
        return (SCContentFilter(display: display, including: chosen, exceptingWindows: []), chosen.map(\.processID).sorted())
    }
    @MainActor func refreshAudio(_ request: Request) async throws {
        guard audio, let stream else { return }
        let content = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
        let (filter, processes) = audioFilter(request, content)
        if processes != audioProcesses {
            try await stream.updateContentFilter(filter)
            audioProcesses = processes
        }
    }
    @MainActor func start(_ request: Request) async throws {
        let content = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
        let filter: SCContentFilter
        if audio {
            (filter, audioProcesses) = audioFilter(request, content)
        } else if request.kind == "window" {
            guard let window = content.windows.first(where: { $0.windowID == request.id }) else { fail() }
            filter = SCContentFilter(desktopIndependentWindow: window)
        } else {
            guard let display = content.displays.first(where: { $0.displayID == request.id }) else { fail() }
            filter = SCContentFilter(display: display, excludingWindows: [])
        }
        let config = SCStreamConfiguration()
        config.width = audio ? 16 : max(2, min(3840, request.width ?? 1920))
        config.height = audio ? 16 : max(2, min(2160, request.height ?? 1080))
        config.minimumFrameInterval = CMTime(value: 1, timescale: CMTimeScale(audio ? 1 : max(1, min(120, request.fps ?? 60))))
        config.pixelFormat = kCVPixelFormatType_32BGRA
        config.queueDepth = 3
        config.showsCursor = !audio
        config.capturesAudio = audio
        config.excludesCurrentProcessAudio = true
        config.sampleRate = 48000
        config.channelCount = 2
        let capture = SCStream(filter: filter, configuration: config, delegate: self)
        try capture.addStreamOutput(self, type: audio ? .audio : .screen, sampleHandlerQueue: DispatchQueue(label: "app.podetelar.capture"))
        stream = capture
        try await capture.startCapture()
    }
}

@MainActor func command(_ action: String) async throws {
    if action == "output" {
        var address = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyDefaultOutputDevice, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        var device = AudioDeviceID(0), size = UInt32(MemoryLayout<AudioDeviceID>.size)
        guard AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, &device) == noErr else { fail() }
        if device == 0 { try emit(""); return }
        address.mSelector = kAudioDevicePropertyDeviceUID
        var uid: Unmanaged<CFString>?
        size = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
        guard AudioObjectGetPropertyData(device, &address, 0, nil, &size, &uid) == noErr, let uid else { fail() }
        try emit(uid.takeUnretainedValue() as String)
        return
    }
    let request = try JSONDecoder().decode(Request.self, from: FileHandle.standardInput.readDataToEndOfFile())
    if action == "enumerate" || action == "applications" {
        let content = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
        if action == "applications" {
            let apps = content.applications.filter { !$0.bundleIdentifier.isEmpty && !$0.bundleIdentifier.lowercased().contains("discord") && !$0.bundleIdentifier.hasPrefix("app.podetelar.") }
            var seen = Set<String>()
            try emit(apps.filter { seen.insert($0.bundleIdentifier).inserted }.map { Application(id: $0.bundleIdentifier, label: $0.applicationName) })
        } else {
            var items = content.displays.map { display in
                let filter = SCContentFilter(display: display, excludingWindows: [])
                return Item(kind: "monitor", id: display.displayID, label: "Display \(display.displayID)", width: Int(ceil(filter.contentRect.width * CGFloat(filter.pointPixelScale))), height: Int(ceil(filter.contentRect.height * CGFloat(filter.pointPixelScale))), pid: 0)
            }
            items += content.windows.filter { $0.windowLayer == 0 && $0.owningApplication != nil && $0.frame.width > 1 && $0.frame.height > 1 && !($0.owningApplication?.bundleIdentifier.hasPrefix("app.podetelar.") ?? false) }.map {
                let filter = SCContentFilter(desktopIndependentWindow: $0)
                return Item(kind: "window", id: $0.windowID, label: ($0.owningApplication?.applicationName ?? "Window") + " — " + ($0.title ?? ""), width: Int(ceil(filter.contentRect.width * CGFloat(filter.pointPixelScale))), height: Int(ceil(filter.contentRect.height * CGFloat(filter.pointPixelScale))), pid: $0.owningApplication?.processID ?? 0)
            }
            try emit(items)
        }
    } else if action == "identity" {
        guard let app = NSRunningApplication(processIdentifier: Int32(request.id ?? 0)) else { fail() }
        try emit(birth(app))
    } else {
        let capture = Capture(audio: action == "audio")
        try await capture.start(request)
        while getppid() != 1 {
            try await Task.sleep(nanoseconds: 1_000_000_000)
            try await capture.refreshAudio(request)
            withExtendedLifetime(capture) {}
        }
    }
}

// The app host receives Launch Services URL events, leaving invitations out of
// Velopack startup logging. Secondary bootstraps forward through authenticated IPC.
final class Host: NSObject, NSApplicationDelegate {
    var child: Process?
    var pending: URL?
    func application(_ application: NSApplication, open urls: [URL]) {
        guard urls.count == 1, let url = urls.first, url.scheme == "freedomstream", url.absoluteString.utf8.count <= 4096 else { return }
        if child == nil { pending = url } else { launch(url, primary: false) }
    }
    func applicationDidFinishLaunching(_ notification: Notification) {
        DispatchQueue.main.async { self.launch(self.pending, primary: true) }
    }
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        // Closing the media UI owns graceful shutdown. Do not kill it mid-capture.
        if child != nil { return .terminateCancel }
        return .terminateNow
    }
    func launch(_ link: URL?, primary: Bool) {
        let process = Process()
        process.executableURL = Bundle.main.executableURL!.deletingLastPathComponent().appendingPathComponent("FreedomStreamInstalled")
        process.arguments = link.map { ["--browser-link", $0.absoluteString] } ?? []
        process.standardOutput = FileHandle.nullDevice; process.standardError = FileHandle.nullDevice
        if primary {
            child = process
            process.terminationHandler = { _ in DispatchQueue.main.async { self.child = nil; NSApplication.shared.terminate(nil) } }
        }
        do { try process.run() } catch { if primary { child = nil; NSApplication.shared.terminate(nil) } }
    }
}
if CommandLine.arguments.count > 1 {
    let action = CommandLine.arguments[1]
    let app = NSApplication.shared
    app.setActivationPolicy(.prohibited)
    Task { @MainActor in do { try await command(action); exit(0) } catch {
        let code = (error as NSError).code
        FileHandle.standardError.write(Data("macOS capture error code: \(code)\n".utf8)); fail()
    } }
    app.run()
} else {
    let app = NSApplication.shared
    let host = Host()
    app.setActivationPolicy(.accessory)
    app.delegate = host
    app.run()
}
