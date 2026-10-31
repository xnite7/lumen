import AVFoundation
import AVKit
import MediaPlayer
import SafariServices
import SwiftRs
import Tauri
import UIKit
import WebKit
#if canImport(ActivityKit)
import ActivityKit
#endif

class HapticArgs: Decodable {
    let style: String?
}

class ShareArgs: Decodable {
    let title: String?
    let text: String?
    let url: String?
}

class AuthArgs: Decodable {
    let url: String
}

class PlayArgs: Decodable {
    let url: String
    let title: String?
    let subtitle: String?
    let artwork: String?
    let startAt: Double?
    /// resume as a fraction of the length (a Simkl pause point)
    let startPct: Double?
    let tint: String?
    let headers: [String: String]?
    /// "auto" (Apple's player when it can, Lumen's otherwise) | "lumen"
    let engine: String?
    let subLangs: [String]?
    let audioLangs: [String]?
}

/// Lumen's native layer on iPhone.
///
/// - `play` presents AVPlayerViewController full screen: system controls,
///   picture-in-picture (also automatically when you swipe home), AirPlay,
///   lock screen / Control Centre playback, plus a Live Activity in the
///   Dynamic Island. Progress goes back to the page every few seconds so
///   Continue Watching and Simkl keep working.
/// - `auth` shows a Safari sheet for sign-in pages.
/// - `haptic` and `share` do what they say.
class LumenNativePlugin: Plugin, AVPlayerViewControllerDelegate {
    private var safari: SFSafariViewController?
    private var playerVC: AVPlayerViewController?
    private var player: AVPlayer?
    private var timeObserver: Any?
    private var statusObs: NSKeyValueObservation?
    private var controlObs: NSKeyValueObservation?
    private var endObs: NSObjectProtocol?
    private var current: PlayArgs?
    private var lastReport: Double = -10
    private var presented = false
    private var inPip = false
    private var activity: Any?
    private var mpvActive = false
    private var resumedPct = false
    private var observing = false

    // MARK: haptics

    @objc public func haptic(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(HapticArgs.self)
        DispatchQueue.main.async {
            switch args.style ?? "light" {
            case "selection":
                UISelectionFeedbackGenerator().selectionChanged()
            case "success":
                UINotificationFeedbackGenerator().notificationOccurred(.success)
            case "warning":
                UINotificationFeedbackGenerator().notificationOccurred(.warning)
            case "error":
                UINotificationFeedbackGenerator().notificationOccurred(.error)
            case "medium":
                UIImpactFeedbackGenerator(style: .medium).impactOccurred()
            case "heavy":
                UIImpactFeedbackGenerator(style: .heavy).impactOccurred()
            case "soft":
                UIImpactFeedbackGenerator(style: .soft).impactOccurred()
            case "rigid":
                UIImpactFeedbackGenerator(style: .rigid).impactOccurred()
            default:
                UIImpactFeedbackGenerator(style: .light).impactOccurred()
            }
        }
        invoke.resolve()
    }

    // MARK: share sheet

    @objc public func share(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(ShareArgs.self)
        var items: [Any] = []
        if let t = args.text ?? args.title { items.append(t) }
        if let u = args.url, let url = URL(string: u) { items.append(url) }
        DispatchQueue.main.async {
            guard let host = self.top() else {
                invoke.reject("No window to present from")
                return
            }
            let sheet = UIActivityViewController(activityItems: items, applicationActivities: nil)
            sheet.popoverPresentationController?.sourceView = host.view
            sheet.popoverPresentationController?.sourceRect = CGRect(x: host.view.bounds.midX, y: host.view.bounds.maxY - 80, width: 1, height: 1)
            host.present(sheet, animated: true)
            invoke.resolve()
        }
    }

    // MARK: sign-in sheet

    @objc public func auth(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(AuthArgs.self)
        guard let url = URL(string: args.url) else {
            invoke.reject("Bad address")
            return
        }
        DispatchQueue.main.async {
            let s = SFSafariViewController(url: url)
            s.dismissButtonStyle = .close
            s.modalPresentationStyle = .pageSheet
            self.safari = s
            self.top()?.present(s, animated: true)
            invoke.resolve()
        }
    }

    @objc public func endauth(_ invoke: Invoke) throws {
        DispatchQueue.main.async {
            self.safari?.dismiss(animated: true)
            self.safari = nil
            invoke.resolve()
        }
    }

