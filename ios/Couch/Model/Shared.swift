// Shared by the app and the widget extension: what the home-screen widgets
// show (written by the app into the App Group), and the Live Activity's shape.

import Foundation
#if canImport(ActivityKit)
import ActivityKit
#endif

enum Shared {
    static let group = "group.com.midnite.couch"
    static var defaults: UserDefaults { UserDefaults(suiteName: group) ?? .standard }
    static var folder: URL {
        FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: group)
            ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
    }
}

/// One card on a widget: something to keep watching or a pick from your list.
struct WidgetItem: Codable, Hashable, Identifiable {
    var id: String          // imdb id (tt…) or tt…:season:episode
    var type: String        // movie | series
    var title: String
    var subtitle: String    // "S2 · E5" or the year
    var progress: Double    // 0…1, 0 when not started
    var imageFile: String?  // a downscaled backdrop saved in the App Group
}

struct WidgetSnapshot: Codable {
    var pro: Bool
    var continueWatching: [WidgetItem]
    var myList: [WidgetItem]
    var updated: Date

    static let empty = WidgetSnapshot(pro: false, continueWatching: [], myList: [], updated: .distantPast)

    static var url: URL { Shared.folder.appendingPathComponent("widgets.json") }
    static func load() -> WidgetSnapshot {
        guard let d = try? Data(contentsOf: url), let s = try? JSONDecoder().decode(WidgetSnapshot.self, from: d) else { return .empty }
        return s
    }
    func save() {
        if let d = try? JSONEncoder().encode(self) { try? d.write(to: Self.url, options: .atomic) }
    }
}

#if canImport(ActivityKit)
/// The "now playing" Live Activity (Dynamic Island + Lock Screen).
struct CouchActivityAttributes: ActivityAttributes {
    public struct ContentState: Codable, Hashable {
        var paused: Bool
        /// seconds into the video when this state was sampled
        var position: Double
        /// total length in seconds (0 while unknown)
        var duration: Double
        /// wall-clock time of the sample, so the widget runs the clock itself
        var sampledAt: Date
    }

    var title: String
    var subtitle: String
    /// hex colour from the artwork
    var tint: String
}
#endif
