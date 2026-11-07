// couch's own video player on iPhone: mpv (via MPVKit) rendering through
// Metal, with couch's controls in Liquid Glass. Plays what Apple's player
// can't: MKV, HEVC/AV1 in any container, Dolby Vision, styled (ASS)
// subtitles, multiple audio tracks.
//
// It lives in the app target (not the Tauri plugin) because MPVKit ships as
// Swift-package binaries Xcode links. The plugin finds it by its Objective-C
// name, hands it a config dictionary, and listens for these notifications:
//   couch.player.progress  { pos, duration, paused }
//   couch.player.ended     {}
//   couch.player.closed    { pos, duration }
//   couch.player.failed    { message }
//   couch.player.rate      { value: -1 meh | 1 liked | 2 loved | 0 cleared }
//   couch.player.episode   { id }   picked from the Episodes list
//   couch.player.next      {}       the Next Episode button
// and it listens for couch.player.load (a new config) to switch episodes
// without closing.

import AVFoundation
import Libmpv
import MediaPlayer
import UIKit

private enum Note {
    static let progress = Notification.Name("couch.player.progress")
    static let ended = Notification.Name("couch.player.ended")
    static let closed = Notification.Name("couch.player.closed")
    static let failed = Notification.Name("couch.player.failed")
    static let rate = Notification.Name("couch.player.rate")
    static let episode = Notification.Name("couch.player.episode")
    static let next = Notification.Name("couch.player.next")
    static let load = Notification.Name("couch.player.load")
}

/// couch's colours (match the app's tokens)
private extension UIColor {
    static let lamp = UIColor(red: 1.0, green: 0.698, blue: 0.361, alpha: 1)
    static let neon = UIColor(red: 0.878, green: 0.188, blue: 0.604, alpha: 1)
    static let warmWhite = UIColor(red: 0.957, green: 0.918, blue: 0.941, alpha: 1)
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

@objc(CouchMPVPlayer)
final class CouchMPVPlayer: UIViewController {
    /// url, title, subtitle, artwork, startAt, headers, subLangs, audioLangs
    @objc var config: NSDictionary = [:]

    private var metalLayer = MPVMetalLayer()
    private var mpv: OpaquePointer?
    private let queue = DispatchQueue(label: "couch.mpv", qos: .userInitiated)

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
    private var lockedMask: UIInterfaceOrientationMask?
    private var rating = 0 // -1 meh, 1 liked, 2 loved
    private var episodesOpen = false

    // UI
    private let chrome = UIView()
    private let topBar = UIStackView()
    private let titleLabel = UILabel()
    private let closeButton = UIButton(type: .system)
    private let lockButton = UIButton(type: .system)
    private let rateStack = UIStackView()
    private var rateButtons: [Int: UIButton] = [:]
    private let playButton = UIButton(type: .system)
    private let backButton = UIButton(type: .system)
    private let fwdButton = UIButton(type: .system)
    private let slider = UISlider()
    private let remainLabel = UILabel()
    private let speedButton = UIButton(type: .system)
    private let episodesButton = UIButton(type: .system)
    private let tracksButton = UIButton(type: .system)
    private let nextButton = UIButton(type: .system)
    private let optionsRow = UIStackView()
    private let brightness = UISlider()
    private let brightnessBox = UIView()
    private let spinner = UIActivityIndicatorView(style: .large)
    private let errorLabel = UILabel()
    private let episodesPanel = UIView()
    private let episodesTable = UITableView(frame: .zero, style: .plain)
    private var episodesTrailing: NSLayoutConstraint?
    private var landscapeRate: [NSLayoutConstraint] = []
    private var portraitRate: [NSLayoutConstraint] = []

    private var episodes: [[String: Any]] { config["episodes"] as? [[String: Any]] ?? [] }

    // MARK: lifecycle

