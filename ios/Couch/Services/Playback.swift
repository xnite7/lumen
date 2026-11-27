// Playing something: finds a stream from your sources, opens couch's mpv
// player, remembers where you stop, rolls on to the next episode, and runs
// the Live Activity.

import ActivityKit
import Foundation
import Observation
import UIKit

@MainActor @Observable
final class Playback {
    static let shared = Playback()

    private(set) var finding: String?     // meta id while looking for a stream
    var problem: String?                  // shown as an alert
    private(set) var meta: MetaDetail?
    private(set) var episode: Episode?
    private(set) var stream: Stream?
    private var activity: Activity<CouchActivityAttributes>?
    private var observers: [NSObjectProtocol] = []
    private var lastPos: Double = 0
    private var lastDur: Double = 0
    private var lastSaved = Date.distantPast

    private init() {}

    // MARK: start

    /// Play a movie, or an episode (nil = resume or the first unwatched one).
    func play(_ m: MetaDetail, episode e: Episode? = nil, stream picked: Stream? = nil) async {
        guard Sources.shared.any else {
            problem = "Add a source first. Settings → Sources lets you add your own server."
            return
        }
        let ep = m.type == .series ? (e ?? upNext(m)) : nil
        if m.type == .series && ep == nil {
            problem = "No episodes are out yet."
            return
        }
        let vid = ep?.id ?? m.id
        finding = m.id
        defer { finding = nil }
        guard let s = picked ?? (await Sources.shared.best(m.type, vid, like: stream)) else {
            problem = "None of your sources has \(ep.map { "\(m.name) \($0.code)" } ?? m.name) right now."
            return
        }
        meta = m
        episode = ep
        stream = s
        present(config(m, ep, s, resume: true))
    }

    /// Where a series picks up: the episode in progress, else the one after
    /// the last you finished, else the first.
    func upNext(_ m: MetaDetail) -> Episode? {
        let lib = Library.shared
        if let p = lib.resume(m.id), let id = Optional(p.videoId), let e = m.videos?.first(where: { $0.id == id }) {
            return p.finished ? (m.after(id) ?? e) : e
        }
        let all = m.seasons.filter { $0 > 0 }.flatMap { m.episodes($0) }.filter(\.isOut)
        if let last = all.lastIndex(where: { lib.isWatched($0.id) }) {
            return last + 1 < all.count ? all[last + 1] : all[last]
        }
        return all.first
    }

    private func config(_ m: MetaDetail, _ ep: Episode?, _ s: Stream, resume: Bool) -> [String: Any] {
        var c: [String: Any] = [
            "url": s.url ?? "",
            "title": m.name,
            "subtitle": ep.map { "\($0.code)  \($0.label)" } ?? m.preview.year,
            "artwork": m.backdropURL?.absoluteString ?? "",
        ]
        if let ep { c["episodeLabel"] = ep.code }
        if resume, let p = Library.shared.resume(m.id), p.videoId == (ep?.id ?? m.id), !p.finished, p.position > 30 {
            c["startAt"] = p.position
        }
        if let h = s.behaviorHints?.proxyHeaders?.request, !h.isEmpty { c["headers"] = h }
        let langs = Locale.preferredLanguages.compactMap { Locale(identifier: $0).language.languageCode?.identifier(.alpha3) }
        c["audioLangs"] = langs + ["eng"]
        c["subLangs"] = langs + ["eng"]
        if let ep, let season = ep.season {
            c["hasNext"] = m.after(ep.id) != nil
            c["episodes"] = m.episodes(season).map { e -> [String: Any] in
                var d: [String: Any] = ["id": e.id, "episode": e.episode ?? 0, "title": e.label, "current": e.id == ep.id]
                if let o = e.about { d["overview"] = o }
                if let t = e.thumbnail { d["thumbnail"] = t }
                if Library.shared.isWatched(e.id) { d["progress"] = 1.0 }
                return d
            }
        }
        return c
    }

    // MARK: the player

    private func present(_ cfg: [String: Any]) {
        guard let host = Self.top() else { return }
        if NSStringFromClass(type(of: host)).contains("CouchMPVPlayer") {
            NotificationCenter.default.post(name: .init("couch.player.load"), object: nil, userInfo: ["config": cfg as NSDictionary])
        } else {
            let vc = CouchMPVPlayer()
            vc.config = cfg as NSDictionary
            vc.modalPresentationStyle = .fullScreen
            listen()
            host.present(vc, animated: true)
        }
        lastPos = (cfg["startAt"] as? Double) ?? 0
        lastDur = 0
        startActivity()
    }