    // MARK: player

    @objc public func play(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(PlayArgs.self)
        guard let url = URL(string: args.url) else {
            invoke.reject("Bad address")
            return
        }
        DispatchQueue.main.async {
            self.teardown(dismiss: true)
            self.observeLumenPlayer()
            if (args.engine == "lumen" || Self.needsLumenPlayer(args.url)), self.presentLumenPlayer(args) {
                invoke.resolve()
                return
            }
            let session = AVAudioSession.sharedInstance()
            try? session.setCategory(.playback, mode: .moviePlayback)
            try? session.setActive(true)

            var options: [String: Any] = [:]
            if let h = args.headers, !h.isEmpty { options["AVURLAssetHTTPHeaderFieldsKey"] = h }
            let asset = AVURLAsset(url: url, options: options)
            let item = AVPlayerItem(asset: asset)
            item.externalMetadata = self.metadata(args, artwork: nil)
            let p = AVPlayer(playerItem: item)
            p.allowsExternalPlayback = true

            let vc = AVPlayerViewController()
            vc.player = p
            vc.delegate = self
            vc.allowsPictureInPicturePlayback = true
            if #available(iOS 14.2, *) { vc.canStartPictureInPictureAutomaticallyFromInline = true }
            vc.updatesNowPlayingInfoCenter = true
            vc.modalPresentationStyle = .fullScreen

            self.player = p
            self.playerVC = vc
            self.current = args
            self.resumedPct = false
            self.lastReport = -10
            self.observe(p, item)
            self.loadArtwork(args, item: item)

            self.top()?.present(vc, animated: true) {
                self.presented = true
                if let s = args.startAt, s > 1 {
                    p.seek(to: CMTime(seconds: s, preferredTimescale: 600)) { _ in p.play() }
                } else {
                    p.play()
                }
            }
            self.startActivity(args)
            invoke.resolve()
        }
    }

    @objc public func stop(_ invoke: Invoke) throws {
        DispatchQueue.main.async {
            if self.mpvActive, let vc = self.top(), NSStringFromClass(type(of: vc)) == "LumenMPVPlayer" {
                self.mpvActive = false
                vc.perform(NSSelectorFromString("close"))
            }
            self.finish()
            invoke.resolve()
        }
    }

    private func observe(_ p: AVPlayer, _ item: AVPlayerItem) {
        timeObserver = p.addPeriodicTimeObserver(forInterval: CMTime(seconds: 1, preferredTimescale: 600), queue: .main) { [weak self] _ in
            self?.tick()
        }
        statusObs = item.observe(\.status, options: [.new]) { [weak self] it, _ in
            if it.status == .failed {
                DispatchQueue.main.async { self?.failed() }
            } else if it.status == .readyToPlay {
                DispatchQueue.main.async {
                    guard let self = self, let pct = self.current?.startPct, self.current?.startAt == nil, !self.resumedPct else { return }
                    self.resumedPct = true
                    let d = it.duration.seconds
                    if d.isFinite, d > 0 { self.player?.seek(to: CMTime(seconds: d * pct, preferredTimescale: 600)) }
                }
            }
        }
        controlObs = p.observe(\.timeControlStatus, options: [.new]) { [weak self] _, _ in
            DispatchQueue.main.async {
                self?.report(force: true)
                self?.updateActivity()
            }
        }
        endObs = NotificationCenter.default.addObserver(forName: .AVPlayerItemDidPlayToEndTime, object: item, queue: .main) { [weak self] _ in
            guard let self = self else { return }
            self.report(force: true)
            self.trigger("ended", data: [:] as JSObject)
        }
    }

    private var position: Double {
        let t = player?.currentTime().seconds ?? 0
        return t.isFinite ? t : 0
    }

    private var duration: Double {
        let d = player?.currentItem?.duration.seconds ?? 0
        return d.isFinite ? d : 0
    }

    private func tick() {
        report(force: false)
        // Closed some way we weren't told about (swipe-down, PiP closed).
        if presented, !inPip, let vc = playerVC, vc.viewIfLoaded?.window == nil, vc.presentingViewController == nil {
            finish()
        }
    }

    private func report(force: Bool) {
        guard let p = player else { return }
        let pos = position
        if !force && abs(pos - lastReport) < 5 { return }
        lastReport = pos
        trigger("progress", data: ["pos": pos, "duration": duration, "paused": p.timeControlStatus != .playing])
    }