    override var prefersStatusBarHidden: Bool { !chromeVisible }
    override var prefersHomeIndicatorAutoHidden: Bool { true }
    override var supportedInterfaceOrientations: UIInterfaceOrientationMask { lockedMask ?? .allButUpsideDown }
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
        NotificationCenter.default.addObserver(self, selector: #selector(loadNew(_:)), name: Note.load, object: nil)
        rating = config["rating"] as? Int ?? 0
        updateRating()
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
        // the swapchain must follow the view, or portrait comes out squashed
        // into the old landscape size
        let scale = metalLayer.contentsScale
        metalLayer.drawableSize = CGSize(width: view.bounds.width * scale, height: view.bounds.height * scale)
        CATransaction.commit()
        let portrait = view.bounds.height > view.bounds.width
        NSLayoutConstraint.deactivate(portrait ? landscapeRate : portraitRate)
        NSLayoutConstraint.activate(portrait ? portraitRate : landscapeRate)
        // labels drop off the bottom options when there isn't room for them
        for b in [speedButton, episodesButton, tracksButton, nextButton] {
            b.configuration?.title = portrait ? nil : b.accessibilityLabel
        }
        updateSpeedTitle()
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
        set("user-agent", "couch/1.0")
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
            let me = Unmanaged<CouchMPVPlayer>.fromOpaque(raw).takeUnretainedValue()
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
        if let s = config["startAt"] as? Double, s > 1 {
            opts.append("start=\(Int(s))")
        } else if let p = config["startPct"] as? Double, p > 0.01 {
            opts.append(String(format: "start=%.2f%%", p * 100))
        }
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
        lockedMask = nil
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
        scheduleHide()
    }

    @objc private func sliderBegan() {
        scrubbing = true
        hideTimer?.invalidate()
    }

    @objc private func sliderMoved() {
        guard duration > 0 else { return }
        let t = Double(slider.value) * duration
        remainLabel.text = Self.fmt(t)
    }

    @objc private func sliderEnded() {
        scrubbing = false
        guard duration > 0 else { return }
        command(["seek", String(Double(slider.value) * duration), "absolute"])
        haptic(.light)
        scheduleHide()
    }

