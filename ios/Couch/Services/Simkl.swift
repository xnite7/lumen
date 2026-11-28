// Simkl (optional): your watchlist and what you've watched, the same on
// every couch. Signing in is PKCE in the system's sign-in sheet; the tokens
// sit in your iCloud Keychain, so the link follows your Apple ID.
//
// What couch does with it: brings your Plan to Watch and Watching into My
// List, adds / removes there when you do here, and marks what you finish.

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
    var connected: Bool { Keychain.get("simkl.access") != nil }

    override private init() { super.init() }

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
        await whoAmI()
        await sync()
    }

    func disconnect() {
        Keychain.set("simkl.access", nil)
        Keychain.set("simkl.refresh", nil)
        name = nil
        UserDefaults.standard.removeObject(forKey: "simkl.name")
    }

    private func tokens(_ form: [String: String]) async throws {
        var r = URLRequest(url: URL(string: api + "/oauth2/token")!)
        r.httpMethod = "POST"
        r.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "content-type")
        r.httpBody = form.map { "\($0.key)=\($0.value.addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? $0.value)" }.joined(separator: "&").data(using: .utf8)
        let (d, _) = try await Net.session.data(for: r)
        guard let v = try? JSONSerialization.jsonObject(with: d) as? [String: Any], let access = v["access_token"] as? String else {
            throw CouchError.message("Simkl didn't accept the sign-in")
        }
        Keychain.set("simkl.access", access)
        if let rt = v["refresh_token"] as? String { Keychain.set("simkl.refresh", rt) }
    }

    // MARK: requests

    private func call(_ method: String, _ path: String, _ body: Any? = nil, retry: Bool = true) async -> Any? {
        guard let tok = Keychain.get("simkl.access") else { return nil }
        let v = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "1"
        let sep = path.contains("?") ? "&" : "?"
        var r = URLRequest(url: URL(string: "\(api)\(path)\(sep)client_id=\(clientId)&app-name=couch&app-version=\(v)")!)
        r.httpMethod = method
        r.cachePolicy = .reloadIgnoringLocalCacheData
        r.setValue("Bearer \(tok)", forHTTPHeaderField: "authorization")
        r.setValue(clientId, forHTTPHeaderField: "simkl-api-key")
        r.setValue("application/json", forHTTPHeaderField: "content-type")
        r.setValue("midnite-couch (+https://github.com/xnite7) couch-ios/\(v)", forHTTPHeaderField: "user-agent")
        if let body { r.httpBody = try? JSONSerialization.data(withJSONObject: body) }
        guard let (d, res) = try? await Net.session.data(for: r), let h = res as? HTTPURLResponse else { return nil }
        if h.statusCode == 401, retry, let rt = Keychain.get("simkl.refresh"),
           (try? await tokens(["grant_type": "refresh_token", "client_id": clientId, "refresh_token": rt])) != nil {
            return await call(method, path, body, retry: false)
        }
        guard (200..<300).contains(h.statusCode) else { return nil }
        return (try? JSONSerialization.jsonObject(with: d)) ?? [String: Any]()
    }

    private func whoAmI() async {
        if let v = await call("POST", "/users/settings") as? [String: Any], let u = v["user"] as? [String: Any], let n = u["name"] as? String {
            name = n
            UserDefaults.standard.set(n, forKey: "simkl.name")
        }
    }

    // MARK: sync

    /// Plan to Watch and Watching, into My List (nothing is removed here).
    func sync() async {
        guard connected, !syncing else { return }
        syncing = true
        defer { syncing = false }
        var found: [MetaPreview] = []
        for (path, key, kind) in [("shows", "shows", Kind.series), ("movies", "movies", Kind.movie)] {
            for status in ["watching", "plantowatch"] where !(kind == .movie && status == "watching") {
                guard let v = await call("GET", "/sync/all-items/\(path)/\(status)") as? [String: Any], let items = v[key] as? [[String: Any]] else { continue }
                for it in items {
                    guard let m = (it["show"] ?? it["movie"]) as? [String: Any], let ids = m["ids"] as? [String: Any],
                          let imdb = ids["imdb"] as? String, imdb.hasPrefix("tt"), let title = m["title"] as? String else { continue }
                    found.append(MetaPreview(id: imdb, type: kind, name: title, releaseInfo: (m["year"] as? Int).map(String.init)))
                }
            }
        }
        Library.shared.merge(found)
        if name == nil { await whoAmI() }
    }

    private func item(_ m: MetaPreview) -> [String: Any] {
        [m.type == .series ? "shows" : "movies": [["ids": ["imdb": m.id], "title": m.name]]]
    }

    /// My List changed here: the same on Simkl (Plan to Watch).
    func listed(_ m: MetaPreview, _ on: Bool) {
        guard connected, m.id.hasPrefix("tt") else { return }
        Task {
            if on {
                var b = item(m)
                let k = m.type == .series ? "shows" : "movies"
                b[k] = [["ids": ["imdb": m.id], "title": m.name, "to": "plantowatch"]]
                _ = await call("POST", "/sync/add-to-list", b)
            }
            // (removing here doesn't touch Simkl: that would erase its history too)
        }
    }

    /// Finished a film or an episode: into your Simkl history.
    func watched(_ p: Progress) {
        guard connected, p.meta.id.hasPrefix("tt") else { return }
        Task {
            if let s = p.season, let e = p.episode {
                _ = await call("POST", "/sync/history", ["shows": [["ids": ["imdb": p.meta.id], "title": p.meta.name, "seasons": [["number": s, "episodes": [["number": e]]]]]]])
            } else {
                _ = await call("POST", "/sync/history", item(p.meta))
            }
        }
    }
}