    /// Player closed by the person: hand the final position back.
    private func finish() {
        guard player != nil else { return }
        let data: JSObject = ["pos": position, "duration": duration]
        teardown(dismiss: true)
        trigger("closed", data: data)
    }

    /// Apple's player couldn't open it: Lumen's own player takes over.
    private func failed() {
        guard let args = current else { return }
        let wasPresented = playerVC?.presentingViewController != nil
        teardown(dismiss: true)
        let fallback = { [weak self] in
            guard let self = self else { return }
            if !self.presentLumenPlayer(args) { self.trigger("failed", data: ["url": args.url]) }
        }
        if wasPresented { DispatchQueue.main.asyncAfter(deadline: .now() + 0.45, execute: fallback) } else { fallback() }
    }

    // MARK: Lumen's own player (mpv, in the app target)

    /// Containers AVPlayer can't open at all.
    static func needsLumenPlayer(_ url: String) -> Bool {
        let path = (URL(string: url)?.path ?? url).lowercased()
        return ["mkv", "avi", "webm", "wmv", "flv", "ts", "m2ts", "mts", "ogm", "divx", "3gp"].contains((path as NSString).pathExtension)
    }

    private func presentLumenPlayer(_ a: PlayArgs) -> Bool {
        guard let cls = NSClassFromString("LumenMPVPlayer") as? UIViewController.Type, let host = top() else { return false }
        let vc = cls.init(nibName: nil, bundle: nil)
        var cfg: [String: Any] = ["url": a.url, "title": a.title ?? "Lumen", "subtitle": a.subtitle ?? "", "artwork": a.artwork ?? ""]
        if let s = a.startAt { cfg["startAt"] = s }
        if let p = a.startPct { cfg["startPct"] = p }
        if let h = a.headers, !h.isEmpty { cfg["headers"] = h }
        if let s = a.subLangs { cfg["subLangs"] = s }
        if let l = a.audioLangs { cfg["audioLangs"] = l }
        vc.setValue(cfg as NSDictionary, forKey: "config")
        vc.modalPresentationStyle = .fullScreen
        current = a
        mpvActive = true
        host.present(vc, animated: true)
        startActivity(a)
        return true
    }

    private func observeLumenPlayer() {
        guard !observing else { return }
        observing = true
        let nc = NotificationCenter.default
        nc.addObserver(forName: Notification.Name("lumen.player.progress"), object: nil, queue: .main) { [weak self] n in
            guard let self = self, let u = n.userInfo else { return }
            let pos = u["pos"] as? Double ?? 0
            let dur = u["duration"] as? Double ?? 0
            let paused = u["paused"] as? Bool ?? false
            self.trigger("progress", data: ["pos": pos, "duration": dur, "paused": paused])
            self.updateActivity(paused: paused, position: pos, duration: dur)
        }
        nc.addObserver(forName: Notification.Name("lumen.player.ended"), object: nil, queue: .main) { [weak self] _ in
            self?.trigger("ended", data: [:] as JSObject)
        }
        nc.addObserver(forName: Notification.Name("lumen.player.closed"), object: nil, queue: .main) { [weak self] n in
            guard let self = self else { return }
            self.mpvActive = false
            self.endActivity()
            let u = n.userInfo ?? [:]
            self.trigger("closed", data: ["pos": u["pos"] as? Double ?? 0, "duration": u["duration"] as? Double ?? 0])
        }
    }

    private func teardown(dismiss: Bool) {
        if let t = timeObserver { player?.removeTimeObserver(t) }
        timeObserver = nil
        statusObs?.invalidate()
        controlObs?.invalidate()
        statusObs = nil
        controlObs = nil
        if let e = endObs { NotificationCenter.default.removeObserver(e) }
        endObs = nil
        player?.pause()
        if dismiss, let vc = playerVC, vc.presentingViewController != nil {
            vc.dismiss(animated: true)
        }
        playerVC?.player = nil
        playerVC = nil
        player = nil
        presented = false
        inPip = false
        endActivity()
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }

    // MARK: AVPlayerViewControllerDelegate

    func playerViewController(_ playerViewController: AVPlayerViewController, willEndFullScreenPresentationWithAnimationCoordinator coordinator: UIViewControllerTransitionCoordinator) {
        coordinator.animate(alongsideTransition: nil) { ctx in
            if !ctx.isCancelled && !self.inPip { self.finish() }
        }
    }