    @objc private func tapped(_ g: UITapGestureRecognizer) {
        if episodesOpen { return closeEpisodes() }
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

    // MARK: new episode in the same player

    @objc private func loadNew(_ n: Notification) {
        guard let cfg = n.userInfo?["config"] as? NSDictionary, let url = cfg["url"] as? String else { return }
        config = cfg
        titleLabel.text = Self.title(cfg)
        rating = cfg["rating"] as? Int ?? 0
        updateRating()
        nextButton.isHidden = !(cfg["hasNext"] as? Bool ?? false)
        episodesButton.isHidden = episodes.isEmpty
        episodesTable.reloadData()
        errorLabel.isHidden = true
        buffering = true
        pos = 0
        duration = 0
        slider.value = 0
        updateSpinner()
        closeEpisodes()
        let h = cfg["headers"] as? [String: String] ?? [:]
        command(["set", "http-header-fields", h.map { "\($0.key): \($0.value.replacingOccurrences(of: ",", with: "\\,"))" }.joined(separator: ",")])
        load(url)
        updateNowPlaying()
    }

    @objc private func rateTapped(_ b: UIButton) {
        let v = b.tag
        rating = rating == v ? 0 : v
        haptic(rating == 2 ? .heavy : .light)
        updateRating()
        UIView.animate(withDuration: 0.12, animations: { b.transform = CGAffineTransform(scaleX: 1.25, y: 1.25) }) { _ in
            UIView.animate(withDuration: 0.35, delay: 0, usingSpringWithDamping: 0.45, initialSpringVelocity: 4) { b.transform = .identity }
        }
        NotificationCenter.default.post(name: Note.rate, object: nil, userInfo: ["value": rating])
        scheduleHide()
    }

    private func updateRating() {
        let syms = [-1: ("hand.thumbsdown", "hand.thumbsdown.fill"), 1: ("hand.thumbsup", "hand.thumbsup.fill"), 2: ("heart", "heart.fill")]
        for (v, b) in rateButtons {
            let on = rating == v
            let (off, fill) = syms[v] ?? ("circle", "circle.fill")
            b.setImage(UIImage(systemName: on ? fill : off, withConfiguration: UIImage.SymbolConfiguration(pointSize: 18, weight: .semibold)), for: .normal)
            b.tintColor = on ? (v == 2 ? .neon : .lamp) : .warmWhite
            b.superview?.backgroundColor = on ? UIColor.white.withAlphaComponent(0.2) : UIColor.white.withAlphaComponent(0.1)
        }
    }

    @objc private func toggleLock() {
        haptic(.light)
        if lockedMask != nil {
            lockedMask = nil
        } else {
            let o = view.window?.windowScene?.interfaceOrientation ?? .landscapeRight
            lockedMask = o.isPortrait ? .portrait : (o == .landscapeLeft ? .landscapeLeft : .landscapeRight)
        }
        if #available(iOS 16.0, *) { setNeedsUpdateOfSupportedInterfaceOrientations() }
        lockButton.setImage(UIImage(systemName: lockedMask != nil ? "lock.rotation" : "lock.rotation.open", withConfiguration: UIImage.SymbolConfiguration(pointSize: 20, weight: .semibold)), for: .normal)
        lockButton.tintColor = lockedMask != nil ? .lamp : .warmWhite
        scheduleHide()
    }

    @objc private func brightnessChanged() {
        UIScreen.main.brightness = CGFloat(brightness.value)
        scheduleHide()
    }

    @objc private func nextTapped() {
        haptic(.light)
        NotificationCenter.default.post(name: Note.next, object: nil)
    }

    @objc private func episodesTapped() {
        haptic(.light)
        episodesOpen ? closeEpisodes() : openEpisodes()
    }

    private func openEpisodes() {
        episodesOpen = true
        hideTimer?.invalidate()
        episodesTable.reloadData()
        if let i = episodes.firstIndex(where: { ($0["current"] as? Bool) == true }) {
            episodesTable.scrollToRow(at: IndexPath(row: i, section: 0), at: .middle, animated: false)
        }
        episodesTrailing?.constant = 0
        UIView.animate(withDuration: 0.45, delay: 0, usingSpringWithDamping: 0.86, initialSpringVelocity: 0) { self.view.layoutIfNeeded() }
    }

    private func closeEpisodes() {
        guard episodesOpen else { return }
        episodesOpen = false
        episodesTrailing?.constant = 440
        UIView.animate(withDuration: 0.3) { self.view.layoutIfNeeded() }
        scheduleHide()
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
        // one menu, two sections: Audio and Subtitles
        tracksButton.menu = UIMenu(title: "", children: [UIDeferredMenuElement.uncached { [weak self] done in
            guard let self = self else { return done([]) }
            let all = self.tracks()
            let audio = all.filter { $0.type == "audio" }
            let subs = all.filter { $0.type == "sub" }
            let audioMenu = UIMenu(title: "Audio", options: .displayInline, children: audio.map { t in
                UIAction(title: t.title, state: t.selected ? .on : .off) { [weak self] _ in self?.command(["set", "aid", String(t.id)]) }
            })
            var subItems: [UIMenuElement] = [UIAction(title: "Off", state: subs.contains { $0.selected } ? .off : .on) { [weak self] _ in self?.command(["set", "sid", "no"]) }]
            subItems += subs.map { t in UIAction(title: t.title, state: t.selected ? .on : .off) { [weak self] _ in self?.command(["set", "sid", String(t.id)]) } }
            let subMenu = UIMenu(title: "Subtitles", options: .displayInline, children: subItems)
            let timing = UIMenu(title: "Subtitle timing", children: [
                UIAction(title: "Earlier 0.1 s", image: UIImage(systemName: "minus")) { [weak self] _ in self?.command(["add", "sub-delay", "-0.1"]) },
                UIAction(title: "Later 0.1 s", image: UIImage(systemName: "plus")) { [weak self] _ in self?.command(["add", "sub-delay", "0.1"]) },
            ])
            done([audioMenu, subMenu, timing])
        }])
        speedButton.menu = UIMenu(title: "Speed", children: [UIDeferredMenuElement.uncached { [weak self] done in
            guard let self = self else { return done([]) }
            let cur = Double(self.getString("speed") ?? "1") ?? 1
            done([0.5, 0.75, 1.0, 1.25, 1.5, 2.0].map { s in
                UIAction(title: s == 1 ? "1× (normal)" : "\(s)×", state: abs(cur - s) < 0.01 ? .on : .off) { [weak self] _ in
                    self?.command(["set", "speed", String(s)])
                    self?.speed = s
                    self?.updateSpeedTitle()
                }
            })
        }])
        [tracksButton, speedButton].forEach { $0.showsMenuAsPrimaryAction = true }
    }

    private var speed: Double = 1
    private func updateSpeedTitle() {
        let portrait = view.bounds.height > view.bounds.width
        let s = speed == 1 ? "1x" : String(format: "%gx", speed)
        speedButton.configuration?.title = portrait ? nil : "Speed (\(s))"
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
            MPMediaItemPropertyTitle: config["title"] as? String ?? "couch",
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

    private func iconButton(_ b: UIButton, _ symbol: String, point: CGFloat, action: Selector?) {
        b.translatesAutoresizingMaskIntoConstraints = false
        b.setImage(UIImage(systemName: symbol, withConfiguration: UIImage.SymbolConfiguration(pointSize: point, weight: .semibold)), for: .normal)
        b.tintColor = .warmWhite
        if let a = action { b.addTarget(self, action: a, for: .touchUpInside) }
    }

    /// The labelled options along the bottom ("Speed (1x)", "Episodes"…).
    private func option(_ b: UIButton, _ symbol: String, _ title: String, action: Selector?) {
        var c = UIButton.Configuration.plain()
        c.image = UIImage(systemName: symbol, withConfiguration: UIImage.SymbolConfiguration(pointSize: 17, weight: .semibold))
        c.imagePadding = 8
        c.title = title
        c.baseForegroundColor = .warmWhite
        c.contentInsets = NSDirectionalEdgeInsets(top: 8, leading: 10, bottom: 8, trailing: 10)
        c.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { a in
            var a = a
            a.font = .systemFont(ofSize: 15, weight: .semibold)
            return a
        }
        b.configuration = c
        b.accessibilityLabel = title
        if let a = action { b.addTarget(self, action: a, for: .touchUpInside) }
    }

    private func buildUI() {
        chrome.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(chrome)
        // shade top and bottom so the controls read on any frame
        let shade = CAGradientLayer()
        shade.colors = [UIColor.black.withAlphaComponent(0.6).cgColor, UIColor.clear.cgColor, UIColor.clear.cgColor, UIColor.black.withAlphaComponent(0.7).cgColor]
        shade.locations = [0, 0.28, 0.6, 1]
        let shadeView = GradientView(gradient: shade)
        shadeView.translatesAutoresizingMaskIntoConstraints = false
        shadeView.isUserInteractionEnabled = false
        chrome.addSubview(shadeView)

        // top left: couch mark + "S1:E3 "Title""
        let mark = UIImageView(image: UIImage(systemName: "sofa.fill"))
        mark.tintColor = .neon
        mark.contentMode = .scaleAspectFit
        mark.widthAnchor.constraint(equalToConstant: 26).isActive = true
        titleLabel.text = Self.title(config)
        titleLabel.font = .systemFont(ofSize: 17, weight: .bold)
        titleLabel.textColor = .warmWhite
        titleLabel.lineBreakMode = .byTruncatingTail
        titleLabel.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        // top right: rotation lock, close
        iconButton(lockButton, "lock.rotation.open", point: 20, action: #selector(toggleLock))
        iconButton(closeButton, "xmark", point: 20, action: #selector(CouchMPVPlayer.close))
        for b in [lockButton, closeButton] {
            b.widthAnchor.constraint(equalToConstant: 44).isActive = true
            b.heightAnchor.constraint(equalToConstant: 44).isActive = true
        }
        let spacer = UIView()
        spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
        [mark, titleLabel, spacer, lockButton, closeButton].forEach { topBar.addArrangedSubview($0) }
        topBar.axis = .horizontal
        topBar.alignment = .center
        topBar.spacing = 12
        topBar.setCustomSpacing(4, after: lockButton)
        topBar.translatesAutoresizingMaskIntoConstraints = false
        chrome.addSubview(topBar)

        // ratings: meh / liked / loved (Simkl 4, 8, 10)
        for (v, label) in [(-1, "Not for me"), (1, "I liked it"), (2, "Loved it")] {
            let bubble = UIView()
            bubble.translatesAutoresizingMaskIntoConstraints = false
            bubble.layer.cornerRadius = 26
            bubble.backgroundColor = UIColor.white.withAlphaComponent(0.1)
            let b = UIButton(type: .system)
            b.tag = v
            b.accessibilityLabel = label
            iconButton(b, "circle", point: 18, action: #selector(rateTapped(_:)))
            bubble.addSubview(b)
            NSLayoutConstraint.activate([
                bubble.widthAnchor.constraint(equalToConstant: 52), bubble.heightAnchor.constraint(equalToConstant: 52),
                b.leadingAnchor.constraint(equalTo: bubble.leadingAnchor), b.trailingAnchor.constraint(equalTo: bubble.trailingAnchor),
                b.topAnchor.constraint(equalTo: bubble.topAnchor), b.bottomAnchor.constraint(equalTo: bubble.bottomAnchor),
            ])
            rateButtons[v] = b
            rateStack.addArrangedSubview(bubble)
        }
        rateStack.axis = .horizontal
        rateStack.spacing = 18
        rateStack.translatesAutoresizingMaskIntoConstraints = false
        chrome.addSubview(rateStack)

        // left: brightness
        brightnessBox.translatesAutoresizingMaskIntoConstraints = false
        chrome.addSubview(brightnessBox)
        let sun = UIImageView(image: UIImage(systemName: "sun.max", withConfiguration: UIImage.SymbolConfiguration(pointSize: 18, weight: .semibold)))
        sun.tintColor = .warmWhite
        sun.translatesAutoresizingMaskIntoConstraints = false
        brightnessBox.addSubview(sun)
        brightness.minimumValue = 0
        brightness.maximumValue = 1
        brightness.value = Float(UIScreen.main.brightness)
        brightness.minimumTrackTintColor = .warmWhite
        brightness.maximumTrackTintColor = UIColor.white.withAlphaComponent(0.25)
        brightness.setThumbImage(UIImage(), for: .normal)
        brightness.transform = CGAffineTransform(rotationAngle: -.pi / 2)
        brightness.translatesAutoresizingMaskIntoConstraints = false
        brightness.addTarget(self, action: #selector(brightnessChanged), for: .valueChanged)
        brightnessBox.addSubview(brightness)

        // centre: back 10 · play/pause · forward 10 (plain glyphs, no discs)
        iconButton(backButton, "gobackward.10", point: 34, action: #selector(skipBack))
        iconButton(playButton, "pause.fill", point: 52, action: #selector(togglePause))
        iconButton(fwdButton, "goforward.10", point: 34, action: #selector(skipForward))
        for b in [backButton, fwdButton] {
            b.widthAnchor.constraint(equalToConstant: 72).isActive = true
            b.heightAnchor.constraint(equalToConstant: 72).isActive = true
        }
        playButton.widthAnchor.constraint(equalToConstant: 96).isActive = true
        playButton.heightAnchor.constraint(equalToConstant: 96).isActive = true
        let centre = UIStackView(arrangedSubviews: [backButton, playButton, fwdButton])
        centre.axis = .horizontal
        centre.alignment = .center
        centre.spacing = 56
        centre.translatesAutoresizingMaskIntoConstraints = false
        chrome.addSubview(centre)

        // bottom: scrubber with time left, then the labelled options
        slider.minimumTrackTintColor = .lamp
        slider.maximumTrackTintColor = UIColor.white.withAlphaComponent(0.28)
        slider.setThumbImage(Self.thumb(.lamp, 20), for: .normal)
        slider.setThumbImage(Self.thumb(.lamp, 28), for: .highlighted)
        slider.addTarget(self, action: #selector(sliderBegan), for: .touchDown)
        slider.addTarget(self, action: #selector(sliderMoved), for: .valueChanged)
        slider.addTarget(self, action: #selector(sliderEnded), for: [.touchUpInside, .touchUpOutside, .touchCancel])
        remainLabel.font = .monospacedDigitSystemFont(ofSize: 15, weight: .semibold)
        remainLabel.textColor = .warmWhite
        remainLabel.text = "--:--"
        remainLabel.setContentHuggingPriority(.required, for: .horizontal)
        let scrubRow = UIStackView(arrangedSubviews: [slider, remainLabel])
        scrubRow.axis = .horizontal
        scrubRow.alignment = .center
        scrubRow.spacing = 16

        option(speedButton, "gauge.with.dots.needle.67percent", "Speed (1x)", action: nil)
        option(episodesButton, "rectangle.stack", "Episodes", action: #selector(episodesTapped))
        option(tracksButton, "captions.bubble", "Audio & Subtitles", action: nil)
        option(nextButton, "forward.end", "Next Episode", action: #selector(nextTapped))
        episodesButton.isHidden = episodes.isEmpty
        nextButton.isHidden = !(config["hasNext"] as? Bool ?? false)
        [UIView(), speedButton, episodesButton, tracksButton, nextButton, UIView()].forEach { optionsRow.addArrangedSubview($0) }
        optionsRow.axis = .horizontal
        optionsRow.alignment = .center
        optionsRow.spacing = 14
        optionsRow.distribution = .equalSpacing

        let bottom = UIStackView(arrangedSubviews: [scrubRow, optionsRow])
        bottom.axis = .vertical
        bottom.spacing = 6
        bottom.translatesAutoresizingMaskIntoConstraints = false
        chrome.addSubview(bottom)

        // the Episodes list slides in from the right
        let panelGlass = UIVisualEffectView(effect: UIBlurEffect(style: .systemUltraThinMaterialDark))
        panelGlass.translatesAutoresizingMaskIntoConstraints = false
        episodesPanel.translatesAutoresizingMaskIntoConstraints = false
        episodesPanel.layer.cornerRadius = 22
        episodesPanel.layer.cornerCurve = .continuous
        episodesPanel.clipsToBounds = true
        episodesPanel.backgroundColor = UIColor.black.withAlphaComponent(0.55)
        episodesPanel.addSubview(panelGlass)
        episodesTable.translatesAutoresizingMaskIntoConstraints = false
        episodesTable.backgroundColor = .clear
        episodesTable.separatorStyle = .none
        episodesTable.dataSource = self
        episodesTable.delegate = self
        episodesTable.register(EpisodeCell.self, forCellReuseIdentifier: "ep")
        episodesTable.rowHeight = 92
        episodesPanel.addSubview(episodesTable)
        view.addSubview(episodesPanel)

        spinner.color = .warmWhite
        spinner.hidesWhenStopped = true
        spinner.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(spinner)
        spinner.startAnimating()

        errorLabel.font = .systemFont(ofSize: 16, weight: .semibold)
        errorLabel.textColor = .warmWhite
        errorLabel.numberOfLines = 0
        errorLabel.textAlignment = .center
        errorLabel.isHidden = true
        errorLabel.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(errorLabel)

        let safe = view.safeAreaLayoutGuide
        let trailing = episodesPanel.trailingAnchor.constraint(equalTo: safe.trailingAnchor, constant: 440)
        episodesTrailing = trailing
        landscapeRate = [rateStack.centerXAnchor.constraint(equalTo: view.centerXAnchor), rateStack.centerYAnchor.constraint(equalTo: topBar.centerYAnchor),
                         titleLabel.trailingAnchor.constraint(lessThanOrEqualTo: rateStack.leadingAnchor, constant: -16).withPriority(.defaultHigh)]
        portraitRate = [rateStack.centerXAnchor.constraint(equalTo: view.centerXAnchor), rateStack.topAnchor.constraint(equalTo: topBar.bottomAnchor, constant: 18)]
        NSLayoutConstraint.activate([
            chrome.leadingAnchor.constraint(equalTo: view.leadingAnchor), chrome.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            chrome.topAnchor.constraint(equalTo: view.topAnchor), chrome.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            shadeView.leadingAnchor.constraint(equalTo: chrome.leadingAnchor), shadeView.trailingAnchor.constraint(equalTo: chrome.trailingAnchor),
            shadeView.topAnchor.constraint(equalTo: chrome.topAnchor), shadeView.bottomAnchor.constraint(equalTo: chrome.bottomAnchor),
            topBar.leadingAnchor.constraint(equalTo: safe.leadingAnchor, constant: 20),
            topBar.trailingAnchor.constraint(equalTo: safe.trailingAnchor, constant: -12),
            topBar.topAnchor.constraint(equalTo: safe.topAnchor, constant: 8),

            brightnessBox.leadingAnchor.constraint(equalTo: safe.leadingAnchor, constant: 18),
            brightnessBox.centerYAnchor.constraint(equalTo: view.centerYAnchor),
            brightnessBox.widthAnchor.constraint(equalToConstant: 40),
            brightnessBox.heightAnchor.constraint(equalToConstant: 200),
            sun.centerXAnchor.constraint(equalTo: brightnessBox.centerXAnchor), sun.topAnchor.constraint(equalTo: brightnessBox.topAnchor),
            brightness.centerXAnchor.constraint(equalTo: brightnessBox.centerXAnchor),
            brightness.centerYAnchor.constraint(equalTo: brightnessBox.centerYAnchor, constant: 18),
            brightness.widthAnchor.constraint(equalToConstant: 150),

            centre.centerXAnchor.constraint(equalTo: view.centerXAnchor), centre.centerYAnchor.constraint(equalTo: view.centerYAnchor),

            bottom.leadingAnchor.constraint(equalTo: safe.leadingAnchor, constant: 24),
            bottom.trailingAnchor.constraint(equalTo: safe.trailingAnchor, constant: -24),
            bottom.bottomAnchor.constraint(equalTo: safe.bottomAnchor, constant: -6),

            episodesPanel.topAnchor.constraint(equalTo: safe.topAnchor, constant: 12),
            episodesPanel.bottomAnchor.constraint(equalTo: safe.bottomAnchor, constant: -12),
            episodesPanel.widthAnchor.constraint(equalToConstant: 400),
            trailing,
            panelGlass.leadingAnchor.constraint(equalTo: episodesPanel.leadingAnchor), panelGlass.trailingAnchor.constraint(equalTo: episodesPanel.trailingAnchor),
            panelGlass.topAnchor.constraint(equalTo: episodesPanel.topAnchor), panelGlass.bottomAnchor.constraint(equalTo: episodesPanel.bottomAnchor),
            episodesTable.leadingAnchor.constraint(equalTo: episodesPanel.leadingAnchor), episodesTable.trailingAnchor.constraint(equalTo: episodesPanel.trailingAnchor),
            episodesTable.topAnchor.constraint(equalTo: episodesPanel.topAnchor, constant: 8), episodesTable.bottomAnchor.constraint(equalTo: episodesPanel.bottomAnchor),

            spinner.centerXAnchor.constraint(equalTo: view.centerXAnchor), spinner.centerYAnchor.constraint(equalTo: view.centerYAnchor),
            errorLabel.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            errorLabel.topAnchor.constraint(equalTo: centre.bottomAnchor, constant: 28),
            errorLabel.widthAnchor.constraint(lessThanOrEqualTo: view.widthAnchor, multiplier: 0.8),
        ])
        NSLayoutConstraint.activate(landscapeRate)

        let tap = UITapGestureRecognizer(target: self, action: #selector(tapped(_:)))
        let dbl = UITapGestureRecognizer(target: self, action: #selector(doubleTapped(_:)))
        dbl.numberOfTapsRequired = 2
        tap.require(toFail: dbl)
        let pinch = UIPinchGestureRecognizer(target: self, action: #selector(pinched(_:)))
        let swipe = UISwipeGestureRecognizer(target: self, action: #selector(CouchMPVPlayer.swipedDown))
        swipe.direction = .down
        [tap, dbl, pinch, swipe].forEach { view.addGestureRecognizer($0) }
        for g in [tap, dbl, swipe] { g.cancelsTouchesInView = false; g.delegate = self }
        rebuildMenus()
    }

    private static func title(_ c: NSDictionary) -> String {
        let t = c["title"] as? String ?? ""
        let s = c["subtitle"] as? String ?? ""
        if let label = c["episodeLabel"] as? String, !label.isEmpty {
            return s.isEmpty ? "\(t)  \(label)" : "\(label) \u{201C}\(s)\u{201D}"
        }
        return t
    }

    private func updatePlayButton() {
        playButton.setImage(UIImage(systemName: paused ? "play.fill" : "pause.fill", withConfiguration: UIImage.SymbolConfiguration(pointSize: 52, weight: .bold)), for: .normal)
    }

    private func updateSpinner() {
        if buffering && errorLabel.isHidden { spinner.startAnimating() } else { spinner.stopAnimating() }
        playButton.superview?.alpha = buffering ? 0 : 1
    }

    private func updateTimes() {
        guard !scrubbing else { return }
        remainLabel.text = duration > 0 ? Self.fmt(max(0, duration - pos)) : "--:--"
    }

    private func showChrome() {
        chromeVisible = true
        UIView.animate(withDuration: 0.25) { self.chrome.alpha = 1 }
        setNeedsStatusBarAppearanceUpdate()
        scheduleHide()
    }

    private func hideChrome() {
        guard errorLabel.isHidden, !scrubbing, !episodesOpen else { return }
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

    private static func thumb(_ color: UIColor, _ d: CGFloat) -> UIImage {
        let size = CGSize(width: d, height: d)
        return UIGraphicsImageRenderer(size: size).image { ctx in
            color.setFill()
            ctx.cgContext.fillEllipse(in: CGRect(origin: .zero, size: size))
        }
    }

    private static func language(_ code: String) -> String {
        guard !code.isEmpty else { return "" }
        return Locale.current.localizedString(forLanguageCode: code) ?? code.uppercased()
    }
}

extension CouchMPVPlayer: UIGestureRecognizerDelegate {
    // taps on buttons and the scrubber belong to them, not to show/hide
    func gestureRecognizer(_ g: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        var v = touch.view
        while let cur = v {
            if cur is UIControl || cur === episodesPanel { return false }
            v = cur.superview
        }
        return true
    }
}

extension CouchMPVPlayer: UITableViewDataSource, UITableViewDelegate {
    func tableView(_ t: UITableView, numberOfRowsInSection s: Int) -> Int { episodes.count }

    func tableView(_ t: UITableView, cellForRowAt i: IndexPath) -> UITableViewCell {
        let c = t.dequeueReusableCell(withIdentifier: "ep", for: i) as! EpisodeCell
        c.show(episodes[i.row])
        return c
    }

    func tableView(_ t: UITableView, didSelectRowAt i: IndexPath) {
        t.deselectRow(at: i, animated: true)
        let e = episodes[i.row]
        guard let id = e["id"] as? String, (e["current"] as? Bool) != true else { return closeEpisodes() }
        haptic(.medium)
        NotificationCenter.default.post(name: Note.episode, object: nil, userInfo: ["id": id])
    }
}

/// One row of the Episodes list: still, "3 · Title", length or progress.
private final class EpisodeCell: UITableViewCell {
    private let still = UIImageView()
    private let name = UILabel()
    private let detail = UILabel()
    private let bar = UIProgressView(progressViewStyle: .bar)
    private var task: URLSessionDataTask?

    override init(style: UITableViewCell.CellStyle, reuseIdentifier: String?) {
        super.init(style: style, reuseIdentifier: reuseIdentifier)
        backgroundColor = .clear
        selectionStyle = .none
        still.contentMode = .scaleAspectFill
        still.clipsToBounds = true
        still.layer.cornerRadius = 8
        still.backgroundColor = UIColor.white.withAlphaComponent(0.08)
        name.font = .systemFont(ofSize: 15, weight: .semibold)
        name.textColor = .warmWhite
        detail.font = .systemFont(ofSize: 12)
        detail.textColor = UIColor.white.withAlphaComponent(0.6)
        detail.numberOfLines = 2
        bar.progressTintColor = .lamp
        bar.trackTintColor = UIColor.white.withAlphaComponent(0.15)
        let text = UIStackView(arrangedSubviews: [name, detail, bar])
        text.axis = .vertical
        text.spacing = 4
        let row = UIStackView(arrangedSubviews: [still, text])
        row.spacing = 12
        row.alignment = .center
        row.translatesAutoresizingMaskIntoConstraints = false
        contentView.addSubview(row)
        NSLayoutConstraint.activate([
            still.widthAnchor.constraint(equalToConstant: 112), still.heightAnchor.constraint(equalToConstant: 63),
            row.leadingAnchor.constraint(equalTo: contentView.leadingAnchor, constant: 14),
            row.trailingAnchor.constraint(equalTo: contentView.trailingAnchor, constant: -14),
            row.centerYAnchor.constraint(equalTo: contentView.centerYAnchor),
        ])
    }

    required init?(coder: NSCoder) { fatalError() }

    func show(_ e: [String: Any]) {
        let n = e["episode"] as? Int ?? 0
        let cur = (e["current"] as? Bool) == true
        name.text = "\(n) · \(e["title"] as? String ?? "Episode \(n)")"
        name.textColor = cur ? .neon : .warmWhite
        detail.text = e["overview"] as? String
        let p = e["progress"] as? Double ?? 0
        bar.isHidden = p <= 0 && !cur
        bar.progress = Float(cur ? 1 : p)
        bar.progressTintColor = cur ? .neon : .lamp
        still.image = nil
        task?.cancel()
        if let s = e["thumbnail"] as? String, let url = URL(string: s) {
            task = URLSession.shared.dataTask(with: url) { [weak self] d, _, _ in
                guard let d = d, let img = UIImage(data: d) else { return }
                DispatchQueue.main.async { self?.still.image = img }
            }
            task?.resume()
        }
    }
}

private extension NSLayoutConstraint {
    func withPriority(_ p: UILayoutPriority) -> NSLayoutConstraint {
        priority = p
        return self
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
