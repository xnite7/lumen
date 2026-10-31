// Lumen's own video player on iPhone: mpv (via MPVKit) rendering through
// Metal, with Lumen's controls in Liquid Glass. Plays what Apple's player
// can't: MKV, HEVC/AV1 in any container, Dolby Vision, styled (ASS)
// subtitles, multiple audio tracks.
//
// It lives in the app target (not the Tauri plugin) because MPVKit ships as
// Swift-package binaries Xcode links. The plugin finds it by its Objective-C
// name, hands it a config dictionary, and listens for these notifications:
//   lumen.player.progress  { pos, duration, paused }
//   lumen.player.ended     {}
//   lumen.player.closed    { pos, duration }
//   lumen.player.failed    { message }

import AVFoundation
import Libmpv
import MediaPlayer
import UIKit

private enum Note {
    static let progress = Notification.Name("lumen.player.progress")
    static let ended = Notification.Name("lumen.player.ended")
    static let closed = Notification.Name("lumen.player.closed")
    static let failed = Notification.Name("lumen.player.failed")
}

/// CAMetalLayer with MoltenVK's 1x1 drawable workaround (mpv PR 13651) and
/// EDR activation on the main thread for HDR.
private final class MPVMetalLayer: CAMetalLayer {
    override var drawableSize: CGSize {
        get { super.drawableSize }
        set {
            if Int(newValue.width) > 1 && Int(newValue.height) > 1 { super.drawableSize = newValue }
        }
    }

    override var wantsExtendedDynamicRangeContent: Bool {
        get { super.wantsExtendedDynamicRangeContent }
        set {
            if Thread.isMainThread {
                super.wantsExtendedDynamicRangeContent = newValue
            } else {
                DispatchQueue.main.sync { super.wantsExtendedDynamicRangeContent = newValue }
            }
        }
    }
}

private struct Track {
    let id: Int
    let type: String
    let title: String
    let selected: Bool
}

@objc(LumenMPVPlayer)
final class LumenMPVPlayer: UIViewController {
    /// url, title, subtitle, artwork, startAt, headers, subLangs, audioLangs
    @objc var config: NSDictionary = [:]

    private var metalLayer = MPVMetalLayer()
    private var mpv: OpaquePointer?
    private let queue = DispatchQueue(label: "lumen.mpv", qos: .userInitiated)

    // state (main thread)
    private var pos: Double = 0
    private var duration: Double = 0
    private var paused = false
    private var buffering = true
    private var lastReport: Double = -10
    private var closing = false
    private var scrubbing = false
    private var fill = false
    private var chromeVisible = true
    private var hideTimer: Timer?

    // UI
    private let chrome = UIView()
    private let topBar = UIStackView()
    private let titleLabel = UILabel()
    private let subtitleLabel = UILabel()
    private let closeButton = UIButton(type: .system)
    private let playButton = UIButton(type: .system)
    private let backButton = UIButton(type: .system)
    private let fwdButton = UIButton(type: .system)
    private let slider = UISlider()
    private let timeLabel = UILabel()
    private let remainLabel = UILabel()
    private let subsButton = UIButton(type: .system)
    private let audioButton = UIButton(type: .system)
    private let speedButton = UIButton(type: .system)
    private let fillButton = UIButton(type: .system)
    private let spinner = UIActivityIndicatorView(style: .large)
    private let errorLabel = UILabel()
    private let bottomPanel = UIView()

    // MARK: lifecycle

    override var prefersStatusBarHidden: Bool { !chromeVisible }
    override var prefersHomeIndicatorAutoHidden: Bool { true }
    override var supportedInterfaceOrientations: UIInterfaceOrientationMask { .allButUpsideDown }
    override var preferredInterfaceOrientationForPresentation: UIInterfaceOrientation { .landscapeRight }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        metalLayer.frame = view.bounds
        metalLayer.contentsScale = UIScreen.main.nativeScale
        metalLayer.framebufferOnly = true
        metalLayer.backgroundColor = UIColor.black.cgColor
        view.layer.addSublayer(metalLayer)

        buildUI()
        startAudioSession()
        setupMpv()
        setupRemoteCommands()
        loadArtwork()
        if let url = config["url"] as? String { load(url) }
        scheduleHide()

