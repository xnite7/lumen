// The catalogue: Cinemeta (Stremio's free IMDb-backed metadata), no key.
// Everything here is cached in memory for the session.

import Foundation

enum Net {
    static let session: URLSession = {
        let c = URLSessionConfiguration.default
        c.urlCache = URLCache(memoryCapacity: 64 << 20, diskCapacity: 512 << 20)
        c.requestCachePolicy = .returnCacheDataElseLoad
        c.timeoutIntervalForRequest = 20
        return URLSession(configuration: c)
    }()

    static func json<T: Decodable>(_ url: URL, as: T.Type = T.self, headers: [String: String] = [:], fresh: Bool = false) async throws -> T {
        var r = URLRequest(url: url)
        if fresh { r.cachePolicy = .reloadIgnoringLocalCacheData }
        for (k, v) in headers { r.setValue(v, forHTTPHeaderField: k) }
        let (d, res) = try await session.data(for: r)
        if let h = res as? HTTPURLResponse, !(200..<300).contains(h.statusCode) {
            throw CouchError.http(h.statusCode)
        }
        return try JSONDecoder().decode(T.self, from: d)
    }
}

enum CouchError: LocalizedError {
    case http(Int)
    case message(String)
    var errorDescription: String? {
        switch self {
        case .http(let c): "the server answered \(c)"
        case .message(let m): m
        }
    }
}

actor Cinemeta {
    static let shared = Cinemeta()
    private let base = "https://v3-cinemeta.strem.io"
    private var metas: [String: MetaDetail] = [:]
    private var catalogs: [String: [MetaPreview]] = [:]

    private struct Catalog: Decodable { let metas: [MetaPreview] }
    private struct One: Decodable { let meta: MetaDetail }

    /// "top" (popular now) or "imdbRating" (best rated); optionally one genre.
    func catalog(_ type: Kind, _ id: String = "top", genre: String? = nil, skip: Int = 0) async throws -> [MetaPreview] {
        var extra: [String] = []
        if let g = genre { extra.append("genre=\(g.addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? g)") }
        if skip > 0 { extra.append("skip=\(skip)") }
        let path = extra.isEmpty ? "\(id).json" : "\(id)/\(extra.joined(separator: "&")).json"
        let key = "\(type.rawValue)/\(path)"
        if let c = catalogs[key] { return c }
        let list = try await Net.json(URL(string: "\(base)/catalog/\(key)")!, as: Catalog.self).metas
        catalogs[key] = list
        return list
    }

    func search(_ q: String) async -> [MetaPreview] {
        let enc = q.addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? q
        async let m = try? Net.json(URL(string: "\(base)/catalog/movie/top/search=\(enc).json")!, as: Catalog.self).metas
        async let s = try? Net.json(URL(string: "\(base)/catalog/series/top/search=\(enc).json")!, as: Catalog.self).metas
        let (a, b) = await (m ?? [], s ?? [])
        // interleave so the best match of each kind is near the top
        var out: [MetaPreview] = []
        for i in 0..<max(a.count, b.count) {
            if i < a.count { out.append(a[i]) }
            if i < b.count { out.append(b[i]) }
        }
        return out
    }

    func meta(_ type: Kind, _ id: String) async throws -> MetaDetail {
        if let m = metas[id] { return m }
        let m = try await Net.json(URL(string: "\(base)/meta/\(type.rawValue)/\(id).json")!, as: One.self).meta
        metas[id] = m
        return m
    }

    /// Titles like this one: the same first genre, best rated, minus itself.
    func similar(_ m: MetaDetail) async -> [MetaPreview] {
        guard let g = m.genres?.first else { return [] }
        let list = (try? await catalog(m.type, "top", genre: g)) ?? []
        return Array(list.filter { $0.id != m.id }.prefix(20))
    }
}

/// Genres Cinemeta knows (for the browse chips).
let genreList = ["Action", "Comedy", "Drama", "Thriller", "Horror", "Sci-Fi", "Animation", "Crime", "Romance", "Adventure", "Mystery", "Fantasy", "Documentary", "Family"]
