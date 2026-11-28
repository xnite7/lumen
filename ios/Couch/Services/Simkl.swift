// Simkl (optional): your watchlist and what you've watched, the same on
// every couch. Signing in is PKCE in the system's sign-in sheet; the tokens
// sit in your iCloud Keychain, so the link follows your Apple ID.
//
// What couch does with it: brings your Plan to Watch and Watching into My
// List, what you've finished and where you stopped (on any couch) into this
// one, and sends what you do here back.
//
// Built to not break: a pull only happens when Simkl says something moved;
// nothing is removed here unless a whole pull came back clean; what couldn't
// be sent waits on disk and goes out in order later; a busy or failing Simkl
// is retried with a pause; one refresh at a time; a sign-in Simkl no longer
// accepts asks you to link again instead of failing quietly.

import AuthenticationServices
import CryptoKit
import Foundation
import Observation

nonisolated(unsafe) private var liveSimklAuth: ASWebAuthenticationSession?

@MainActor @Observable
final class Simkl: NSObject {
    static let shared = Simkl()

    private let api = "https://api.simkl.com"
    /// couch's Simkl app (public by design; PKCE protects the sign-in)
    private let clientId = "9c325049149440f9e66dc4499c37fc140ba6962583a3fa7927d8e27e3bd68cfc"
    private let redirect = "lumen://auth/simkl"

    private(set) var name: String? = UserDefaults.standard.string(forKey: "simkl.name")
    private(set) var syncing = false
    private(set) var lastError: String?
    /// Simkl turned the saved sign-in down: link again
    private(set) var needsReconnect = false
    /// changes made here that haven't reached Simkl yet
    var waiting: Int { outbox.count }
    var connected: Bool { Keychain.get("simkl.access") != nil }

    private var lastSync = Date.distantPast
    private var refreshing: Task<Bool?, Never>?
    /// list status per IMDb id, from the last pull
    private var status: [String: String] = (UserDefaults.standard.dictionary(forKey: "simkl.status") as? [String: String]) ?? [:]
    /// Simkl's pause ids per video id, from the last pull
    private var pauses: [String: Int] = [:]

    override private init() {
        super.init()
        if let d = try? Data(contentsOf: outboxURL), let o = try? JSONDecoder().decode([Job].self, from: d) { outbox = o }
    }

    // MARK: sign in

    func connect() async throws {
        let verifier = Account.random(64)
        let challenge = Data(SHA256.hash(data: Data(verifier.utf8))).base64URL
        let state = Account.random(24)
        var c = URLComponents(string: "https://simkl.com/oauth2/authorize")!
        c.queryItems = [
            .init(name: "response_type", value: "code"), .init(name: "client_id", value: clientId),
            .init(name: "redirect_uri", value: redirect), .init(name: "scope", value: "media:read media:write"),
            .init(name: "state", value: state), .init(name: "code_challenge", value: challenge),
            .init(name: "code_challenge_method", value: "S256"),
        ]
        let back: URL = try await withCheckedThrowingContinuation { cont in
            let s = ASWebAuthenticationSession(url: c.url!, callbackURLScheme: "lumen") { url, err in
                if let url { cont.resume(returning: url) } else { cont.resume(throwing: err ?? CouchError.message("sign-in was cancelled")) }
            }
            s.presentationContextProvider = Account.shared
            liveSimklAuth = s
            s.start()
        }
        let q = URLComponents(url: back, resolvingAgainstBaseURL: false)?.queryItems ?? []
        guard q.first(where: { $0.name == "state" })?.value == state, let code = q.first(where: { $0.name == "code" })?.value else {
            throw CouchError.message("Simkl didn't finish the sign-in")
        }
        try await tokens(["grant_type": "authorization_code", "client_id": clientId, "code": code, "redirect_uri": redirect, "code_verifier": verifier])
        // possibly another account than before: start from nothing
        forgetMirror()
        needsReconnect = false
        await whoAmI()
        await sync(force: true)
    }

    func disconnect() {
        Keychain.set("simkl.access", nil)
        Keychain.set("simkl.refresh", nil)
        name = nil
        needsReconnect = false
        UserDefaults.standard.removeObject(forKey: "simkl.name")
        forgetMirror()
    }

    private func forgetMirror() {
        outbox = []
        persist()
        status = [:]
        pauses = [:]
        UserDefaults.standard.removeObject(forKey: "simkl.status")
        UserDefaults.standard.removeObject(forKey: "simkl.all")
        lastSync = .distantPast
    }

