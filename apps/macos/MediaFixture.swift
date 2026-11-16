// MIT. Explicit local audio/window qualification fixture; never packaged.
import AppKit
import AVFoundation
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let window = NSWindow(contentRect: NSRect(x:80,y:80,width:640,height:360), styleMask:[.titled,.closable], backing:.buffered, defer:false)
window.title = "PodeTelar qualification"
let view = NSView(frame:NSRect(x:0,y:0,width:640,height:360))
view.wantsLayer = true
view.layer?.backgroundColor = NSColor.systemBlue.cgColor
window.contentView = view
window.orderFrontRegardless()
var light = false
let timer = Timer.scheduledTimer(withTimeInterval:0.25,repeats:true) { _ in
    light.toggle(); view.layer?.backgroundColor = (light ? NSColor.white : NSColor.systemBlue).cgColor
}
let engine = AVAudioEngine()
let format = AVAudioFormat(standardFormatWithSampleRate:48000,channels:2)!
var clock = 0.0
let source = AVAudioSourceNode(format:format) { _,_,count,list in
    let buffers = UnsafeMutableAudioBufferListPointer(list)
    for frame in 0..<Int(count) {
        for channel in 0..<min(2,buffers.count) {
            buffers[channel].mData?.assumingMemoryBound(to:Float.self)[frame] = Float(sin(clock * 2 * .pi * (channel == 0 ? 440 : 880) / 48000) * 0.005)
        }
        clock += 1
    }
    return noErr
}
engine.attach(source)
engine.connect(source,to:engine.mainMixerNode,format:format)
do { try engine.start() } catch { exit(1) }
FileHandle.standardOutput.write(Data("ready\n".utf8))
app.run()
