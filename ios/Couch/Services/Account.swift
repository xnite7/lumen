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

let crewURL = "https://midnite.club"
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

    /// A couch account (Sign in with Apple): for friends and the leaderboards
    /// without Discord. Its token is couch's own, kept in iCloud Keychain.
    struct Couch: Codable {
        var id: String
        var name: String
        var handle: String?
    }
    private(set) var couch: Couch? {
        didSet { UserDefaults.standard.set(try? JSONEncoder().encode(couch), forKey: "couch.v1") }
    }
    /// who you are to other people: the Discord sign-in if there is one, else the couch account
    var personId: String? { discord?.id ?? couch?.id }
    var isPerson: Bool { personId != nil }
    /// bought in the App Store (set by Store)
    var storePro = false { didSet { if oldValue != storePro { Library.shared.refreshWidgets() } } }
    var pro: Bool { storePro || (discord?.coucher ?? false) }
    var crew: Bool { discord?.coucher ?? false }

    private var session: (token: String, exp: Date)?

    override private init() {
        super.init()
        if let d = UserDefaults.standard.data(forKey: "discord.v1") { discord = try? JSONDecoder().decode(Discord.self, from: d) }
        if let d = UserDefaults.standard.data(forKey: "couch.v1") { couch = try? JSONDecoder().decode(Couch.self, from: d) }
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
        // already have a couch account (Apple): the two become one person
        if Keychain.get("couch.account") != nil { await link() }
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

    private var discordRefreshing: Task<String?, Never>?

    private func discordFresh() -> String? {
        if let a = Keychain.get("discord.access"), let e = Keychain.get("discord.access.exp").flatMap(Double.init), e > Date().timeIntervalSince1970 + 300 {
            return a
        }
        return nil
    }

    /// Discord hands out a new refresh token each time and retires the old
    /// one, so: one refresh at a time, and if it's turned down, look again
    /// (another device on your Apple ID may have refreshed a moment ago).
    private func discordAccess() async -> String? {
        if let a = discordFresh() { return a }
        if let t = discordRefreshing { return await t.value }
        let t = Task { () -> String? in
            guard let rt = Keychain.get("discord.refresh") else { return nil }
            if let a = try? await self.discordToken(["grant_type": "refresh_token", "refresh_token": rt]).access_token { return a }
            try? await Task.sleep(for: .seconds(2))
            if let a = self.discordFresh() { return a }
            if let newer = Keychain.get("discord.refresh"), newer != rt {
                return try? await self.discordToken(["grant_type": "refresh_token", "refresh_token": newer]).access_token
            }
            return nil
        }
        discordRefreshing = t
        let a = await t.value
        discordRefreshing = nil
        return a
    }

    // MARK: the couch server

    private struct Auth: Decodable {
        let session: String
        let exp: Double
        let id: String?
        let name: String?
        let avatar: String?
        let coucher: Bool?
        let handle: String?
        /// couch's own long-lived token (only when signing in with Apple)
        let account: String?
    }

    // MARK: couch account (Sign in with Apple)

    /// Apple said who you are: trade that for a couch account.
    func signInApple(_ cred: ASAuthorizationAppleIDCredential) async throws {
        guard let d = cred.identityToken, let tok = String(data: d, encoding: .utf8) else {
            throw CouchError.message("Apple didn't finish the sign-in")
        }
        var body: [String: Any] = ["apple": tok]
        // (a one-time code: lets the server end couch's access if you delete the account)
        if let c = cred.authorizationCode, let code = String(data: c, encoding: .utf8) { body["code"] = code }
        // (Apple only gives the name the first time)
        if let n = cred.fullName {
            let name = PersonNameComponentsFormatter().string(from: n).trimmingCharacters(in: .whitespaces)
            if !name.isEmpty { body["name"] = name }
        }
        let a = try await post(body)
        guard let t = a.account, let id = a.id else { throw CouchError.message("the couch server said no") }
        Keychain.set("couch.account", t)
        couch = Couch(id: id, name: a.name ?? "someone", handle: a.handle)
        // already signed in with Discord: the two become one person
        if discord != nil, let e = await link() { throw CouchError.message(e) }
    }

    /// both sign-ins are joined on the server (one person, one friends list)
    var linked: Bool { discord != nil && couch != nil && UserDefaults.standard.string(forKey: "couch.linked") == discord?.id }

    /// Join the Apple sign-in to the Discord one. Returns what went wrong, or nil.
    @discardableResult
    func link() async -> String? {
        guard let d = discord, let t = Keychain.get("couch.account") else { return nil }
        session = nil
        guard let tok = await token() else { return "couldn't reach the couch server" }
        var r = URLRequest(url: URL(string: crewURL + "/v1/me/account/link")!)
        r.httpMethod = "POST"
        r.setValue("Bearer \(tok)", forHTTPHeaderField: "authorization")
        r.setValue("application/json", forHTTPHeaderField: "content-type")
        r.httpBody = try? JSONSerialization.data(withJSONObject: ["account": t])
        guard let (data, res) = try? await Net.session.data(for: r), let h = res as? HTTPURLResponse else { return "couldn't reach the couch server" }
        if h.statusCode == 200 {
            UserDefaults.standard.set(d.id, forKey: "couch.linked")
            _ = await couchSession()
            return nil
        }
        return ((try? JSONSerialization.jsonObject(with: data)) as? [String: Any])?["error"] as? String ?? "couldn't link them"
    }

    /// what the server now says about your account (after a rename)
    func accountChanged(name: String, handle: String?) {
        guard couch != nil else { return }
        couch?.name = name
        couch?.handle = handle
        session = nil
    }

    /// Leave this account on every device (the token stops working).
    func signOutCouch() async {
        if let tok = await token() {
            var r = URLRequest(url: URL(string: crewURL + "/v1/me/account/signout")!)
            r.httpMethod = "POST"
            r.setValue("Bearer \(tok)", forHTTPHeaderField: "authorization")
            _ = try? await Net.session.data(for: r)
        }
        forgetCouch()
    }

    /// Delete the account for good: name, handle, friends. False if the server couldn't be reached.
    func deleteAccount() async -> Bool {
        guard let tok = await token() else { return false }
        var r = URLRequest(url: URL(string: crewURL + "/v1/me/account")!)
        r.httpMethod = "DELETE"
        r.setValue("Bearer \(tok)", forHTTPHeaderField: "authorization")
        guard let (_, res) = try? await Net.session.data(for: r), (res as? HTTPURLResponse)?.statusCode == 200 else { return false }
        forgetCouch()
        return true
    }

    private func forgetCouch() {
        UserDefaults.standard.removeObject(forKey: "couch.linked")
        Keychain.set("couch.account", nil)
        couch = nil
        session = nil
    }

    /// A session from the couch account's own token (with Pro if it was bought here).
    private func couchSession() async -> String? {
        guard let t = Keychain.get("couch.account") else { return nil }
        var body: [String: Any] = ["account": t]
        if let jws = await Store.shared.proTransaction() { body["appstore"] = jws }
        do {
            let a = try await post(body)
            if let id = a.id { couch = Couch(id: id, name: a.name ?? couch?.name ?? "someone", handle: a.handle) }
            return a.session
        } catch let e as CouchError {
            // the server doesn't know this token any more (signed out or deleted elsewhere)
            if case .message(let m) = e, m == "sign in again" { forgetCouch() }
            return nil
        } catch {
            return nil // offline: still signed in
        }
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
    func token(guest: Bool = false) async -> String? {
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
        if let s = await couchSession() { return s }
        if let jws = await Store.shared.proTransaction(), let r = try? await post(["appstore": jws]) {
            return r.session
        }
        // nobody: a guest session for this device (the scenes game only)
        if guest, let r = try? await post(["guest": deviceId]) { return r.session }
        return nil
    }

    /// has a couch-server identity beyond a guest's
    var known: Bool { discord != nil || storePro || couch != nil }
    let deviceId: String = {
        if let id = UserDefaults.standard.string(forKey: "device.id") { return id }
        let id = String((0..<16).map { _ in "abcdefghijklmnopqrstuvwxyz0123456789".randomElement()! })
        UserDefaults.standard.set(id, forKey: "device.id")
        return id
    }()

    /// A new device on the same Apple ID: the Discord link is already in
    /// iCloud Keychain, so sign in quietly with it.
    func restore() async {
        // (a couch account whose token came over in iCloud Keychain)
        if discord == nil, couch == nil, Keychain.get("couch.account") != nil { _ = await couchSession() }
        // both here but never joined (signed in before linking existed, or on two devices)
        if discord != nil, Keychain.get("couch.account") != nil, !linked {
            if couch == nil { _ = await couchSession() }
            await link()
        }
        guard discord == nil, Keychain.get("discord.refresh") != nil, let a = await discordAccess(),
              let r = try? await post(["token": a, "open": true]) else { return }
        discord = Discord(id: r.id ?? "", name: r.name ?? "you", avatar: r.avatar, coucher: r.coucher ?? false)
        keep()
        Library.shared.refreshWidgets()
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

/// Tokens live in the Keychain, not in preferences: in iCloud Keychain when
/// it's on, so your Discord and Simkl links follow your Apple ID to your
/// other devices (and stay on this one when it's off).
enum Keychain {
    private static func base(_ key: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrAccount as String: key, kSecAttrService as String: "couch"]
    }
    static func set(_ key: String, _ value: String?) {
        var all = base(key)
        all[kSecAttrSynchronizable as String] = kSecAttrSynchronizableAny
        SecItemDelete(all as CFDictionary)
        guard let v = value else { return }
        var add = base(key)
        add[kSecValueData as String] = Data(v.utf8)
        add[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
        add[kSecAttrSynchronizable as String] = true
        if SecItemAdd(add as CFDictionary, nil) != errSecSuccess {
            // no iCloud Keychain: keep it on this device
            add[kSecAttrSynchronizable as String] = false
            SecItemAdd(add as CFDictionary, nil)
        }
    }
    static func get(_ key: String) -> String? {
        var q = base(key)
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        q[kSecAttrSynchronizable as String] = kSecAttrSynchronizableAny
        var out: AnyObject?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let d = out as? Data else { return nil }
        return String(data: d, encoding: .utf8)
    }
}