    static func top() -> UIViewController? {
        var vc = UIApplication.shared.connectedScenes.compactMap { ($0 as? UIWindowScene)?.keyWindow }.first?.rootViewController
        while let p = vc?.presentedViewController { vc = p }
        return vc
    }

    private func listen() {
        guard observers.isEmpty else { return }
        let nc = NotificationCenter.default
        func on(_ n: String, _ f: @escaping @MainActor ([AnyHashable: Any]) -> Void) {
            observers.append(nc.addObserver(forName: .init(n), object: nil, queue: .main) { note in
                let info = note.userInfo ?? [:]
                MainActor.assumeIsolated { f(info) }
            })
        }
        on("couch.player.progress") { [weak self] u in
            guard let self else { return }
            self.lastPos = u["pos"] as? Double ?? 0
            self.lastDur = u["duration"] as? Double ?? 0
            let paused = u["paused"] as? Bool ?? false
            self.updateActivity(paused: paused)
            if Date().timeIntervalSince(self.lastSaved) > 10 || paused { self.save() }
        }
        on("couch.player.ended") { [weak self] _ in
            guard let self else { return }
            self.lastPos = self.lastDur
            self.save()
            if let m = self.meta, let ep = self.episode, let n = m.after(ep.id) { Task { await self.switchTo(n) } }
        }
        on("couch.player.closed") { [weak self] u in
            guard let self else { return }
            if let p = u["pos"] as? Double, p > 0 { self.lastPos = p }
            if let d = u["duration"] as? Double, d > 0 { self.lastDur = d }
            self.save()
            self.endActivity()
            self.observers.forEach { NotificationCenter.default.removeObserver($0) }
            self.observers = []
        }
        on("couch.player.episode") { [weak self] u in
            guard let self, let id = u["id"] as? String, let e = self.meta?.videos?.first(where: { $0.id == id }) else { return }
            self.save()
            Task { await self.switchTo(e) }
        }
        on("couch.player.next") { [weak self] _ in
            guard let self, let m = self.meta, let ep = self.episode, let n = m.after(ep.id) else { return }
            self.save()
            Task { await self.switchTo(n) }
        }
    }

    /// Another episode in the same player, from the same release when it can.
    private func switchTo(_ e: Episode) async {
        guard let m = meta else { return }
        guard let s = await Sources.shared.best(m.type, e.id, like: stream) else {
            NotificationCenter.default.post(name: .init("couch.player.failed"), object: nil, userInfo: ["message": "no stream for \(e.code)"])
            return
        }
        episode = e
        stream = s
        present(config(m, e, s, resume: true))
    }

    private func save() {
        guard let m = meta, lastDur > 0 else { return }
        lastSaved = .now
        Library.shared.record(Progress(meta: m.preview, videoId: episode?.id ?? m.id, season: episode?.season, episode: episode?.episode,
                                       episodeTitle: episode?.label, position: lastPos, duration: lastDur, updated: .now))
    }

    // MARK: Live Activity

    private func startActivity() {
        guard ActivityAuthorizationInfo().areActivitiesEnabled, let m = meta else { return }
        let attrs = CouchActivityAttributes(title: m.name, subtitle: episode.map { "\($0.code)  \($0.label)" } ?? "", tint: "FFFFFF")
        let state = CouchActivityAttributes.ContentState(paused: false, position: lastPos, duration: 0, sampledAt: .now)
        if let a = activity {
            Task { await a.end(nil, dismissalPolicy: .immediate) }
        }
        activity = try? Activity.request(attributes: attrs, content: .init(state: state, staleDate: nil))
    }

    private var lastState: CouchActivityAttributes.ContentState?
    private func updateActivity(paused: Bool) {
        guard let a = activity else { return }
        let s = CouchActivityAttributes.ContentState(paused: paused, position: lastPos, duration: lastDur, sampledAt: .now)
        // the widget runs its own clock: only tell it when that clock would be wrong
        if let l = lastState, l.paused == paused, l.duration == lastDur {
            let drift = abs((l.position + (paused ? 0 : Date().timeIntervalSince(l.sampledAt))) - lastPos)
            if drift < 3 { return }
        }
        lastState = s
        Task { await a.update(.init(state: s, staleDate: nil)) }
    }

    private func endActivity() {
        guard let a = activity else { return }
        activity = nil
        lastState = nil
        Task { await a.end(nil, dismissalPolicy: .immediate) }
    }
}