    func playerViewControllerDidStartPictureInPicture(_ playerViewController: AVPlayerViewController) {
        inPip = true
    }

    func playerViewControllerDidStopPictureInPicture(_ playerViewController: AVPlayerViewController) {
        inPip = false
        if playerViewController.presentingViewController == nil { finish() }
    }

    func playerViewControllerShouldAutomaticallyDismissAtPictureInPictureStart(_ playerViewController: AVPlayerViewController) -> Bool {
        true
    }

    func playerViewController(_ playerViewController: AVPlayerViewController, restoreUserInterfaceForPictureInPictureStopWithCompletionHandler completionHandler: @escaping (Bool) -> Void) {
        if playerViewController.presentingViewController == nil, let host = top() {
            host.present(playerViewController, animated: true) { completionHandler(true) }
        } else {
            completionHandler(true)
        }
    }

    // MARK: metadata (player title + Now Playing)

    private func metadata(_ a: PlayArgs, artwork: Data?) -> [AVMetadataItem] {
        func make(_ id: AVMetadataIdentifier, _ value: NSCopying & NSObjectProtocol) -> AVMetadataItem {
            let m = AVMutableMetadataItem()
            m.identifier = id
            m.value = value
            m.extendedLanguageTag = "und"
            return m.copy() as! AVMetadataItem
        }
        var out: [AVMetadataItem] = []
        if let t = a.title { out.append(make(.commonIdentifierTitle, t as NSString)) }
        if let s = a.subtitle, !s.isEmpty { out.append(make(.iTunesMetadataTrackSubTitle, s as NSString)) }
        if let d = artwork { out.append(make(.commonIdentifierArtwork, d as NSData)) }
        return out
    }

    private func loadArtwork(_ a: PlayArgs, item: AVPlayerItem) {
        guard let s = a.artwork, let url = URL(string: s), url.scheme?.hasPrefix("http") == true else { return }
        URLSession.shared.dataTask(with: url) { [weak self, weak item] data, _, _ in
            guard let self = self, let data = data, let item = item else { return }
            DispatchQueue.main.async { item.externalMetadata = self.metadata(a, artwork: data) }
        }.resume()
    }

    // MARK: Live Activity (Dynamic Island + Lock Screen)

    private func startActivity(_ a: PlayArgs) {
        #if canImport(ActivityKit)
        guard #available(iOS 16.2, *) else { return }
        guard ActivityAuthorizationInfo().areActivitiesEnabled else { return }
        endActivity()
        let attrs = LumenActivityAttributes(title: a.title ?? "Lumen", subtitle: a.subtitle ?? "", tint: a.tint ?? "#FFC22E")
        let state = LumenActivityAttributes.ContentState(paused: false, position: a.startAt ?? 0, duration: 0, sampledAt: Date())
        activity = try? Activity.request(attributes: attrs, content: .init(state: state, staleDate: nil), pushType: nil)
        #endif
    }

    private func updateActivity() {
        #if canImport(ActivityKit)
        guard #available(iOS 16.2, *), let act = activity as? Activity<LumenActivityAttributes>, let p = player else { return }
        let state = LumenActivityAttributes.ContentState(paused: p.timeControlStatus != .playing, position: position, duration: duration, sampledAt: Date())
        Task { await act.update(.init(state: state, staleDate: nil)) }
        #endif
    }

    private func updateActivity(paused: Bool, position: Double, duration: Double) {
        #if canImport(ActivityKit)
        guard #available(iOS 16.2, *), let act = activity as? Activity<LumenActivityAttributes> else { return }
        let state = LumenActivityAttributes.ContentState(paused: paused, position: position, duration: duration, sampledAt: Date())
        Task { await act.update(.init(state: state, staleDate: nil)) }
        #endif
    }

    private func endActivity() {
        #if canImport(ActivityKit)
        guard #available(iOS 16.2, *), let act = activity as? Activity<LumenActivityAttributes> else { return }
        activity = nil
        Task { await act.end(nil, dismissalPolicy: .immediate) }
        #endif
    }

    // MARK: helpers

    /// The topmost view controller to present from.
    private func top() -> UIViewController? {
        var vc = manager.viewController
        while let next = vc?.presentedViewController {
            vc = next
        }
        return vc
    }
}

@_cdecl("init_plugin_lumen_native")
func initPlugin() -> Plugin {
    return LumenNativePlugin()
}