        NotificationCenter.default.addObserver(self, selector: #selector(didEnterBackground), name: UIApplication.didEnterBackgroundNotification, object: nil)
        NotificationCenter.default.addObserver(self, selector: #selector(willEnterForeground), name: UIApplication.willEnterForegroundNotification, object: nil)
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        rotate(to: .landscape)
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        metalLayer.frame = view.bounds
        CATransaction.commit()
    }

    deinit {
        NotificationCenter.default.removeObserver(self)
    }

    private func rotate(to mask: UIInterfaceOrientationMask) {
        if #available(iOS 16.0, *) {
            setNeedsUpdateOfSupportedInterfaceOrientations()
            view.window?.windowScene?.requestGeometryUpdate(.iOS(interfaceOrientations: mask)) { _ in }
        }
    }

    // MARK: mpv

    private func setupMpv() {
        guard let ctx = mpv_create() else {
            fail("The player couldn't start.")
            return
        }
        mpv = ctx
        mpv_request_log_messages(ctx, "no")
        mpv_set_option(ctx, "wid", MPV_FORMAT_INT64, &metalLayer)
        set("vo", "gpu-next")
        set("gpu-api", "vulkan")
        set("gpu-context", "moltenvk")
        set("hwdec", "videotoolbox")
        set("video-rotate", "no")
        set("keep-open", "yes")
        set("ytdl", "no")
        set("cache", "yes")
        set("demuxer-max-bytes", "300MiB")
        set("demuxer-readahead-secs", "90")
        set("network-timeout", "30")
        set("target-colorspace-hint", "yes")
        set("sub-auto", "fuzzy")
        set("subs-fallback", "yes")
        set("sub-font-size", "44")
        set("sub-border-size", "2.4")
        set("sub-shadow-offset", "1")
        set("sub-margin-y", "40")
        set("user-agent", "Lumen/1.0")
        if let s = config["subLangs"] as? [String], !s.isEmpty { set("slang", s.joined(separator: ",")) }
        if let a = config["audioLangs"] as? [String], !a.isEmpty { set("alang", a.joined(separator: ",")) }
        if let h = config["headers"] as? [String: String], !h.isEmpty {
            set("http-header-fields", h.map { "\($0.key): \($0.value.replacingOccurrences(of: ",", with: "\\,"))" }.joined(separator: ","))
        }
        guard mpv_initialize(ctx) >= 0 else {
            fail("The player couldn't start.")
            return
        }
        mpv_observe_property(ctx, 1, "time-pos", MPV_FORMAT_DOUBLE)
        mpv_observe_property(ctx, 2, "duration", MPV_FORMAT_DOUBLE)
        mpv_observe_property(ctx, 3, "pause", MPV_FORMAT_FLAG)
        mpv_observe_property(ctx, 4, "paused-for-cache", MPV_FORMAT_FLAG)
        mpv_observe_property(ctx, 5, "eof-reached", MPV_FORMAT_FLAG)
        mpv_observe_property(ctx, 6, "core-idle", MPV_FORMAT_FLAG)
        mpv_set_wakeup_callback(ctx, { raw in
            guard let raw = raw else { return }
            let me = Unmanaged<LumenMPVPlayer>.fromOpaque(raw).takeUnretainedValue()
            me.drain()
        }, Unmanaged.passUnretained(self).toOpaque())
    }

    private func set(_ name: String, _ value: String) {
        guard let ctx = mpv else { return }
        mpv_set_option_string(ctx, name, value)
    }

    private func command(_ args: [String]) {
        guard let ctx = mpv else { return }
        var cargs: [UnsafePointer<CChar>?] = args.map { UnsafePointer(strdup($0)) }
        cargs.append(nil)
        defer { for p in cargs where p != nil { free(UnsafeMutablePointer(mutating: p!)) } }
        mpv_command(ctx, &cargs)
    }

    private func setFlag(_ name: String, _ on: Bool) {
        guard let ctx = mpv else { return }
        var v: Int32 = on ? 1 : 0
        mpv_set_property(ctx, name, MPV_FORMAT_FLAG, &v)
    }

    private func getString(_ name: String) -> String? {
        guard let ctx = mpv, let c = mpv_get_property_string(ctx, name) else { return nil }
        defer { mpv_free(c) }
        return String(cString: c)
    }

