// Who you are to couch. Nobody has to sign in: the catalogue, your library
// and your own sources work for everyone. Signing in with Discord connects
// you to the midnite server; the server's coucher role includes couch Pro.
// Pro can also be bought in the app (Store).

import AuthenticationServices
import CryptoKit
import Foundation
import Observation
import Security
import UIKit

/// the sign-in sheet must outlive the call that starts it
nonisolated(unsafe) private var liveAuth: ASWebAuthenticationSession?

let crewURL = "https://midnite-couch.xnite7.workers.dev"
private let discordClient = "1568062298721886290"
private let discordRedirect = "couch://auth/discord"

@MainActor @Observable
final class Account: NSObject {
    static let shared = Account()

    struct Discord: Codable {
        var id: String
        var name: String
        var avatar: String?
        var coucher: Bool
    }

    private(set) var discord: Discord?
    /// bought in the App Store (set by Store)
    var storePro = false { didSet { if oldValue != storePro { Library.shared.refreshWidgets() } } }
    var pro: Bool { storePro || (discord?.coucher ?? false) }
    var crew: Bool { discord?.coucher ?? false }

    private var session: (token: String, exp: Date)?

    override private init() {
        super.init()
        if let d = UserDefaults.standard.data(forKey: "discord.v1") { discord = try? JSONDecoder().decode(Discord.self, from: d) }
    }

    private func keep() {
        UserDefaults.standard.set(try? JSONEncoder().encode(discord), forKey: "discord.v1")
    }

    // MARK: Discord sign-in (PKCE, no secret on the phone)

    func signIn() async throws {
        let verifier = Self.random(64)
        let challenge = Data(SHA256.hash(data: Data(verifier.utf8))).base64URL
        let state = Self.random(24)
        var c = URLComponents(string: "https://discord.com/oauth2/authorize")!
        c.queryItems = [
            .init(name: "response_type", value: "code"), .init(name: "client_id", value: discordClient),
            .init(name: "scope", value: "identify guilds"), .init(name: "redirect_uri", value: discordRedirect),
            .init(name: "state", value: state), .init(name: "code_challenge", value: challenge),
            .init(name: "code_challenge_method", value: "S256"), .init(name: "prompt", value: "consent"),
        ]
        let back: URL = try await withCheckedThrowingContinuation { cont in
            let s = ASWebAuthenticationSession(url: c.url!, callbackURLScheme: "couch") { url, err in
                if let url { cont.resume(returning: url) } else { cont.resume(throwing: err ?? CouchError.message("sign-in was cancelled")) }
            }
            s.presentationContextProvider = self
            s.prefersEphemeralWebBrowserSession = false
            liveAuth = s
            s.start()
        }
        let q = URLComponents(url: back, resolvingAgainstBaseURL: false)?.queryItems ?? []
        guard q.first(where: { $0.name == "state" })?.value == state, let code = q.first(where: { $0.name == "code" })?.value else {
            throw CouchError.message("Discord didn't finish the sign-in")
        }
        let tok = try await discordToken([
            "grant_type": "authorization_code", "code": code, "redirect_uri": discordRedirect, "code_verifier": verifier,
        ])
        try await crewLogin(with: tok)
    }

    func signOut() {
        discord = nil
        session = nil
        Keychain.set("discord.refresh", nil)
        Keychain.set("discord.access", nil)
        keep()
        Library.shared.refreshWidgets()
    }

    private struct Token: Decodable {
        let access_token: String
        let refresh_token: String?
        let expires_in: Double?
    }

