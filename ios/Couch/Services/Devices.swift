// Your own devices, Spotify style: this iPhone / iPad tells the couch server
// what it's playing; your PC, Mac or another phone playing shows here as
// "playing on …" with play/pause and Play Here (the film moves over at the
// same spot). Needs a couch session (Discord, or Pro from the App Store).

import Foundation
import Observation
import UIKit

struct OtherDevice: Identifiable, Hashable {
    var id: String
    var name: String
    var kind: String
    var title: String
    var episode: String
    var metaId: String
    var type: String
    var videoId: String?
    var pos: Double
    var duration: Double
    var paused: Bool

    var symbol: String {
        switch kind {
        case "iphone": "iphone"
        case "ipad": "ipad"
        case "mac": "macbook"
        default: "desktopcomputer"
        }
    }
}

@MainActor @Observable
final class Devices {
    static let shared = Devices()
    private(set) var playingElsewhere: OtherDevice?
    private var loop: Task<Void, Never>?
    /// what you just tapped, shown right away until the other device confirms
    private var pending: (id: String, paused: Bool, at: Date)?

    private var me: String { Account.shared.deviceId }
    private var kind: String { UIDevice.current.userInterfaceIdiom == .pad ? "ipad" : "iphone" }

    private init() {}

    func start() {
        guard loop == nil else { return }
        loop = Task {
            while !Task.isCancelled {
                await tick()
                // quick while something is playing somewhere, relaxed otherwise
                let busy = playingElsewhere != nil || Playback.shared.isOpen
                try? await Task.sleep(for: .seconds(busy ? 2.5 : 8))
            }
        }
    }

    private func request(_ method: String, _ path: String, _ body: [String: Any]) async -> [String: Any]? {
        guard let tok = await Account.shared.token() else { return nil }
        var r = URLRequest(url: URL(string: crewURL + path)!)
        r.httpMethod = method
        r.cachePolicy = .reloadIgnoringLocalCacheData
        r.setValue("application/json", forHTTPHeaderField: "content-type")
        r.setValue("Bearer \(tok)", forHTTPHeaderField: "authorization")
        r.httpBody = try? JSONSerialization.data(withJSONObject: body)
        guard let (d, res) = try? await Net.session.data(for: r), (res as? HTTPURLResponse)?.statusCode == 200 else { return nil }
        return try? JSONSerialization.jsonObject(with: d) as? [String: Any]
    }

    private func tick() async {
        guard Account.shared.known, UIApplication.shared.applicationState == .active || Playback.shared.isOpen else { return }
        let pb = Playback.shared
        var body: [String: Any] = ["id": me, "name": UIDevice.current.userInterfaceIdiom == .pad ? "iPad" : "iPhone", "kind": kind]
        if pb.isOpen, let m = pb.meta {
            var p: [String: Any] = [
                "metaId": m.id, "type": m.type.rawValue, "title": m.name, "videoId": pb.episode?.id ?? m.id,
                "pos": pb.lastPos, "duration": pb.lastDur, "paused": pb.paused,
            ]
            if let e = pb.episode { p["episode"] = "S\(e.season ?? 0) E\(e.episode ?? 0) · \(e.label)" }
            if let x = m.poster { p["poster"] = x }
            if let x = m.background { p["background"] = x }
            body["playing"] = p
        }
        guard let r = await request("PUT", "/v1/me/device", body) else {
            playingElsewhere = nil
            return
        }
        for c in (r["cmds"] as? [[String: Any]]) ?? [] { apply(c) }
        let others = ((r["devices"] as? [[String: Any]]) ?? []).filter { ($0["me"] as? Bool) != true }
        var found: OtherDevice?
        for d in others {
            guard let p = d["playing"] as? [String: Any], let id = d["id"] as? String else { continue }
            found = OtherDevice(id: id, name: d["name"] as? String ?? "couch", kind: d["kind"] as? String ?? "pc",
                                title: p["title"] as? String ?? "", episode: p["episode"] as? String ?? "",
                                metaId: p["metaId"] as? String ?? "", type: p["type"] as? String ?? "movie",
                                videoId: p["videoId"] as? String, pos: p["pos"] as? Double ?? 0,
                                duration: p["duration"] as? Double ?? 0, paused: p["paused"] as? Bool ?? false)
            break
        }
        if var f = found, let pend = pending, pend.id == f.id {
            if f.paused == pend.paused || Date().timeIntervalSince(pend.at) > 6 { pending = nil } else { f.paused = pend.paused; found = f }
        }
        // while this device plays, the bar stays out of the way
        playingElsewhere = pb.isOpen ? nil : found
    }

    private func apply(_ c: [String: Any]) {
        let pb = Playback.shared
        guard pb.isOpen else { return }
        switch c["cmd"] as? String {
        case "pause": pb.control("pause", value: true)
        case "play": pb.control("pause", value: false)
        case "seek": if let p = c["pos"] as? Double { pb.control("seek", value: p) }
        case "next": pb.nextEpisode()
        case "stop": pb.control("stop")
        default: break
        }
    }

    // MARK: controlling the other device

    func togglePause(_ d: OtherDevice) {
        let want = !d.paused
        pending = (d.id, want, .now)
        playingElsewhere?.paused = want
        Task { _ = await request("POST", "/v1/me/device/\(d.id)/cmd", ["cmd": want ? "pause" : "play", "from": me]) }
    }

    /// Move it to this device: stop there, carry on here at the same spot.
    func playHere(_ d: OtherDevice) {
        playingElsewhere = nil
        Task {
            _ = await request("POST", "/v1/me/device/\(d.id)/cmd", ["cmd": "stop", "from": me])
            await Playback.shared.takeOver(type: Kind(rawValue: d.type) ?? .movie, metaId: d.metaId, videoId: d.videoId, at: d.pos, duration: d.duration)
        }
    }
}