    private func getInt(_ name: String) -> Int {
        guard let ctx = mpv else { return 0 }
        var v: Int64 = 0
        mpv_get_property(ctx, name, MPV_FORMAT_INT64, &v)
        return Int(v)
    }

    private func load(_ url: String) {
        var opts: [String] = []
        if let s = config["startAt"] as? Double, s > 1 { opts.append("start=\(Int(s))") }
        // loadfile <url> replace <index> <options>  (mpv >= 0.38)
        command(opts.isEmpty ? ["loadfile", url, "replace"] : ["loadfile", url, "replace", "-1", opts.joined(separator: ",")])
    }

    /// Runs on mpv's wakeup thread; read everything queued.
    private func drain() {
        queue.async { [weak self] in
            guard let self = self else { return }
            while let ctx = self.mpv {
                guard let ev = mpv_wait_event(ctx, 0) else { break }
                let id = ev.pointee.event_id
                if id == MPV_EVENT_NONE { break }
                switch id {
                case MPV_EVENT_PROPERTY_CHANGE:
                    guard let data = ev.pointee.data else { continue }
                    let prop = data.assumingMemoryBound(to: mpv_event_property.self).pointee
                    let name = String(cString: prop.name)
                    var dbl: Double?
                    var flag: Bool?
                    if prop.format == MPV_FORMAT_DOUBLE, let p = prop.data { dbl = p.assumingMemoryBound(to: Double.self).pointee }
                    if prop.format == MPV_FORMAT_FLAG, let p = prop.data { flag = p.assumingMemoryBound(to: Int32.self).pointee != 0 }
                    DispatchQueue.main.async { self.property(name, dbl, flag) }
                case MPV_EVENT_END_FILE:
                    guard let data = ev.pointee.data else { continue }
                    let end = data.assumingMemoryBound(to: mpv_event_end_file.self).pointee
                    if end.reason == MPV_END_FILE_REASON_ERROR {
                        let msg = String(cString: mpv_error_string(end.error))
                        DispatchQueue.main.async { self.fail("This stream wouldn't play (\(msg)). Try another stream.") }
                    }
                case MPV_EVENT_FILE_LOADED:
                    DispatchQueue.main.async {
                        self.buffering = false
                        self.updateSpinner()
                        self.rebuildMenus()
                    }
                case MPV_EVENT_SHUTDOWN:
                    return
                default:
                    break
                }
            }
        }
    }

    private func property(_ name: String, _ d: Double?, _ f: Bool?) {
        switch name {
        case "time-pos":
            pos = d ?? pos
            if !scrubbing, duration > 0 { slider.value = Float(pos / duration) }
            updateTimes()
            report(force: false)
        case "duration":
            duration = d ?? duration
            updateTimes()
            updateNowPlaying()
        case "pause":
            paused = f ?? paused
            updatePlayButton()
            updateNowPlaying()
            report(force: true)
        case "paused-for-cache":
            buffering = f ?? false
            updateSpinner()
        case "eof-reached":
            if f == true {
                report(force: true)
                NotificationCenter.default.post(name: Note.ended, object: nil)
            }
        default:
            break
        }
    }

    private func report(force: Bool) {
        if !force && abs(pos - lastReport) < 5 { return }
        lastReport = pos
        NotificationCenter.default.post(name: Note.progress, object: nil, userInfo: ["pos": pos, "duration": duration, "paused": paused])
        if force { updateNowPlaying() }
    }

    private func fail(_ message: String) {
        buffering = false
        updateSpinner()
        errorLabel.text = message
        errorLabel.isHidden = false
        showChrome()
        NotificationCenter.default.post(name: Note.failed, object: nil, userInfo: ["message": message])
    }

    // MARK: actions

    @objc private func togglePause() {
        haptic(.light)
        setFlag("pause", !paused)
        scheduleHide()
    }

    @objc private func skipBack() {
        haptic(.light)
        command(["seek", "-10", "relative"])
        scheduleHide()
    }

    @objc private func skipForward() {
        haptic(.light)
        command(["seek", "10", "relative"])
        scheduleHide()
    }

