// The game's colours and its little celebrations.

import SwiftUI

enum GameTint {
    static let gold = Color(red: 1.0, green: 0.78, blue: 0.36)
    static let green = Color(red: 0.36, green: 0.84, blue: 0.56)
    static let red = Color(red: 0.98, green: 0.43, blue: 0.41)
    static let violet = Color(red: 0.67, green: 0.59, blue: 0.98)
    static let flame = Color(red: 1.0, green: 0.58, blue: 0.26)

    /// easy / film fan / deep cut
    static func level(_ l: Int) -> Color { [gold, green, gold, violet][min(3, max(0, l))] }
    /// how soon you got it: a blink is gold, the whole scene is plain
    static func result(_ r: Int) -> Color {
        r < 0 ? .clear : [gold, green, green.opacity(0.75), violet.opacity(0.8), Color.white.opacity(0.45)][min(4, r)]
    }
}

/// A burst of paper each time `trigger` changes.
struct Confetti: View {
    let trigger: Int
    var origin = UnitPoint(x: 0.5, y: 0.3)
    @State private var start: Date?
    @State private var bits: [Bit] = []
    @Environment(\.accessibilityReduceMotion) private var still

    private struct Bit {
        var angle: Double, speed: Double, spin: Double, size: Double, delay: Double
        var color: Color
    }

    var body: some View {
        TimelineView(.animation(paused: start == nil)) { tl in
            Canvas { ctx, size in
                guard let start else { return }
                let t = tl.date.timeIntervalSince(start)
                for b in bits {
                    let u = t - b.delay
                    guard u > 0, u < 1.7 else { continue }
                    let x = size.width * origin.x + cos(b.angle) * b.speed * u
                    let y = size.height * origin.y + sin(b.angle) * b.speed * u + 620 * u * u
                    var c = ctx
                    c.opacity = min(1, max(0, (1.7 - u) / 0.5))
                    c.translateBy(x: x, y: y)
                    c.rotate(by: .radians(b.spin * u))
                    c.fill(Path(roundedRect: CGRect(x: -b.size / 2, y: -b.size / 4, width: b.size, height: b.size / 2), cornerRadius: 1.5), with: .color(b.color))
                }
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
        .onChange(of: trigger) { _, _ in
            guard !still else { return }
            let colors = [GameTint.gold, GameTint.green, GameTint.violet, GameTint.flame, .white]
            bits = (0..<80).map { _ in
                Bit(angle: .random(in: (-Double.pi * 0.95)...(-Double.pi * 0.05)), speed: .random(in: 180...620), spin: .random(in: -9...9),
                    size: .random(in: 7...13), delay: .random(in: 0...0.12), color: colors.randomElement()!)
            }
            let s = Date.now
            start = s
            Task {
                try? await Task.sleep(for: .seconds(1.9))
                if start == s { start = nil }
            }
        }
    }
}

/// A number that counts up to its value when it appears or changes.
struct CountUp: View {
    let value: Int
    @State private var shown = 0
    var body: some View {
        Text("\(shown)")
            .monospacedDigit()
            .contentTransition(.numericText(value: Double(shown)))
            .onAppear { withAnimation(.easeOut(duration: 0.9)) { shown = value } }
            .onChange(of: value) { _, v in withAnimation(.easeOut(duration: 0.6)) { shown = v } }
    }
}
