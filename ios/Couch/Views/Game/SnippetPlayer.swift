// Plays the first N seconds of a scene, then goes dark again (a blink, 1 s,
// 3 s, 5 s, or the whole thing). With sound: famous lines are half the fun.

import AVFoundation
import Observation
import SwiftUI

@MainActor @Observable
final class Snippet {
    let player = AVPlayer()
    private(set) var showing = false
    private(set) var ready = false
    private(set) var failed = false
    private var stop: Any?
    private var end: NSObjectProtocol?
    private var status: NSKeyValueObservation?
    private var pending: Double??

    func load(_ url: URL) {
        clear()
        ready = false
        failed = false
        showing = false
        try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .moviePlayback)
        try? AVAudioSession.sharedInstance().setActive(true)
        let item = AVPlayerItem(url: url)
        status = item.observe(\.status) { [weak self] i, _ in
            let s = i.status
            Task { @MainActor in
                guard let self else { return }
                if s == .readyToPlay {
                    self.ready = true
                    if let p = self.pending { self.pending = nil; self.play(p) }
                } else if s == .failed {
                    self.failed = true
                }
            }
        }
        player.replaceCurrentItem(with: item)
        end = NotificationCenter.default.addObserver(forName: .AVPlayerItemDidPlayToEndTime, object: item, queue: .main) { [weak self] _ in
            Task { @MainActor in self?.showing = false }
        }
    }

    /// Show the first `seconds` (nil = the whole scene).
    func play(_ seconds: Double?) {
        guard ready else { pending = .some(seconds); return }
        clear()
        player.seek(to: .zero, toleranceBefore: .zero, toleranceAfter: .zero) { [weak self] _ in
            Task { @MainActor in
                guard let self else { return }
                if let s = seconds {
                    let t = NSValue(time: CMTime(seconds: s, preferredTimescale: 600))
                    self.stop = self.player.addBoundaryTimeObserver(forTimes: [t], queue: .main) { [weak self] in
                        Task { @MainActor in
                            self?.player.pause()
                            self?.showing = false
                        }
                    }
                }
                self.showing = true
                self.player.play()
            }
        }
    }

    private func clear() {
        if let s = stop { player.removeTimeObserver(s); stop = nil }
    }

    func close() {
        clear()
        player.pause()
        player.replaceCurrentItem(with: nil)
        status = nil
        if let e = end { NotificationCenter.default.removeObserver(e) }
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }
}

struct SnippetView: UIViewRepresentable {
    let player: AVPlayer
    func makeUIView(context: Context) -> V {
        let v = V()
        (v.layer as! AVPlayerLayer).player = player
        (v.layer as! AVPlayerLayer).videoGravity = .resizeAspect
        return v
    }
    func updateUIView(_ v: V, context: Context) {}
    final class V: UIView { override class var layerClass: AnyClass { AVPlayerLayer.self } }
}