    @objc private func close() {
        guard !closing else { return }
        closing = true
        haptic(.light)
        let data: [String: Any] = ["pos": pos, "duration": duration]
        teardown()
        rotate(to: .portrait)
        dismiss(animated: true) {
            NotificationCenter.default.post(name: Note.closed, object: nil, userInfo: data)
        }
    }

    @objc private func toggleFill() {
        haptic(.light)
        fill.toggle()
        set("panscan", fill ? "1.0" : "0.0")
        command(["set", "panscan", fill ? "1.0" : "0.0"])
        fillButton.setImage(UIImage(systemName: fill ? "arrow.down.right.and.arrow.up.left" : "arrow.up.left.and.arrow.down.right"), for: .normal)
        scheduleHide()
    }

    @objc private func sliderBegan() {
        scrubbing = true
        hideTimer?.invalidate()
    }

    @objc private func sliderMoved() {
        guard duration > 0 else { return }
        let t = Double(slider.value) * duration
        timeLabel.text = Self.fmt(t)
        remainLabel.text = "-" + Self.fmt(duration - t)
    }

    @objc private func sliderEnded() {
        scrubbing = false
        guard duration > 0 else { return }
        command(["seek", String(Double(slider.value) * duration), "absolute"])
        haptic(.light)
        scheduleHide()
    }

    @objc private func tapped(_ g: UITapGestureRecognizer) {
        chromeVisible ? hideChrome() : showChrome()
    }

    @objc private func doubleTapped(_ g: UITapGestureRecognizer) {
        let x = g.location(in: view).x
        if x < view.bounds.width / 3 { skipBack() } else if x > view.bounds.width * 2 / 3 { skipForward() } else { togglePause() }
    }

    @objc private func pinched(_ g: UIPinchGestureRecognizer) {
        guard g.state == .ended else { return }
        if (g.scale > 1.1 && !fill) || (g.scale < 0.9 && fill) { toggleFill() }
    }

    @objc private func swipedDown() {
        close()
    }

    private func teardown() {
        hideTimer?.invalidate()
        UIApplication.shared.isIdleTimerDisabled = false
        MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
        let rc = MPRemoteCommandCenter.shared()
        [rc.playCommand, rc.pauseCommand, rc.togglePlayPauseCommand, rc.skipForwardCommand, rc.skipBackwardCommand, rc.changePlaybackPositionCommand].forEach { $0.removeTarget(nil) }
        guard let ctx = mpv else { return }
        mpv = nil
        mpv_set_wakeup_callback(ctx, nil, nil)
        queue.async {
            mpv_terminate_destroy(ctx)
        }
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }

    // Audio keeps playing with the screen off or the app in the background;
    // the picture comes back when you return.
    @objc private func didEnterBackground() {
        set("vid", "no")
        command(["set", "vid", "no"])
    }

    @objc private func willEnterForeground() {
        command(["set", "vid", "auto"])
    }

    // MARK: menus (native UIMenu)

    private func tracks() -> [Track] {
        let n = getInt("track-list/count")
        var out: [Track] = []
        for i in 0..<n {
            let type = getString("track-list/\(i)/type") ?? ""
            let id = getInt("track-list/\(i)/id")
            let lang = getString("track-list/\(i)/lang") ?? ""
            let title = getString("track-list/\(i)/title") ?? ""
            let codec = getString("track-list/\(i)/codec") ?? ""
            let sel = getString("track-list/\(i)/selected") == "yes"
            let name = [Self.language(lang), title].filter { !$0.isEmpty }.joined(separator: " · ")
            out.append(Track(id: id, type: type, title: name.isEmpty ? (codec.isEmpty ? "Track \(id)" : codec.uppercased()) : name, selected: sel))
        }
        return out
    }

