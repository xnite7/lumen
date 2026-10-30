// Shared shape of the "now playing" Live Activity. The widget extension
// (src-tauri/ios-widgets) declares the identical type; ActivityKit matches
// them by name and fields.

#if canImport(ActivityKit)
import ActivityKit
import Foundation

@available(iOS 16.1, *)
struct LumenActivityAttributes: ActivityAttributes {
    public struct ContentState: Codable, Hashable {
        var paused: Bool
        /// seconds into the video when this state was sampled
        var position: Double
        /// total length in seconds (0 while unknown)
        var duration: Double
        /// wall-clock time of the sample, so the widget can run the clock itself
        var sampledAt: Date
    }

    var title: String
    var subtitle: String
    /// hex colour from the artwork
    var tint: String
}
#endif
