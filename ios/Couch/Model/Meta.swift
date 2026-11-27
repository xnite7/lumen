// What couch shows: titles, episodes and streams, in the Stremio addon
// protocol's shape (Cinemeta for the catalogue, your sources for streams).

import Foundation

enum Kind: String, Codable, Hashable {
    case movie, series
    var label: String { self == .movie ? "Movies" : "Series" }
}

/// A title in a row or search result.
struct MetaPreview: Codable, Hashable, Identifiable {
    let id: String
    let type: Kind
    let name: String
    var poster: String?
    var background: String?
    var logo: String?
    var description: String?
    var releaseInfo: String?
    var imdbRating: String?
    var genres: [String]?

    /// Metahub serves art for every IMDb id, even when the catalogue left it out.
    var posterURL: URL? { URL(string: poster ?? "https://images.metahub.space/poster/medium/\(id)/img") }
    var backdropURL: URL? { URL(string: background ?? "https://images.metahub.space/background/medium/\(id)/img") }
    var logoURL: URL? { URL(string: logo ?? "https://images.metahub.space/logo/medium/\(id)/img") }
    var year: String { String((releaseInfo ?? "").prefix(4)) }
}

struct Trailer: Codable, Hashable {
    let title: String?
    let ytId: String?
}

/// An episode (or, for a movie, nothing: movies have no videos).
struct Episode: Codable, Hashable, Identifiable {
    let id: String
    var title: String?
    var name: String?
    var season: Int?
    var episode: Int?
    var released: String?
    var thumbnail: String?
    var overview: String?
    var description: String?

    var label: String { title ?? name ?? "Episode \(episode ?? 0)" }
    var about: String? { overview ?? description }
    var code: String { "S\(season ?? 0) · E\(episode ?? 0)" }
    var releaseDate: Date? { released.flatMap { ISO8601DateFormatter.couch.date(from: $0) } }
    var isOut: Bool { (releaseDate ?? .distantPast) <= .now }
}

/// A title's full page.
struct MetaDetail: Codable, Hashable, Identifiable {
    let id: String
    let type: Kind
    let name: String
    var poster: String?
    var background: String?
    var logo: String?
    var description: String?
    var releaseInfo: String?
    var year: String?
    var imdbRating: String?
    var runtime: String?
    var genres: [String]?
    var cast: [String]?
    var director: [String]?
    var writer: [String]?
    var trailerStreams: [Trailer]?
    var videos: [Episode]?
    var awards: String?
    var country: String?

    var preview: MetaPreview {
        MetaPreview(id: id, type: type, name: name, poster: poster, background: background, logo: logo,
                    description: description, releaseInfo: releaseInfo ?? year, imdbRating: imdbRating, genres: genres)
    }
    var backdropURL: URL? { preview.backdropURL }
    var logoURL: URL? { preview.logoURL }

    /// Seasons in order, specials (season 0) last.
    var seasons: [Int] {
        let s = Set((videos ?? []).compactMap(\.season))
        return s.filter { $0 > 0 }.sorted() + (s.contains(0) ? [0] : [])
    }
    func episodes(_ season: Int) -> [Episode] {
        (videos ?? []).filter { $0.season == season }.sorted { ($0.episode ?? 0) < ($1.episode ?? 0) }
    }
    /// The episode after this one (next in the season, else the next season's first).
    func after(_ id: String) -> Episode? {
        let all = seasons.filter { $0 > 0 }.flatMap { episodes($0) }
        guard let i = all.firstIndex(where: { $0.id == id }), i + 1 < all.count else { return nil }
        return all[i + 1].isOut ? all[i + 1] : nil
    }
}

/// A playable stream from a source (Stremio addon).
struct Stream: Codable, Hashable, Identifiable {
    var name: String?
    var title: String?
    var description: String?
    var url: String?
    var infoHash: String?
    var behaviorHints: Hints?

    struct Hints: Codable, Hashable {
        var filename: String?
        var bingeGroup: String?
        var notWebReady: Bool?
        var proxyHeaders: Proxy?
        var videoSize: Int64?
    }
    struct Proxy: Codable, Hashable {
        var request: [String: String]?
    }

    var id: String { url ?? infoHash ?? (name ?? "") + (title ?? "") }
    /// couch on the App Store plays links only (no torrent engine).
    var playable: Bool { url.map { $0.hasPrefix("http") } ?? false }
    var text: String { [title, description].compactMap { $0 }.joined(separator: "\n") }
    var label: String { (name ?? "").replacingOccurrences(of: "\n", with: " ") }

    /// 2160p / 1080p / 720p, from the name or the text
    var quality: String? {
        let s = "\(name ?? "") \(text)".lowercased()
        for (k, v) in [("2160", "4K"), ("4k", "4K"), ("1080", "1080p"), ("720", "720p"), ("480", "480p")] where s.contains(k) { return v }
        return nil
    }
    var hdr: String? {
        let s = "\(name ?? "") \(text)".lowercased()
        if s.contains("dolby vision") || s.contains(" dv ") || s.contains(".dv.") || s.contains("dovi") { return "Dolby Vision" }
        if s.contains("hdr") { return "HDR" }
        return nil
    }
    var size: String? {
        if let b = behaviorHints?.videoSize, b > 0 { return ByteCountFormatter.string(fromByteCount: b, countStyle: .file) }
        let r = text.range(of: #"\d+(\.\d+)?\s?(GB|MB)"#, options: .regularExpression)
        return r.map { String(text[$0]) }
    }
    var rank: Int {
        var r = 0
        switch quality { case "4K": r += 40; case "1080p": r += 30; case "720p": r += 20; default: break }
        if hdr != nil { r += 3 }
        return r
    }
}

/// A source's manifest (only the parts couch reads).
struct Manifest: Codable, Hashable {
    let id: String
    let name: String
    var description: String?
    var logo: String?
    var types: [String]?
    var resources: [Resource]?

    enum Resource: Codable, Hashable {
        case name(String)
        case full(name: String, types: [String]?, idPrefixes: [String]?)

        init(from d: Decoder) throws {
            if let s = try? d.singleValueContainer().decode(String.self) { self = .name(s); return }
            struct R: Codable { let name: String; let types: [String]?; let idPrefixes: [String]? }
            let r = try R(from: d)
            self = .full(name: r.name, types: r.types, idPrefixes: r.idPrefixes)
        }
        func encode(to e: Encoder) throws {
            var c = e.singleValueContainer()
            switch self {
            case .name(let s): try c.encode(s)
            case .full(let n, _, _): try c.encode(n)
            }
        }
        var name: String { switch self { case .name(let s): s; case .full(let n, _, _): n } }
    }

    var givesStreams: Bool { (resources ?? []).contains { $0.name == "stream" } }
}

extension ISO8601DateFormatter {
    static let couch: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()
}