    private func rebuildMenus() {
        subsButton.menu = UIMenu(title: "Subtitles", children: [UIDeferredMenuElement.uncached { [weak self] done in
            guard let self = self else { return done([]) }
            let subs = self.tracks().filter { $0.type == "sub" }
            var items: [UIMenuElement] = [UIAction(title: "Off", state: subs.contains { $0.selected } ? .off : .on) { [weak self] _ in self?.command(["set", "sid", "no"]) }]
            items += subs.map { t in UIAction(title: t.title, state: t.selected ? .on : .off) { [weak self] _ in self?.command(["set", "sid", String(t.id)]) } }
            let delay = UIMenu(title: "Timing", options: .displayInline, children: [
                UIAction(title: "Earlier 0.1 s", image: UIImage(systemName: "minus")) { [weak self] _ in self?.command(["add", "sub-delay", "-0.1"]) },
                UIAction(title: "Later 0.1 s", image: UIImage(systemName: "plus")) { [weak self] _ in self?.command(["add", "sub-delay", "0.1"]) },
            ])
            done(items + [delay])
        }])
        audioButton.menu = UIMenu(title: "Audio", children: [UIDeferredMenuElement.uncached { [weak self] done in
            guard let self = self else { return done([]) }
            let audio = self.tracks().filter { $0.type == "audio" }
            done(audio.map { t in UIAction(title: t.title, state: t.selected ? .on : .off) { [weak self] _ in self?.command(["set", "aid", String(t.id)]) } })
        }])
        speedButton.menu = UIMenu(title: "Speed", children: [UIDeferredMenuElement.uncached { [weak self] done in
            guard let self = self else { return done([]) }
            let cur = Double(self.getString("speed") ?? "1") ?? 1
            done([0.75, 1.0, 1.25, 1.5, 2.0].map { s in
                UIAction(title: s == 1 ? "Normal" : "\(s)×", state: abs(cur - s) < 0.01 ? .on : .off) { [weak self] _ in
                    self?.command(["set", "speed", String(s)])
                    self?.speedButton.setTitle(s == 1 ? nil : "\(s)×", for: .normal)
                }
            })
        }])
        [subsButton, audioButton, speedButton].forEach { $0.showsMenuAsPrimaryAction = true }
    }

    // MARK: Now Playing / remote control

    private func startAudioSession() {
        let s = AVAudioSession.sharedInstance()
        try? s.setCategory(.playback, mode: .moviePlayback)
        try? s.setActive(true)
        UIApplication.shared.isIdleTimerDisabled = true
    }

    private func setupRemoteCommands() {
        let rc = MPRemoteCommandCenter.shared()
        rc.playCommand.addTarget { [weak self] _ in self?.setFlag("pause", false); return .success }
        rc.pauseCommand.addTarget { [weak self] _ in self?.setFlag("pause", true); return .success }
        rc.togglePlayPauseCommand.addTarget { [weak self] _ in self?.togglePause(); return .success }
        rc.skipForwardCommand.preferredIntervals = [10]
        rc.skipBackwardCommand.preferredIntervals = [10]
        rc.skipForwardCommand.addTarget { [weak self] _ in self?.command(["seek", "10", "relative"]); return .success }
        rc.skipBackwardCommand.addTarget { [weak self] _ in self?.command(["seek", "-10", "relative"]); return .success }
        rc.changePlaybackPositionCommand.addTarget { [weak self] e in
            guard let e = e as? MPChangePlaybackPositionCommandEvent else { return .commandFailed }
            self?.command(["seek", String(e.positionTime), "absolute"])
            return .success
        }
    }

    private var artwork: MPMediaItemArtwork?

    private func loadArtwork() {
        guard let s = config["artwork"] as? String, let url = URL(string: s), url.scheme?.hasPrefix("http") == true else { return }
        URLSession.shared.dataTask(with: url) { [weak self] data, _, _ in
            guard let data = data, let img = UIImage(data: data) else { return }
            DispatchQueue.main.async {
                self?.artwork = MPMediaItemArtwork(boundsSize: img.size) { _ in img }
                self?.updateNowPlaying()
            }
        }.resume()
    }

    private func updateNowPlaying() {
        var info: [String: Any] = [
            MPMediaItemPropertyTitle: config["title"] as? String ?? "Lumen",
            MPNowPlayingInfoPropertyElapsedPlaybackTime: pos,
            MPNowPlayingInfoPropertyPlaybackRate: paused ? 0.0 : 1.0,
            MPNowPlayingInfoPropertyMediaType: MPNowPlayingInfoMediaType.video.rawValue,
        ]
        if let s = config["subtitle"] as? String, !s.isEmpty { info[MPMediaItemPropertyArtist] = s }
        if duration > 0 { info[MPMediaItemPropertyPlaybackDuration] = duration }
        if let a = artwork { info[MPMediaItemPropertyArtwork] = a }
        MPNowPlayingInfoCenter.default().nowPlayingInfo = info
    }

