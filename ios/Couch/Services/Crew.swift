// The crew (couchers): who's on the couch right now and what they're
// watching, and the watch parties you're invited to. The same server the
// PC and Mac apps talk to, so everyone sees everyone.

import Foundation
import Observation

struct CrewMember: Identifiable, Hashable {
    var id: String
    var name: String
    var avatar: String?
    var title: String?
    var episode: String?
    var metaId: String?
    var type: String?
    var background: String?
    var paused: Bool
    var pos: Double
    var duration: Double

    var preview: MetaPreview? {
        guard let metaId, metaId.hasPrefix("tt"), let title else { return nil }
        return MetaPreview(id: metaId, type: Kind(rawValue: type ?? "movie") ?? .movie, name: title, background: background)
    }
}

struct WatchParty: Identifiable, Hashable {
    var id: String
    var host: String
    var hostName: String
    var title: MetaPreview
    var at: Date
    var note: String
    /// who said yes
    var going: [String]
    /// what you said (nil = not yet)
    var mine: Bool?
}

@MainActor @Observable
final class Crew {
    static let shared = Crew()
    private(set) var watching: [CrewMember] = []
    private(set) var parties: [WatchParty] = []
    private var loaded = Date.distantPast
    private var sent = Date.distantPast

    /// show the crew what you're watching (Settings)
    var share: Bool = UserDefaults.standard.object(forKey: "crew.share") as? Bool ?? true {
        didSet { UserDefaults.standard.set(share, forKey: "crew.share") }
    }

    private init() {}

    private func request(_ method: String, _ path: String, _ body: [String: Any]? = nil) async -> [String: Any]? {
        guard Account.shared.crew, let tok = await Account.shared.token() else { return nil }
        var r = URLRequest(url: URL(string: crewURL + path)!)
        r.httpMethod = method
        r.timeoutInterval = 12
        r.cachePolicy = .reloadIgnoringLocalCacheData
        r.setValue("Bearer \(tok)", forHTTPHeaderField: "authorization")
        if let body {
            r.setValue("application/json", forHTTPHeaderField: "content-type")
            r.httpBody = try? JSONSerialization.data(withJSONObject: body)
        }
        guard let (d, res) = try? await Net.session.data(for: r), (res as? HTTPURLResponse)?.statusCode == 200 else { return nil }
        return try? JSONSerialization.jsonObject(with: d) as? [String: Any]
    }

    /// Who's watching and the parties. Quiet when you're not a coucher, and
    /// keeps what it had when the server can't be reached.
    func refresh(force: Bool = false) async {
        guard Account.shared.crew else {
            watching = []
            parties = []
            return
        }
        if !force, Date().timeIntervalSince(loaded) < 20 { return }
        loaded = .now
        let me = Account.shared.discord?.id
        if let v = await request("GET", "/v1/presence"), let rows = v["members"] as? [[String: Any]] {
            watching = rows.compactMap { m in
                guard let id = m["id"] as? String, id != me, let w = m["watching"] as? [String: Any], let t = w["title"] as? String, !t.isEmpty else { return nil }
                // (anyone not heard from for a while has left the couch)
                if let at = (m["at"] as? NSNumber)?.doubleValue, Date().timeIntervalSince1970 * 1000 - at > 90_000 { return nil }
                return CrewMember(id: id, name: m["name"] as? String ?? "someone", avatar: m["avatar"] as? String, title: t,
                                  episode: (w["episode"] as? String).flatMap { $0.isEmpty ? nil : $0 }, metaId: w["metaId"] as? String,
                                  type: w["type"] as? String, background: (w["background"] as? String).flatMap { $0.isEmpty ? nil : $0 },
                                  paused: m["paused"] as? Bool ?? false, pos: (m["pos"] as? NSNumber)?.doubleValue ?? 0,
                                  duration: (m["duration"] as? NSNumber)?.doubleValue ?? 0)
            }
        }
        if let v = await request("GET", "/v1/parties"), let rows = v["parties"] as? [[String: Any]] {
            parties = rows.compactMap { Self.party($0, me: me) }
        }
    }

