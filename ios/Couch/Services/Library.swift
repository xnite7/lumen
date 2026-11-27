// Your library, kept on this device: My List, where you stopped in each
// title, and what you finished. Feeds Continue Watching and the widgets.

import Foundation
import Observation
import UIKit
import WidgetKit

struct Progress: Codable, Hashable {
    var meta: MetaPreview
    var videoId: String       // tt… for a movie, tt…:s:e for an episode
    var season: Int?
    var episode: Int?
    var episodeTitle: String?
    var position: Double
    var duration: Double
    var updated: Date

    var fraction: Double { duration > 0 ? min(1, position / duration) : 0 }
    var finished: Bool { duration > 0 && (duration - position < 180 || fraction > 0.93) }
    var subtitle: String {
        if let s = season, let e = episode { return "S\(s) · E\(e)" + (episodeTitle.map { "  \($0)" } ?? "") }
        return meta.year
    }
    var left: String {
        let m = Int(max(0, duration - position) / 60)
        return m >= 60 ? "\(m / 60)h \(m % 60)m left" : "\(max(1, m))m left"
    }
}

@MainActor @Observable
final class Library {
    static let shared = Library()

    private(set) var myList: [MetaPreview] = []
    /// last spot per title (meta id)
    private(set) var progress: [String: Progress] = [:]
    /// finished video ids (movies and episodes)
    private(set) var watched: Set<String> = []

    private struct Disk: Codable {
        var myList: [MetaPreview]
        var progress: [String: Progress]
        var watched: Set<String>
    }
    private var url: URL {
        let d = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        try? FileManager.default.createDirectory(at: d, withIntermediateDirectories: true)
        return d.appendingPathComponent("library.json")
    }

    private init() {
        if let d = try? Data(contentsOf: url), let l = try? JSONDecoder().decode(Disk.self, from: d) {
            myList = l.myList
            progress = l.progress
            watched = l.watched
        }
    }

    private func save() {
        let disk = Disk(myList: myList, progress: progress, watched: watched)
        if let d = try? JSONEncoder().encode(disk) { try? d.write(to: url, options: .atomic) }
        refreshWidgets()
    }

    // MARK: My List

    func inList(_ id: String) -> Bool { myList.contains { $0.id == id } }
    func toggleList(_ m: MetaPreview) {
        if inList(m.id) { myList.removeAll { $0.id == m.id } } else { myList.insert(m, at: 0) }
        save()
    }

    // MARK: progress

    /// Titles you're partway through, newest first.
    var continueWatching: [Progress] {
        progress.values.filter { !$0.finished && $0.position > 60 }.sorted { $0.updated > $1.updated }
    }

    func resume(_ id: String) -> Progress? { progress[id] }
    func isWatched(_ videoId: String) -> Bool { watched.contains(videoId) }

    func record(_ p: Progress) {
        var p = p
        p.updated = .now
        if p.finished {
            watched.insert(p.videoId)
            if p.season == nil { progress[p.meta.id] = nil } else { progress[p.meta.id] = p }
        } else {
            progress[p.meta.id] = p
        }
        save()
    }

    func markWatched(_ videoId: String, _ on: Bool) {
        if on { watched.insert(videoId) } else { watched.remove(videoId) }
        save()
    }
    func forget(_ metaId: String) {
        progress[metaId] = nil
        save()
    }

    // MARK: widgets (Pro)

    func refreshWidgets() {
        let pro = Account.shared.pro
        let cw = Array(continueWatching.prefix(3))
        let ml = Array(myList.prefix(3))
        Task.detached(priority: .utility) {
            func item(_ m: MetaPreview, sub: String, progress: Double, key: String) async -> WidgetItem {
                let file = "w-\(key.replacingOccurrences(of: ":", with: "_")).jpg"
                let path = Shared.folder.appendingPathComponent(file)
                if !FileManager.default.fileExists(atPath: path.path), let u = m.backdropURL,
                   let (d, _) = try? await Net.session.data(from: u), let img = UIImage(data: d) {
                    // widgets have a tight memory budget: keep it small
                    let w: CGFloat = 640
                    let size = CGSize(width: w, height: w * img.size.height / max(1, img.size.width))
                    let small = UIGraphicsImageRenderer(size: size).image { _ in img.draw(in: CGRect(origin: .zero, size: size)) }
                    try? small.jpegData(compressionQuality: 0.75)?.write(to: path)
                }
                return WidgetItem(id: m.id, type: m.type.rawValue, title: m.name, subtitle: sub, progress: progress, imageFile: file)
            }
            var a: [WidgetItem] = []
            for p in cw { a.append(await item(p.meta, sub: p.subtitle, progress: p.fraction, key: p.meta.id)) }
            var b: [WidgetItem] = []
            for m in ml { b.append(await item(m, sub: m.year, progress: 0, key: m.id)) }
            WidgetSnapshot(pro: pro, continueWatching: a, myList: b, updated: .now).save()
            WidgetCenter.shared.reloadAllTimelines()
        }
    }
}
