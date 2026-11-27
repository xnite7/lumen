// A muted scene clip that loops behind the artwork (couch Pro). It fades in
// over the backdrop once it's actually playing, and pauses off screen.

import AVFoundation
import SwiftUI

struct ClipBackdrop: View {
    let meta: MetaPreview
    var active = true
    @State private var url: URL?
    @State private var playing = false

    var body: some View {
        ZStack {
            Art(url: meta.backdropURL)
            if let url {
                LoopingVideo(url: url, active: active, playing: $playing)
                    .opacity(playing ? 1 : 0)
                    .animation(.easeInOut(duration: 0.8), value: playing)
            }
        }
        .task(id: meta.id) {
            playing = false
            url = nil
            // a beat on the artwork first, like Apple TV
            try? await Task.sleep(for: .seconds(1.2))
            // Settings → Accessibility → Motion → Auto-Play Video Previews off: artwork only
            guard UIAccessibility.isVideoAutoplayEnabled else { return }
            url = await Clips.shared.url(for: meta)
        }
    }
}

struct LoopingVideo: UIViewRepresentable {
    let url: URL
    var active: Bool
    @Binding var playing: Bool

    func makeUIView(context: Context) -> PlayerView {
        let v = PlayerView()
        v.onPlaying = { playing = $0 }
        v.load(url)
        return v
    }

    func updateUIView(_ v: PlayerView, context: Context) {
        if v.url != url { v.load(url) }
        active ? v.player.play() : v.player.pause()
    }

    static func dismantleUIView(_ v: PlayerView, coordinator: ()) {
        v.player.pause()
        v.player.removeAllItems()
    }

    final class PlayerView: UIView {
        override class var layerClass: AnyClass { AVPlayerLayer.self }
        let player = AVQueuePlayer()
        var looper: AVPlayerLooper?
        var url: URL?
        var onPlaying: ((Bool) -> Void)?
        private var watch: NSKeyValueObservation?

        override init(frame: CGRect) {
            super.init(frame: frame)
            let l = layer as! AVPlayerLayer
            l.player = player
            l.videoGravity = .resizeAspectFill
            player.isMuted = true
            player.preventsDisplaySleepDuringVideoPlayback = false
            // muted previews never interrupt your music
            try? AVAudioSession.sharedInstance().setCategory(.ambient, options: [.mixWithOthers])
            watch = player.observe(\.timeControlStatus) { [weak self] p, _ in
                let on = p.timeControlStatus == .playing
                DispatchQueue.main.async { self?.onPlaying?(on) }
            }
        }
        required init?(coder: NSCoder) { fatalError() }

        func load(_ u: URL) {
            url = u
            let item = AVPlayerItem(url: u)
            looper = AVPlayerLooper(player: player, templateItem: item)
            player.play()
        }
    }
}
