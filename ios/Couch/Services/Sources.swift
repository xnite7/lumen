// Your sources: the servers couch plays from. couch ships with none; you add
// a server's address (any Stremio-compatible addon or media server link).
// Streams are links only: couch has no torrent engine.

import Foundation
import Observation

struct Source: Codable, Hashable, Identifiable {
    var base: String          // https://host/path (no /manifest.json)
    var manifest: Manifest
    var on: Bool = true
    var id: String { base }
    var name: String { manifest.name }
    var host: String { URL(string: base)?.host ?? base }
}

@MainActor @Observable
final class Sources {
    static let shared = Sources()
    private(set) var list: [Source] = []
    private let key = "sources.v1"

    private init() {
        if let d = UserDefaults.standard.data(forKey: key), let l = try? JSONDecoder().decode([Source].self, from: d) { list = l }
    }

    var active: [Source] { list.filter(\.on) }
    var any: Bool { !active.isEmpty }

    private func save() {
        if let d = try? JSONEncoder().encode(list) { UserDefaults.standard.set(d, forKey: key) }
    }

    /// Turn whatever was pasted into the server's base address.
    static func normalise(_ raw: String) -> String? {
        var s = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if s.hasPrefix("stremio://") { s = "https://" + s.dropFirst("stremio://".count) }
        if !s.contains("://") { s = "https://" + s }
        if s.hasSuffix("/manifest.json") { s = String(s.dropLast("/manifest.json".count)) }
        while s.hasSuffix("/") { s.removeLast() }
        guard let u = URL(string: s), u.scheme?.hasPrefix("http") == true, u.host != nil else { return nil }
        return s
    }

    /// Check the server answers like a source, then keep it.
    func add(_ raw: String) async throws -> Source {
        guard let base = Self.normalise(raw) else { throw CouchError.message("that doesn't look like a link") }
        let m: Manifest
        do {
            m = try await Net.json(URL(string: base + "/manifest.json")!, as: Manifest.self, fresh: true)
        } catch {
            throw CouchError.message("couldn't reach that server (\(error.localizedDescription))")
        }
        guard m.givesStreams else { throw CouchError.message("\(m.name) doesn't give streams") }
        let s = Source(base: base, manifest: m)
        list.removeAll { $0.base == base }
        list.append(s)
        save()
        return s
    }

    func remove(_ s: Source) {
        list.removeAll { $0.id == s.id }
        save()
    }
    func toggle(_ s: Source) {
        guard let i = list.firstIndex(where: { $0.id == s.id }) else { return }
        list[i].on.toggle()
        save()
    }
    func move(from: IndexSet, to: Int) {
        list.move(fromOffsets: from, toOffset: to)
        save()
    }

    /// Every active source at once; each source's streams arrive as they come.
    func streams(_ type: Kind, _ id: String) -> AsyncStream<(Source, Result<[Stream], Error>)> {
        let sources = active
        return AsyncStream { cont in
            let task = Task {
                await withTaskGroup(of: (Source, Result<[Stream], Error>).self) { g in
                    for s in sources {
                        g.addTask {
                            struct R: Decodable { let streams: [Stream] }
                            let enc = id.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? id
                            do {
                                let r = try await Net.json(URL(string: "\(s.base)/stream/\(type.rawValue)/\(enc).json")!, as: R.self, fresh: true)
                                return (s, .success(r.streams.filter(\.playable)))
                            } catch {
                                return (s, .failure(error))
                            }
                        }
                    }
                    for await r in g { cont.yield(r) }
                }
                cont.finish()
            }
            cont.onTermination = { _ in task.cancel() }
        }
    }

    /// The best stream (for "Play" without opening the list): highest quality
    /// from the first source in your order that has any.
    func best(_ type: Kind, _ id: String, like previous: Stream? = nil) async -> Stream? {
        var bySource: [String: [Stream]] = [:]
        for await (s, r) in streams(type, id) {
            if case .success(let l) = r { bySource[s.id] = l }
        }
        let ordered = active.flatMap { bySource[$0.id] ?? [] }
        // same release group as the last episode keeps quality/audio steady
        if let g = previous?.behaviorHints?.bingeGroup, let m = ordered.first(where: { $0.behaviorHints?.bingeGroup == g }) { return m }
        return ordered.max { $0.rank < $1.rank }
    }
}
