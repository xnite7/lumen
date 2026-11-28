// Friends: people you've added by their couch name. Works with a couch
// account (Sign in with Apple) or a Discord sign-in; sits alongside the
// Discord crew. A friend sees what you're watching (if you let them).

import Foundation
import Observation

struct Friend: Identifiable, Hashable {
    var id: String
    var name: String
    var handle: String?
    var avatar: String?
    var watching: CrewMember?
}

@MainActor @Observable
final class Friends {
    static let shared = Friends()
    private(set) var friends: [Friend] = []
    private(set) var incoming: [Friend] = []
    private(set) var outgoing: [Friend] = []
    /// your own couch name (nil until you pick one)
    private(set) var handle: String?
    private(set) var name: String = ""
    private(set) var loaded = false
    private var at = Date.distantPast

    var watching: [CrewMember] { friends.compactMap(\.watching) }

    private init() {}

    /// (status, body); status 0 = couldn't reach the server
    private func call(_ method: String, _ path: String, _ body: [String: Any]? = nil) async -> (Int, [String: Any]) {
        guard Account.shared.isPerson, let tok = await Account.shared.token() else { return (0, [:]) }
        var r = URLRequest(url: URL(string: crewURL + "/v1/me" + path)!)
        r.httpMethod = method
        r.timeoutInterval = 12
        r.cachePolicy = .reloadIgnoringLocalCacheData
        r.setValue("Bearer \(tok)", forHTTPHeaderField: "authorization")
        if let body {
            r.setValue("application/json", forHTTPHeaderField: "content-type")
            r.httpBody = try? JSONSerialization.data(withJSONObject: body)
        }
        guard let (d, res) = try? await Net.session.data(for: r), let h = res as? HTTPURLResponse else { return (0, [:]) }
        return (h.statusCode, ((try? JSONSerialization.jsonObject(with: d)) as? [String: Any]) ?? [:])
    }

    private func friend(_ v: [String: Any]) -> Friend? {
        guard let id = v["id"] as? String else { return nil }
        let name = v["name"] as? String ?? "someone"
        var f = Friend(id: id, name: name, handle: v["handle"] as? String, avatar: v["avatar"] as? String)
        if let w = v["watching"] as? [String: Any], let t = w["title"] as? String, !t.isEmpty {
            f.watching = CrewMember(id: id, name: name, avatar: f.avatar, title: t, episode: (w["episode"] as? String).flatMap { $0.isEmpty ? nil : $0 },
                                    metaId: w["metaId"] as? String, type: w["type"] as? String,
                                    background: (w["background"] as? String).flatMap { $0.isEmpty ? nil : $0 },
                                    paused: v["paused"] as? Bool ?? false, pos: (v["pos"] as? NSNumber)?.doubleValue ?? 0,
                                    duration: (v["duration"] as? NSNumber)?.doubleValue ?? 0)
        }
        return f
    }

    /// Keeps what it had when the server can't be reached.
    func refresh(force: Bool = false) async {
        guard Account.shared.isPerson else {
            friends = []; incoming = []; outgoing = []; handle = nil; loaded = false
            return
        }
        if !force, Date().timeIntervalSince(at) < 20 { return }
        at = .now
        let (code, v) = await call("GET", "/friends")
        guard code == 200 else { return }
        let list = { (k: String) in ((v[k] as? [[String: Any]]) ?? []).compactMap(self.friend) }
        friends = list("friends").sorted { ($0.watching != nil ? 0 : 1, $0.name.lowercased()) < ($1.watching != nil ? 0 : 1, $1.name.lowercased()) }
        incoming = list("incoming")
        outgoing = list("outgoing")
        if let me = v["me"] as? [String: Any] {
            handle = me["handle"] as? String
            name = me["name"] as? String ?? ""
        }
        loaded = true
    }

    /// Your name and couch name. Returns what went wrong, or nil.
    func save(name: String?, handle: String?) async -> String? {
        var b: [String: Any] = [:]
        if let name { b["name"] = name }
        if let handle { b["handle"] = handle }
        let (code, v) = await call("PUT", "/account", b)
        if code == 200, let a = v["account"] as? [String: Any] {
            self.handle = a["handle"] as? String
            self.name = a["name"] as? String ?? self.name
            Account.shared.accountChanged(name: self.name, handle: self.handle)
            return nil
        }
        return v["error"] as? String ?? (code == 0 ? "couldn't reach the couch server" : "that didn't work")
    }

    /// Ask someone to be friends, by couch name. Returns what went wrong, or nil.
    func add(_ handle: String) async -> String? {
        let (code, v) = await call("POST", "/friends/add", ["handle": handle])
        if code == 200 {
            await refresh(force: true)
            return nil
        }
        return v["error"] as? String ?? (code == 0 ? "couldn't reach the couch server" : "that didn't work")
    }

    func accept(_ f: Friend) async {
        _ = await call("POST", "/friends/accept", ["id": f.id])
        await refresh(force: true)
    }

    /// unfriend / turn down / take back
    func remove(_ f: Friend) async {
        _ = await call("POST", "/friends/remove", ["id": f.id])
        await refresh(force: true)
    }

    func block(_ f: Friend) async {
        _ = await call("POST", "/friends/block", ["id": f.id])
        await refresh(force: true)
    }

    func report(_ f: Friend, why: String) async {
        _ = await call("POST", "/report", ["id": f.id, "what": "person", "why": why])
    }
}
