// "scenes": couch's guess-the-movie game. The couch server runs it (which
// scene, what you've revealed, the score, the leaderboards); this is the
// line to it. Anyone can play (guests too); Discord sign-ins are ranked.

import Foundation

struct GameRow: Codable, Hashable, Identifiable {
    let id: String
    let name: String
    let avatar: String?
    let score: Int
}

struct GameHome: Codable {
    struct Daily: Codable { let played: Bool; let score: Int?; let results: [Int]? }
    struct Boards: Codable { let daily: [GameRow]; let runs: [GameRow] }
    struct Ranks: Codable { let daily: Int?; let runs: Int? }
    let day: String
    let nextIn: Int
    let daily: Daily
    let streak: Int
    let bestStreak: Int
    let bestRun: Int
    let ranked: Bool
    let boards: Boards
    let ranks: Ranks
}

struct GameRound: Codable, Identifiable {
    struct Option: Codable, Hashable { let title: String; let year: String }
    let game: String
    let mode: String
    let n: Int
    let total: Int
    let options: [Option]
    let stage: Int
    let stages: [Double]
    let points: [Int]
    let clip: String
    let score: Int
    let lives: Int?
    let level: Int
    var id: String { "\(game)-\(n)" }
}

struct GameDone: Codable {
    let mode: String
    let score: Int
    let right: Int
    let rounds: Int
    let results: [Int]
    let streak: Int
    let bestRun: Int
    let rank: Int?
}

struct GameAnswer: Codable {
    struct Answer: Codable { let index: Int; let title: String; let year: String; let poster: String?; let id: String }
    let correct: Bool
    let late: Bool?
    let gained: Int
    let answer: Answer
    let score: Int
    let lives: Int?
    let next: GameRound?
    let done: GameDone?
}

enum GameAPI {
    private static func call<T: Decodable>(_ method: String, _ path: String, _ body: [String: Any]? = nil) async throws -> T {
        guard let tok = await Account.shared.token(guest: true) else { throw CouchError.message("couldn't reach the couch server") }
        var r = URLRequest(url: URL(string: crewURL + "/v1/game" + path)!)
        r.httpMethod = method
        r.cachePolicy = .reloadIgnoringLocalCacheData
        r.setValue("Bearer \(tok)", forHTTPHeaderField: "authorization")
        if let body {
            r.setValue("application/json", forHTTPHeaderField: "content-type")
            r.httpBody = try JSONSerialization.data(withJSONObject: body)
        }
        let (d, res) = try await Net.session.data(for: r)
        guard (res as? HTTPURLResponse)?.statusCode == 200 else {
            let msg = (try? JSONSerialization.jsonObject(with: d) as? [String: Any])?["error"] as? String
            throw CouchError.message(msg ?? "the couch server said no")
        }
        return try JSONDecoder().decode(T.self, from: d)
    }

    static func home() async throws -> GameHome { try await call("GET", "/home") }
    static func start(daily: Bool) async throws -> GameRound { try await call("POST", "/start", ["mode": daily ? "daily" : "endless"]) }
    static func reveal(_ game: String) async throws -> Int {
        struct R: Decodable { let stage: Int }
        return try await (call("POST", "/reveal", ["game": game]) as R).stage
    }
    static func answer(_ game: String, pick: Int) async throws -> GameAnswer { try await call("POST", "/answer", ["game": game, "pick": pick]) }
    static func quit(_ game: String) async {
        struct R: Decodable { let done: GameDone? }
        _ = try? await (call("POST", "/quit", ["game": game]) as R)
    }
    /// the round's scene (the session rides in the link: a player can't send headers)
    static func clip(_ round: GameRound) async -> URL? {
        guard let tok = await Account.shared.token(guest: true) else { return nil }
        return URL(string: "\(crewURL)\(round.clip)?t=\(tok)")
    }
}
