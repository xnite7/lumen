// Scene clips for the billboard and title pages (couch Pro): the couch
// server picks a ClipCafe scene and caches it. Free users see the artwork.

import Foundation

actor Clips {
    static let shared = Clips()
    /// does this title have a clip (the session in the link is fresh each time)
    private var known: [String: Bool] = [:]

    /// A playable clip for this title, or nil (not Pro, none found, or out of clips today).
    func url(for m: MetaPreview) async -> URL? {
        guard await Account.shared.pro, m.id.hasPrefix("tt") else { return nil }
        if known[m.id] == false { return nil }
        guard let tok = await Account.shared.token() else { return nil }
        var q = URLComponents()
        q.queryItems = [.init(name: "title", value: m.name), .init(name: "year", value: m.year)]
        func link() -> URL? {
            var v = q
            v.queryItems?.append(.init(name: "t", value: tok))
            return URL(string: "\(crewURL)/clip/\(m.id)\(v.string ?? "")")
        }
        if known[m.id] == true { return link() }
        var r = URLRequest(url: URL(string: "\(crewURL)/v1/clip/\(m.id)\(q.string ?? "")")!)
        r.setValue("Bearer \(tok)", forHTTPHeaderField: "authorization")
        struct A: Decodable { let clip: Bool? }
        guard let (d, res) = try? await Net.session.data(for: r), (res as? HTTPURLResponse)?.statusCode == 200,
              (try? JSONDecoder().decode(A.self, from: d))?.clip == true else {
            known[m.id] = false
            return nil
        }
        known[m.id] = true
        return link()
    }
}