    /// Throws CouchError when Simkl says no, the network's own error when it
    /// couldn't be reached (those two mean different things to the caller).
    private func tokens(_ form: [String: String]) async throws {
        var r = URLRequest(url: URL(string: api + "/oauth2/token")!)
        r.httpMethod = "POST"
        r.timeoutInterval = 15
        r.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "content-type")
        r.httpBody = form.map { "\($0.key)=\($0.value.addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? $0.value)" }.joined(separator: "&").data(using: .utf8)
        let (d, res) = try await Net.session.data(for: r)
        let code = (res as? HTTPURLResponse)?.statusCode ?? 0
        if code == 429 || code >= 500 { throw URLError(.badServerResponse) }
        guard let v = try? JSONSerialization.jsonObject(with: d) as? [String: Any], let access = v["access_token"] as? String, !access.isEmpty else {
            throw CouchError.message("Simkl didn't accept the sign-in")
        }
        Keychain.set("simkl.access", access)
        if let rt = v["refresh_token"] as? String, !rt.isEmpty { Keychain.set("simkl.refresh", rt) }
    }

    /// One refresh at a time. true = fresh tokens, false = Simkl refused,
    /// nil = couldn't tell (offline, Simkl down).
    private func refresh(used: String) async -> Bool? {
        // another call, or another of your devices through iCloud, already did
        if let now = Keychain.get("simkl.access"), now != used { return true }
        if let t = refreshing { return await t.value }
        let t = Task { () -> Bool? in
            guard let rt = Keychain.get("simkl.refresh") else { return false }
            do {
                try await self.tokens(["grant_type": "refresh_token", "client_id": self.clientId, "refresh_token": rt])
                return true
            } catch is CouchError {
                // a device on the same Apple ID may have refreshed a moment ago
                try? await Task.sleep(for: .seconds(2))
                if let now = Keychain.get("simkl.access"), now != used { return true }
                return false
            } catch {
                return nil
            }
        }
        refreshing = t
        let ok = await t.value
        refreshing = nil
        return ok
    }

    // MARK: requests

    private enum Reply {
        case ok(Any)
        case gone          // the sign-in isn't accepted any more
        case refused(Int)  // Simkl said no to this request (won't change by retrying)
        case failed        // couldn't reach it / it was busy: try later
    }

    private func send(_ method: String, _ path: String, _ body: Data? = nil) async -> Reply {
        let v = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "1"
        let sep = path.contains("?") ? "&" : "?"
        guard let url = URL(string: "\(api)\(path)\(sep)client_id=\(clientId)&app-name=couch&app-version=\(v)") else { return .refused(0) }
        var refreshed = false
        for attempt in 0..<3 {
            guard let tok = Keychain.get("simkl.access") else { return .gone }
            var r = URLRequest(url: url)
            r.httpMethod = method
            r.timeoutInterval = 15
            r.cachePolicy = .reloadIgnoringLocalCacheData
            r.setValue("Bearer \(tok)", forHTTPHeaderField: "authorization")
            r.setValue(clientId, forHTTPHeaderField: "simkl-api-key")
            r.setValue("application/json", forHTTPHeaderField: "content-type")
            r.setValue("midnite-couch (+https://github.com/xnite7) couch-ios/\(v)", forHTTPHeaderField: "user-agent")
            r.httpBody = body
            let pause = Double(attempt * 2 + 1)
            guard let (d, res) = try? await Net.session.data(for: r), let h = res as? HTTPURLResponse else {
                if attempt < 2 { try? await Task.sleep(for: .seconds(pause)) }
                continue
            }
            switch h.statusCode {
            case 200..<300:
                needsReconnect = false
                return .ok((try? JSONSerialization.jsonObject(with: d, options: [.fragmentsAllowed])) ?? [String: Any]())
            case 401:
                if refreshed { needsReconnect = true; return .gone }
                refreshed = true
                switch await refresh(used: tok) {
                case .some(true): continue
                case .some(false): needsReconnect = true; return .gone
                case .none: return .failed
                }
            case 429, 500...599:
                let asked = h.value(forHTTPHeaderField: "retry-after").flatMap(Double.init) ?? pause
                if asked > 20 { return .failed }
                if attempt < 2 { try? await Task.sleep(for: .seconds(max(1, asked))) }
            default:
                return .refused(h.statusCode)
            }
        }
        return .failed
    }

    private func get(_ path: String) async -> Any? {
        if case .ok(let v) = await send("GET", path) { return v }
        return nil
    }

    private func whoAmI() async {
        if case .ok(let v) = await send("POST", "/users/settings"),
           let u = (v as? [String: Any])?["user"] as? [String: Any], let n = u["name"] as? String {
            name = n
            UserDefaults.standard.set(n, forKey: "simkl.name")
        }
    }

    // MARK: what's waiting to be sent

    private struct Job: Codable {
        var path: String
        var body: Data
        var at: Date
        var tries = 0
    }
    private var outbox: [Job] = []
    private var flushing = false
    private var outboxURL: URL {
        let d = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        try? FileManager.default.createDirectory(at: d, withIntermediateDirectories: true)
        return d.appendingPathComponent("simkl-outbox.json")
    }
    private func persist() {
        if let d = try? JSONEncoder().encode(outbox) { try? d.write(to: outboxURL, options: .atomic) }
    }

    /// A change for Simkl: kept on disk until Simkl has it.
    private func queue(_ path: String, _ body: [String: Any]) {
        guard connected, let d = try? JSONSerialization.data(withJSONObject: body) else { return }
        outbox.append(Job(path: path, body: d, at: .now))
        if outbox.count > 300 { outbox.removeFirst(outbox.count - 300) }
        persist()
        Task { await flush() }
    }

    /// Sends what's waiting, oldest first; stops at the first that can't go yet.
    func flush() async {
        guard connected, !flushing, !outbox.isEmpty else { return }
        flushing = true
        defer { flushing = false }
        while let j = outbox.first {
            if Date().timeIntervalSince(j.at) > 14 * 86400 || j.tries > 40 {
                outbox.removeFirst()
                persist()
                continue
            }
            switch await send("POST", j.path, j.body) {
            case .ok, .refused:
                // sent, or something Simkl will never take: either way it's done
                if !outbox.isEmpty { outbox.removeFirst() }
                persist()
            case .gone, .failed:
                if !outbox.isEmpty { outbox[0].tries += 1 }
                persist()
                return
            }
        }
    }

    // MARK: sync

    /// Brings Simkl's side here. Cheap when nothing moved (one small request).
    func sync(force: Bool = false) async {
        guard connected, !syncing else { return }
        if !force, Date().timeIntervalSince(lastSync) < 60 { return }
        syncing = true
        defer { syncing = false }
        lastSync = .now
        await flush()

        guard let act = await get("/sync/activities") as? [String: Any], let all = act["all"] as? String else {
            lastError = needsReconnect ? "Simkl needs you to link again" : "couldn't reach Simkl"
            return
        }
        if !force, all == UserDefaults.standard.string(forKey: "simkl.all") { lastError = nil; return }

        var list: [MetaPreview] = []
        var seen: Set<String> = []
        var done: Set<String> = []
        var stat: [String: String] = [:]
        var seenAt: [WatchedTitle] = []
        let stamp = ISO8601DateFormatter()
        for bucket in ["shows", "movies", "anime"] {
            // every type has to come back, or nothing here is touched
            guard let v = await get("/sync/all-items/\(bucket)?extended=full&episode_watched_at=yes") else {
                lastError = "couldn't reach Simkl"
                return
            }
            let rows = ((v as? [String: Any])?[bucket] as? [[String: Any]]) ?? []
            for it in rows {
                guard let m = (it["show"] ?? it["movie"]) as? [String: Any], let ids = m["ids"] as? [String: Any],
                      let imdb = ids["imdb"] as? String, imdb.hasPrefix("tt"), let title = m["title"] as? String else { continue }
                let st = it["status"] as? String ?? ""
                let film = bucket == "movies" || (bucket == "anime" && (it["anime_type"] as? String ?? m["anime_type"] as? String) == "movie")
                stat[imdb] = st
                // when you last watched it (for "your month")
                if let at = (it["last_watched_at"] as? String).flatMap({ stamp.date(from: $0) }) {
                    seenAt.append(WatchedTitle(id: imdb, type: film ? .movie : .series, name: title, at: at,
                                               episodes: (it["watched_episodes_count"] as? NSNumber)?.intValue,
                                               minutes: (m["runtime"] as? NSNumber)?.intValue))
                }
                if st == "plantowatch" || (st == "watching" && !film), seen.insert(imdb).inserted {
                    list.append(MetaPreview(id: imdb, type: film ? .movie : .series, name: title, releaseInfo: (m["year"] as? Int).map(String.init)))
                }
                if film {
                    if st == "completed" { done.insert(imdb) }
                } else if bucket == "shows" {
                    // (anime counts episodes its own way: left alone)
                    for s in (it["seasons"] as? [[String: Any]]) ?? [] {
                        guard let sn = s["number"] as? Int, sn > 0 else { continue }
                        for e in (s["episodes"] as? [[String: Any]]) ?? [] {
                            if let en = e["number"] as? Int { done.insert("\(imdb):\(sn):\(en)") }
                        }
                    }
                }
            }
        }
        status = stat
        UserDefaults.standard.set(stat, forKey: "simkl.status")
        History.shared.fromSimkl(seenAt)
        // with changes of ours still waiting, Simkl's list isn't the whole truth yet
        Library.shared.fromSimkl(list: list, watched: done, complete: outbox.isEmpty)

        // where you stopped on your other devices
        if let rows = await get("/sync/playback") as? [[String: Any]] {
            var spots: [Progress] = []
            var ids: [String: Int] = [:]
            let iso = ISO8601DateFormatter()
            for p in rows {
                guard let m = (p["show"] ?? p["movie"] ?? p["anime"]) as? [String: Any], let mi = m["ids"] as? [String: Any],
                      let imdb = mi["imdb"] as? String, imdb.hasPrefix("tt"), let title = m["title"] as? String,
                      let pct = (p["progress"] as? NSNumber)?.doubleValue, pct >= 2, pct < 93 else { continue }
                let film = (p["type"] as? String) == "movie"
                let ep = p["episode"] as? [String: Any]
                let s = (ep?["tvdb_season"] ?? ep?["season"]) as? Int
                let e = (ep?["tvdb_number"] ?? ep?["number"]) as? Int
                if !film && (s == nil || e == nil) { continue }
                let vid = film ? imdb : "\(imdb):\(s!):\(e!)"
                if let id = (p["id"] as? NSNumber)?.intValue { ids[vid] = id }
                let at = (p["paused_at"] as? String).flatMap { iso.date(from: $0) } ?? .distantPast
                spots.append(Progress(meta: MetaPreview(id: imdb, type: film ? .movie : .series, name: title, releaseInfo: (m["year"] as? Int).map(String.init)),
                                      videoId: vid, season: film ? nil : s, episode: film ? nil : e, episodeTitle: nil,
                                      position: 0, duration: 0, updated: at, pct: pct / 100))
            }
            pauses = ids
            Library.shared.fromSimkl(spots: spots)
        }

        UserDefaults.standard.set(all, forKey: "simkl.all")
        lastError = nil
        if name == nil { await whoAmI() }
    }

    // MARK: what you do here, sent to Simkl

    private func ids(_ m: MetaPreview, _ extra: [String: Any] = [:]) -> [String: Any] {
        var e: [String: Any] = ["ids": ["imdb": m.id], "title": m.name]
        for (k, v) in extra { e[k] = v }
        return [m.type == .series ? "shows" : "movies": [e]]
    }

    /// My List changed here: the same on Simkl (Plan to Watch).
    func listed(_ m: MetaPreview, _ on: Bool) {
        guard connected, m.id.hasPrefix("tt") else { return }
        if on {
            // already tracked there (watching, completed…): leave its status alone
            if let s = status[m.id], !s.isEmpty { return }
            queue("/sync/add-to-list", ids(m, ["to": "plantowatch"]))
            status[m.id] = "plantowatch"
        } else if status[m.id] == "plantowatch" {
            // only when it's just sitting in Plan to Watch: for anything you've
            // watched, removing there would erase its history too
            queue("/sync/history/remove", ids(m))
            status[m.id] = nil
        }
        UserDefaults.standard.set(status, forKey: "simkl.status")
    }

    private func episodeBody(_ m: MetaPreview, _ s: Int, _ e: Int) -> [String: Any] {
        ["shows": [["ids": ["imdb": m.id], "title": m.name, "seasons": [["number": s, "episodes": [["number": e]]]]]]]
    }

    /// Finished a film or an episode: into your Simkl history.
    func watched(_ p: Progress) {
        guard connected, p.meta.id.hasPrefix("tt") else { return }
        if let s = p.season, let e = p.episode {
            queue("/sync/history", episodeBody(p.meta, s, e))
        } else {
            queue("/sync/history", ids(p.meta))
        }
        // its "stopped here" on Simkl is over
        if let id = pauses[p.videoId] {
            pauses[p.videoId] = nil
            Task { _ = await send("DELETE", "/sync/playback/\(id)") }
        }
    }

    /// Stopped partway: your other couches can carry on from here. Sent once,
    /// never from the queue (a late one could overwrite a newer spot).
    func paused(_ p: Progress) {
        guard connected, p.meta.id.hasPrefix("tt"), !p.finished else { return }
        let f = p.fraction
        guard f >= 0.02, f < 0.93 else { return }
        var b: [String: Any] = ["progress": (f * 10000).rounded() / 100]
        if let s = p.season, let e = p.episode {
            b["show"] = ["ids": ["imdb": p.meta.id], "title": p.meta.name]
            b["episode"] = ["season": s, "number": e]
        } else if p.meta.type == .movie {
            b["movie"] = ["ids": ["imdb": p.meta.id], "title": p.meta.name]
        } else {
            return
        }
        guard let d = try? JSONSerialization.data(withJSONObject: b) else { return }
        Task { _ = await send("POST", "/scrobble/pause?min_progress=80", d) }
    }
}