    private static func party(_ p: [String: Any], me: String?) -> WatchParty? {
        guard let id = p["id"] as? String, let t = p["title"] as? [String: Any], let tid = t["id"] as? String, let name = t["name"] as? String,
              let at = (p["at"] as? NSNumber)?.doubleValue else { return nil }
        let rsvp = (p["rsvp"] as? [String: [String: Any]]) ?? [:]
        let s = { (k: String) in (t[k] as? String).flatMap { $0.isEmpty ? nil : $0 } }
        return WatchParty(id: id, host: p["host"] as? String ?? "", hostName: p["hostName"] as? String ?? "someone",
                          title: MetaPreview(id: tid, type: Kind(rawValue: t["type"] as? String ?? "movie") ?? .movie, name: name,
                                             poster: s("poster"), background: s("background"), logo: s("logo"), releaseInfo: s("year")),
                          at: Date(timeIntervalSince1970: at / 1000), note: p["note"] as? String ?? "",
                          going: rsvp.values.filter { $0["yes"] as? Bool == true }.compactMap { $0["name"] as? String }.sorted(),
                          mine: me.flatMap { rsvp[$0]?["yes"] as? Bool })
    }

    /// "I'm in" / "can't make it"
    func rsvp(_ party: WatchParty, _ yes: Bool) async {
        let me = Account.shared.discord?.id
        if let v = await request("POST", "/v1/party/\(party.id)/rsvp", ["yes": yes]), let p = v["party"] as? [String: Any],
           let new = Self.party(p, me: me), let i = parties.firstIndex(where: { $0.id == party.id }) {
            parties[i] = new
        }
    }

    /// Tell the crew what's on here (every ~10 s while playing; once more when it stops).
    func announce() async {
        guard Account.shared.crew, share else { return }
        let pb = Playback.shared
        var body: [String: Any] = [:]
        if pb.isOpen, let m = pb.meta {
            guard Date().timeIntervalSince(sent) > 10 else { return }
            var w: [String: Any] = ["title": m.name, "metaId": m.id, "type": m.type.rawValue,
                                    "link": "\(crewURL)/t/\(m.type.rawValue)/\(m.id)"]
            if let e = pb.episode { w["episode"] = "S\(e.season ?? 0) E\(e.episode ?? 0) · \(e.label)" }
            if let x = m.poster { w["poster"] = x }
            if let x = m.backdropURL?.absoluteString { w["background"] = x }
            if let y = Int(m.preview.year) { w["year"] = y }
            body = ["watching": w, "paused": pb.paused, "pos": pb.lastPos, "duration": pb.lastDur]
        } else {
            // only say "stopped" if we'd said "watching"
            guard sent != .distantPast else { return }
            sent = .distantPast
            _ = await request("PUT", "/v1/presence", ["watching": NSNull()])
            return
        }
        sent = .now
        _ = await request("PUT", "/v1/presence", body)
    }
}

/// What's coming for the shows you follow (My List and what you're in the
/// middle of): episodes out in the last few days and the next two months.
struct Upcoming: Identifiable, Hashable {
    var id: String { episode.id }
    var show: MetaPreview
    var episode: Episode
    var date: Date
}

@MainActor @Observable
final class ComingUp {
    static let shared = ComingUp()
    private(set) var items: [Upcoming] = []
    private(set) var loading = false
    private var loaded = Date.distantPast
    private init() {}

    func refresh(force: Bool = false) async {
        if loading || (!force && Date().timeIntervalSince(loaded) < 1800) { return }
        loading = true
        defer { loading = false }
        let lib = Library.shared
        var shows: [MetaPreview] = []
        var seen: Set<String> = []
        for m in lib.continueWatching.map(\.meta) + lib.myList where m.type == .series && m.id.hasPrefix("tt") && seen.insert(m.id).inserted {
            shows.append(m)
        }
        let from = Date().addingTimeInterval(-3 * 86400), to = Date().addingTimeInterval(60 * 86400)
        var out: [Upcoming] = []
        // a few at a time: kind to the catalogue server
        for m in shows.prefix(60) {
            guard let d = try? await Cinemeta.shared.meta(.series, m.id) else { continue }
            for e in d.videos ?? [] where (e.season ?? 0) > 0 {
                if let at = e.releaseDate, at >= from, at <= to, !lib.isWatched(e.id) { out.append(Upcoming(show: d.preview, episode: e, date: at)) }
            }
        }
        items = out.sorted { $0.date < $1.date }
        loaded = .now
    }
}
