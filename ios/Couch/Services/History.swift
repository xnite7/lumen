// What you watched and when: from Simkl (your whole history, every couch)
// and from this device. Feeds "your month".

import Foundation
import Observation

struct WatchedTitle: Codable, Hashable, Identifiable {
    var id: String
    var type: Kind
    var name: String
    var at: Date
    var episodes: Int?
    var minutes: Int?   // one film / one episode
}

@MainActor @Observable
final class History {
    static let shared = History()
    private(set) var simkl: [WatchedTitle] = []
    private(set) var here: [String: WatchedTitle] = [:]

    private struct Disk: Codable { var simkl: [WatchedTitle]; var here: [String: WatchedTitle] }
    private var url: URL {
        let d = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        try? FileManager.default.createDirectory(at: d, withIntermediateDirectories: true)
        return d.appendingPathComponent("history.json")
    }
    private var lastSave = Date.distantPast

    private init() {
        if let d = try? Data(contentsOf: url), let l = try? JSONDecoder().decode(Disk.self, from: d) {
            simkl = l.simkl
            here = l.here
        }
    }
    private func save() {
        lastSave = .now
        if let d = try? JSONEncoder().encode(Disk(simkl: simkl, here: here)) { try? d.write(to: url, options: .atomic) }
    }

    func fromSimkl(_ list: [WatchedTitle]) {
        guard list != simkl else { return }
        simkl = list
        save()
    }

    /// something was played on this device (called as the player saves its spot)
    func watchedHere(_ p: Progress) {
        guard p.position > 120 || p.finished else { return }
        here[p.meta.id] = WatchedTitle(id: p.meta.id, type: p.meta.type, name: p.meta.name, at: .now, episodes: nil, minutes: p.duration > 0 ? Int(p.duration / 60) : nil)
        // (the player reports every few seconds: don't write the file each time)
        if Date().timeIntervalSince(lastSave) > 60 { save() }
    }

    /// Everything watched in a month (Simkl's word first, this device's for the rest), newest first.
    func month(_ m: Date) -> [WatchedTitle] {
        let cal = Calendar.current
        var by: [String: WatchedTitle] = [:]
        for w in Array(here.values) + simkl where cal.isDate(w.at, equalTo: m, toGranularity: .month) { by[w.id] = w }
        return by.values.sorted { $0.at > $1.at }
    }
}