    private func discordToken(_ form: [String: String]) async throws -> Token {
        var r = URLRequest(url: URL(string: "https://discord.com/api/oauth2/token")!)
        r.httpMethod = "POST"
        r.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "content-type")
        var all = form
        all["client_id"] = discordClient
        r.httpBody = all.map { "\($0.key)=\($0.value.addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? $0.value)" }.joined(separator: "&").data(using: .utf8)
        let (d, res) = try await Net.session.data(for: r)
        guard (res as? HTTPURLResponse)?.statusCode == 200, let t = try? JSONDecoder().decode(Token.self, from: d) else {
            throw CouchError.message("Discord didn't accept the sign-in")
        }
        Keychain.set("discord.access", t.access_token)
        Keychain.set("discord.access.exp", String(Date().timeIntervalSince1970 + (t.expires_in ?? 3600)))
        if let rt = t.refresh_token { Keychain.set("discord.refresh", rt) }
        return t
    }

    private func discordAccess() async -> String? {
        if let a = Keychain.get("discord.access"), let e = Keychain.get("discord.access.exp").flatMap(Double.init), e > Date().timeIntervalSince1970 + 300 {
            return a
        }
        guard let rt = Keychain.get("discord.refresh") else { return nil }
        return try? await discordToken(["grant_type": "refresh_token", "refresh_token": rt]).access_token
    }

    // MARK: the couch server

    private struct Auth: Decodable {
        let session: String
        let exp: Double
        let id: String?
        let name: String?
        let avatar: String?
        let coucher: Bool?
    }

    private func crewLogin(with tok: Token) async throws {
        let a = try await post(["token": tok.access_token, "open": true])
        discord = Discord(id: a.id ?? "", name: a.name ?? "you", avatar: a.avatar, coucher: a.coucher ?? false)
        keep()
        Library.shared.refreshWidgets()
    }

    private func post(_ body: [String: Any]) async throws -> Auth {
        var r = URLRequest(url: URL(string: crewURL + "/v1/auth")!)
        r.httpMethod = "POST"
        r.setValue("application/json", forHTTPHeaderField: "content-type")
        r.httpBody = try JSONSerialization.data(withJSONObject: body)
        let (d, res) = try await Net.session.data(for: r)
        guard (res as? HTTPURLResponse)?.statusCode == 200, let a = try? JSONDecoder().decode(Auth.self, from: d) else {
            let msg = (try? JSONSerialization.jsonObject(with: d) as? [String: Any])?["error"] as? String
            throw CouchError.message(msg ?? "the couch server said no")
        }
        session = (a.session, Date(timeIntervalSince1970: a.exp))
        return a
    }

    /// A fresh couch-server session: from Discord, else from an App Store
    /// purchase. Nil when you're neither (the free app needs no server).
    func token() async -> String? {
        if let s = session, s.exp > Date().addingTimeInterval(15) { return s.token }
        if discord != nil, let a = await discordAccess(), let r = try? await post(["token": a, "open": true]) {
            // the role can change any time: keep the tier current
            if let c = r.coucher, c != discord?.coucher {
                discord?.coucher = c
                keep()
                Library.shared.refreshWidgets()
            }
            return r.session
        }
        if let jws = await Store.shared.proTransaction(), let r = try? await post(["appstore": jws]) {
            return r.session
        }
        return nil
    }

    /// Check the role again (pull to refresh in Settings).
    func refresh() async {
        session = nil
        _ = await token()
    }

    static func random(_ n: Int) -> String {
        let c = Array("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")
        return String((0..<n).map { _ in c.randomElement()! })
    }
}

extension Account: ASWebAuthenticationPresentationContextProviding {
    nonisolated func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        MainActor.assumeIsolated {
            UIApplication.shared.connectedScenes.compactMap { ($0 as? UIWindowScene)?.keyWindow }.first ?? ASPresentationAnchor()
        }
    }
}

extension Data {
    var base64URL: String {
        base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
    }
}

/// Tokens live in the Keychain, not in preferences.
enum Keychain {
    static func set(_ key: String, _ value: String?) {
        let q: [String: Any] = [kSecClass as String: kSecClassGenericPassword, kSecAttrAccount as String: key, kSecAttrService as String: "couch"]
        SecItemDelete(q as CFDictionary)
        guard let v = value else { return }
        var add = q
        add[kSecValueData as String] = Data(v.utf8)
        add[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
        SecItemAdd(add as CFDictionary, nil)
    }
    static func get(_ key: String) -> String? {
        let q: [String: Any] = [kSecClass as String: kSecClassGenericPassword, kSecAttrAccount as String: key, kSecAttrService as String: "couch",
                                kSecReturnData as String: true, kSecMatchLimit as String: kSecMatchLimitOne]
        var out: AnyObject?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let d = out as? Data else { return nil }
        return String(data: d, encoding: .utf8)
    }
}