    // MARK: UI

    private func glass(_ radius: CGFloat) -> UIVisualEffectView {
        let v: UIVisualEffectView
        if #available(iOS 26.0, *) {
            let g = UIGlassEffect()
            g.isInteractive = true
            v = UIVisualEffectView(effect: g)
        } else {
            v = UIVisualEffectView(effect: UIBlurEffect(style: .systemUltraThinMaterialDark))
        }
        v.layer.cornerRadius = radius
        v.layer.cornerCurve = .continuous
        v.clipsToBounds = true
        v.overrideUserInterfaceStyle = .dark
        return v
    }

    private func roundButton(_ b: UIButton, _ symbol: String, size: CGFloat, point: CGFloat, action: Selector?) -> UIView {
        let g = glass(size / 2)
        g.translatesAutoresizingMaskIntoConstraints = false
        b.translatesAutoresizingMaskIntoConstraints = false
        b.setImage(UIImage(systemName: symbol, withConfiguration: UIImage.SymbolConfiguration(pointSize: point, weight: .semibold)), for: .normal)
        b.tintColor = .white
        if let a = action { b.addTarget(self, action: a, for: .touchUpInside) }
        g.contentView.addSubview(b)
        NSLayoutConstraint.activate([
            g.widthAnchor.constraint(equalToConstant: size), g.heightAnchor.constraint(equalToConstant: size),
            b.leadingAnchor.constraint(equalTo: g.contentView.leadingAnchor), b.trailingAnchor.constraint(equalTo: g.contentView.trailingAnchor),
            b.topAnchor.constraint(equalTo: g.contentView.topAnchor), b.bottomAnchor.constraint(equalTo: g.contentView.bottomAnchor),
        ])
        return g
    }

    private func buildUI() {
        chrome.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(chrome)
        // soft shade top and bottom so white controls read on any frame
        let shade = CAGradientLayer()
        shade.colors = [UIColor.black.withAlphaComponent(0.55).cgColor, UIColor.clear.cgColor, UIColor.clear.cgColor, UIColor.black.withAlphaComponent(0.6).cgColor]
        shade.locations = [0, 0.25, 0.65, 1]
        let shadeView = GradientView(gradient: shade)
        shadeView.translatesAutoresizingMaskIntoConstraints = false
        shadeView.isUserInteractionEnabled = false
        chrome.addSubview(shadeView)

        // top: close + titles
        let closeView = roundButton(closeButton, "xmark", size: 44, point: 16, action: #selector(LumenMPVPlayer.close))
        titleLabel.text = config["title"] as? String
        titleLabel.font = .systemFont(ofSize: 17, weight: .semibold)
        titleLabel.textColor = .white
        subtitleLabel.text = config["subtitle"] as? String
        subtitleLabel.font = .systemFont(ofSize: 13)
        subtitleLabel.textColor = UIColor.white.withAlphaComponent(0.72)
        let titles = UIStackView(arrangedSubviews: [titleLabel, subtitleLabel])
        titles.axis = .vertical
        titles.spacing = 1
        topBar.addArrangedSubview(closeView)
        topBar.addArrangedSubview(titles)
        topBar.axis = .horizontal
        topBar.alignment = .center
        topBar.spacing = 12
        topBar.translatesAutoresizingMaskIntoConstraints = false
        chrome.addSubview(topBar)

        // centre: back / play / forward
        let back = roundButton(backButton, "gobackward.10", size: 60, point: 24, action: #selector(skipBack))
        let play = roundButton(playButton, "pause.fill", size: 80, point: 32, action: #selector(togglePause))
        let fwd = roundButton(fwdButton, "goforward.10", size: 60, point: 24, action: #selector(skipForward))
        let centre = UIStackView(arrangedSubviews: [back, play, fwd])
        centre.axis = .horizontal
        centre.alignment = .center
        centre.spacing = 44
        centre.translatesAutoresizingMaskIntoConstraints = false
        chrome.addSubview(centre)

        // bottom: glass panel with scrubber, times and menus
        let panel = glass(26)
        panel.translatesAutoresizingMaskIntoConstraints = false
        chrome.addSubview(panel)
        slider.minimumTrackTintColor = .white
        slider.maximumTrackTintColor = UIColor.white.withAlphaComponent(0.25)
        slider.setThumbImage(Self.thumb(), for: .normal)
        slider.addTarget(self, action: #selector(sliderBegan), for: .touchDown)
        slider.addTarget(self, action: #selector(sliderMoved), for: .valueChanged)
        slider.addTarget(self, action: #selector(sliderEnded), for: [.touchUpInside, .touchUpOutside, .touchCancel])
        for l in [timeLabel, remainLabel] {
            l.font = .monospacedDigitSystemFont(ofSize: 13, weight: .semibold)
            l.textColor = .white
            l.text = "0:00"
        }
        let iconCfg = UIImage.SymbolConfiguration(pointSize: 18, weight: .semibold)
        for (b, sym) in [(subsButton, "captions.bubble"), (audioButton, "waveform"), (speedButton, "gauge.with.dots.needle.67percent"), (fillButton, "arrow.up.left.and.arrow.down.right")] {
            b.setImage(UIImage(systemName: sym, withConfiguration: iconCfg), for: .normal)
            b.tintColor = .white
            b.setTitleColor(.white, for: .normal)
            b.titleLabel?.font = .systemFont(ofSize: 13, weight: .semibold)
            b.widthAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
            b.heightAnchor.constraint(equalToConstant: 40).isActive = true
        }
        fillButton.addTarget(self, action: #selector(toggleFill), for: .touchUpInside)
        let row = UIStackView(arrangedSubviews: [timeLabel, slider, remainLabel])
        row.axis = .horizontal
        row.alignment = .center
        row.spacing = 12
        let buttons = UIStackView(arrangedSubviews: [UIView(), subsButton, audioButton, speedButton, fillButton])
        buttons.axis = .horizontal
        buttons.spacing = 6
        let stack = UIStackView(arrangedSubviews: [row, buttons])
        stack.axis = .vertical
        stack.spacing = 4
        stack.translatesAutoresizingMaskIntoConstraints = false
        panel.contentView.addSubview(stack)

        spinner.color = .white
        spinner.hidesWhenStopped = true
        spinner.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(spinner)
        spinner.startAnimating()

        errorLabel.font = .systemFont(ofSize: 16, weight: .semibold)
        errorLabel.textColor = .white
        errorLabel.numberOfLines = 0
        errorLabel.textAlignment = .center
        errorLabel.isHidden = true
        errorLabel.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(errorLabel)

        let safe = view.safeAreaLayoutGuide
        NSLayoutConstraint.activate([
            chrome.leadingAnchor.constraint(equalTo: view.leadingAnchor), chrome.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            chrome.topAnchor.constraint(equalTo: view.topAnchor), chrome.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            shadeView.leadingAnchor.constraint(equalTo: chrome.leadingAnchor), shadeView.trailingAnchor.constraint(equalTo: chrome.trailingAnchor),
            shadeView.topAnchor.constraint(equalTo: chrome.topAnchor), shadeView.bottomAnchor.constraint(equalTo: chrome.bottomAnchor),
            topBar.leadingAnchor.constraint(equalTo: safe.leadingAnchor, constant: 16),
            topBar.trailingAnchor.constraint(lessThanOrEqualTo: safe.trailingAnchor, constant: -16),
            topBar.topAnchor.constraint(equalTo: safe.topAnchor, constant: 10),
            centre.centerXAnchor.constraint(equalTo: view.centerXAnchor), centre.centerYAnchor.constraint(equalTo: view.centerYAnchor),
            panel.leadingAnchor.constraint(equalTo: safe.leadingAnchor, constant: 16),
            panel.trailingAnchor.constraint(equalTo: safe.trailingAnchor, constant: -16),
            panel.bottomAnchor.constraint(equalTo: safe.bottomAnchor, constant: -10),
            stack.leadingAnchor.constraint(equalTo: panel.contentView.leadingAnchor, constant: 18),
            stack.trailingAnchor.constraint(equalTo: panel.contentView.trailingAnchor, constant: -10),
            stack.topAnchor.constraint(equalTo: panel.contentView.topAnchor, constant: 10),
            stack.bottomAnchor.constraint(equalTo: panel.contentView.bottomAnchor, constant: -6),
            spinner.centerXAnchor.constraint(equalTo: view.centerXAnchor), spinner.centerYAnchor.constraint(equalTo: view.centerYAnchor),
            errorLabel.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            errorLabel.topAnchor.constraint(equalTo: centre.bottomAnchor, constant: 28),
            errorLabel.widthAnchor.constraint(lessThanOrEqualTo: view.widthAnchor, multiplier: 0.8),
        ])

        let tap = UITapGestureRecognizer(target: self, action: #selector(tapped(_:)))
        let dbl = UITapGestureRecognizer(target: self, action: #selector(doubleTapped(_:)))
        dbl.numberOfTapsRequired = 2
        tap.require(toFail: dbl)
        let pinch = UIPinchGestureRecognizer(target: self, action: #selector(pinched(_:)))
        let swipe = UISwipeGestureRecognizer(target: self, action: #selector(LumenMPVPlayer.swipedDown))
        swipe.direction = .down
        [tap, dbl, pinch, swipe].forEach { view.addGestureRecognizer($0) }
        for g in [tap, dbl] { g.cancelsTouchesInView = false; g.delegate = self }
    }

    private func updatePlayButton() {
        playButton.setImage(UIImage(systemName: paused ? "play.fill" : "pause.fill", withConfiguration: UIImage.SymbolConfiguration(pointSize: 32, weight: .semibold)), for: .normal)
    }

    private func updateSpinner() {
        if buffering && errorLabel.isHidden { spinner.startAnimating() } else { spinner.stopAnimating() }
        playButton.superview?.superview?.alpha = buffering ? 0 : 1
    }

    private func updateTimes() {
        guard !scrubbing else { return }
        timeLabel.text = Self.fmt(pos)
        remainLabel.text = duration > 0 ? "-" + Self.fmt(max(0, duration - pos)) : "--:--"
    }

    private func showChrome() {
        chromeVisible = true
        UIView.animate(withDuration: 0.25) { self.chrome.alpha = 1 }
        setNeedsStatusBarAppearanceUpdate()
        scheduleHide()
    }

    private func hideChrome() {
        guard errorLabel.isHidden, !scrubbing else { return }
        chromeVisible = false
        UIView.animate(withDuration: 0.35) { self.chrome.alpha = 0 }
        setNeedsStatusBarAppearanceUpdate()
    }

    private func scheduleHide() {
        hideTimer?.invalidate()
        hideTimer = Timer.scheduledTimer(withTimeInterval: 3.5, repeats: false) { [weak self] _ in
            guard let self = self, !self.paused else { return }
            self.hideChrome()
        }
    }

    private func haptic(_ s: UIImpactFeedbackGenerator.FeedbackStyle) {
        UIImpactFeedbackGenerator(style: s).impactOccurred()
    }

    private static func fmt(_ s: Double) -> String {
        let t = max(0, Int(s.isFinite ? s : 0))
        return t >= 3600 ? String(format: "%d:%02d:%02d", t / 3600, (t % 3600) / 60, t % 60) : String(format: "%d:%02d", t / 60, t % 60)
    }

    private static func thumb() -> UIImage {
        let size = CGSize(width: 16, height: 16)
        return UIGraphicsImageRenderer(size: size).image { ctx in
            UIColor.white.setFill()
            ctx.cgContext.fillEllipse(in: CGRect(origin: .zero, size: size))
        }
    }

    private static func language(_ code: String) -> String {
        guard !code.isEmpty else { return "" }
        return Locale.current.localizedString(forLanguageCode: code) ?? code.uppercased()
    }
}

extension LumenMPVPlayer: UIGestureRecognizerDelegate {
    // taps on buttons and the scrubber belong to them, not to show/hide
    func gestureRecognizer(_ g: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        var v = touch.view
        while let cur = v {
            if cur is UIControl { return false }
            v = cur.superview
        }
        return true
    }
}

/// A view whose backing layer is the given gradient (stays sized to the view).
private final class GradientView: UIView {
    private let gradient: CAGradientLayer

    init(gradient g: CAGradientLayer) {
        gradient = g
        super.init(frame: .zero)
        self.layer.addSublayer(gradient)
    }

    required init?(coder: NSCoder) { fatalError() }

    override func layoutSubviews() {
        super.layoutSubviews()
        gradient.frame = bounds
    }
}
