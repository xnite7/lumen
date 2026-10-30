// Lumen's Live Activity: what's playing, in the Dynamic Island and on the
// Lock Screen. The clock runs on its own from the last sample (no updates
// needed while playing); the app only updates on pause, play and seek.

import ActivityKit
import SwiftUI
import WidgetKit

struct LumenActivityAttributes: ActivityAttributes {
    public struct ContentState: Codable, Hashable {
        var paused: Bool
        var position: Double
        var duration: Double
        var sampledAt: Date
    }

    var title: String
    var subtitle: String
    var tint: String
}

@main
struct LumenWidgetBundle: WidgetBundle {
    var body: some Widget {
        NowPlayingActivity()
    }
}

extension Color {
    init(hex: String) {
        let s = hex.trimmingCharacters(in: CharacterSet.alphanumerics.inverted)
        var v: UInt64 = 0
        Scanner(string: s).scanHexInt64(&v)
        let r, g, b: Double
        if s.count == 3 {
            r = Double((v >> 8) & 0xF) / 15
            g = Double((v >> 4) & 0xF) / 15
            b = Double(v & 0xF) / 15
        } else {
            r = Double((v >> 16) & 0xFF) / 255
            g = Double((v >> 8) & 0xFF) / 255
            b = Double(v & 0xFF) / 255
        }
        self.init(red: r, green: g, blue: b)
    }
}

private extension LumenActivityAttributes.ContentState {
    /// when the video "started", given the sampled position
    var startDate: Date { sampledAt.addingTimeInterval(-position) }
    var endDate: Date { startDate.addingTimeInterval(max(duration, position + 1)) }
    var hasLength: Bool { duration > 1 }
}

struct ProgressLine: View {
    let state: LumenActivityAttributes.ContentState
    let tint: Color

    var body: some View {
        if state.hasLength {
            if state.paused {
                ProgressView(value: min(state.position, state.duration), total: state.duration)
                    .tint(tint)
            } else {
                ProgressView(timerInterval: state.startDate ... state.endDate, countsDown: false) {
                    EmptyView()
                } currentValueLabel: {
                    EmptyView()
                }
                .tint(tint)
            }
        }
    }
}

struct TimeLeft: View {
    let state: LumenActivityAttributes.ContentState

    var body: some View {
        if !state.hasLength {
            Image(systemName: "waveform")
        } else if state.paused {
            Text(Self.format(state.duration - state.position)).monospacedDigit()
        } else {
            Text(timerInterval: Date() ... state.endDate, countsDown: true)
                .monospacedDigit()
                .multilineTextAlignment(.trailing)
        }
    }

    static func format(_ s: Double) -> String {
        let t = max(0, Int(s))
        return t >= 3600 ? String(format: "%d:%02d:%02d", t / 3600, (t % 3600) / 60, t % 60) : String(format: "%d:%02d", t / 60, t % 60)
    }
}

struct NowPlayingActivity: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: LumenActivityAttributes.self) { context in
            let tint = Color(hex: context.attributes.tint)
            // Lock Screen / banner
            HStack(spacing: 14) {
                ZStack {
                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                        .fill(LinearGradient(colors: [tint, tint.opacity(0.55)], startPoint: .topLeading, endPoint: .bottomTrailing))
                    Image(systemName: context.state.paused ? "pause.fill" : "play.fill")
                        .font(.title3.weight(.bold))
                        .foregroundStyle(.white)
                }
                .frame(width: 52, height: 52)
                VStack(alignment: .leading, spacing: 4) {
                    Text(context.attributes.title)
                        .font(.headline)
                        .lineLimit(1)
                    if !context.attributes.subtitle.isEmpty {
                        Text(context.attributes.subtitle)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                    }
                    HStack(spacing: 8) {
                        ProgressLine(state: context.state, tint: tint)
                        TimeLeft(state: context.state)
                            .font(.caption.weight(.semibold))
                            .foregroundStyle(.secondary)
                            .frame(width: 56, alignment: .trailing)
                    }
                }
            }
            .padding(16)
            .activityBackgroundTint(Color.black.opacity(0.6))
            .activitySystemActionForegroundColor(.white)
        } dynamicIsland: { context in
            let tint = Color(hex: context.attributes.tint)
            return DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    ZStack {
                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                            .fill(LinearGradient(colors: [tint, tint.opacity(0.5)], startPoint: .topLeading, endPoint: .bottomTrailing))
                        Image(systemName: "sun.max.fill")
                            .font(.title3)
                            .foregroundStyle(.white)
                    }
                    .frame(width: 44, height: 44)
                    .padding(.leading, 4)
                }
                DynamicIslandExpandedRegion(.trailing) {
                    Image(systemName: context.state.paused ? "pause.circle.fill" : "waveform")
                        .font(.title2)
                        .foregroundStyle(tint)
                        .padding(.trailing, 4)
                }
                DynamicIslandExpandedRegion(.center) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(context.attributes.title)
                            .font(.headline)
                            .lineLimit(1)
                        if !context.attributes.subtitle.isEmpty {
                            Text(context.attributes.subtitle)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .lineLimit(1)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                DynamicIslandExpandedRegion(.bottom) {
                    HStack(spacing: 10) {
                        ProgressLine(state: context.state, tint: tint)
                        TimeLeft(state: context.state)
                            .font(.caption.weight(.semibold))
                            .foregroundStyle(.secondary)
                            .frame(width: 58, alignment: .trailing)
                    }
                    .padding(.horizontal, 6)
                }
            } compactLeading: {
                Image(systemName: context.state.paused ? "pause.fill" : "play.fill")
                    .foregroundStyle(tint)
            } compactTrailing: {
                TimeLeft(state: context.state)
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(tint)
                    .frame(maxWidth: 46)
            } minimal: {
                Image(systemName: context.state.paused ? "pause.fill" : "play.fill")
                    .foregroundStyle(tint)
            }
            .keylineTint(tint)
        }
    }
}
