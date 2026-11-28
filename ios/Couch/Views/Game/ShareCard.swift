// A finished game as a picture to share: the score, how much of each scene
// you needed, the streak and your rank. The same card couch makes on PC.

import SwiftUI

struct GameCardData {
    var daily: Bool
    var score: Int
    var right: Int
    var rounds: Int
    var results: [Int]
    var streak: Int
    var rank: Int?
    var name: String
    var day: String   // "28 Nov 2026"

    static func day(_ iso: String? = nil) -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_GB")
        f.timeZone = TimeZone(identifier: "UTC")
        f.dateFormat = "yyyy-MM-dd"
        let d = iso.flatMap { f.date(from: $0) } ?? .now
        f.dateFormat = "d MMM yyyy"
        return f.string(from: d)
    }
}

struct ShareCard: View {
    let d: GameCardData

    var body: some View {
        ZStack {
            Color.black
            RoundedRectangle(cornerRadius: 44, style: .continuous)
                .fill(RadialGradient(colors: [Color(white: 0.17), Color(white: 0.045), .black], center: .init(x: 0.22, y: 0.1), startRadius: 0, endRadius: 1100))
                .overlay(RoundedRectangle(cornerRadius: 44, style: .continuous).strokeBorder(.white.opacity(0.1), lineWidth: 2))
                .padding(24)
            VStack(alignment: .leading, spacing: 0) {
                HStack(spacing: 18) {
                    Image("Mark").resizable().scaledToFit().frame(width: 78)
                    Text("scenes").font(.system(size: 34, weight: .black))
                    Text("\(d.daily ? "daily five" : "endless run") · \(d.day)").font(.system(size: 26, weight: .medium)).opacity(0.5)
                }
                Spacer()
                HStack(alignment: .center, spacing: 40) {
                    VStack(alignment: .leading, spacing: 0) {
                        Text(d.score.formatted(.number.grouping(.automatic)))
                            .font(.system(size: 176, weight: .black)).tracking(-5).lineLimit(1).minimumScaleFactor(0.5)
                        Text("points · \(d.right) of \(d.rounds) right").font(.system(size: 34, weight: .medium)).opacity(0.6)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    VStack(alignment: .leading, spacing: 22) {
                        Text("HOW MUCH I NEEDED TO SEE").font(.system(size: 21, weight: .bold)).tracking(2).opacity(0.5)
                        HStack(spacing: 12) {
                            ForEach(Array(d.results.prefix(6).enumerated()), id: \.offset) { _, r in
                                ZStack {
                                    if r < 0 {
                                        RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(.white.opacity(0.28), lineWidth: 4)
                                    } else {
                                        RoundedRectangle(cornerRadius: 16, style: .continuous).fill(.white.opacity(1 - Double(r) * 0.18))
                                        Text(["blink", "1s", "3s", "5s", "all"][min(4, r)]).font(.system(size: 21, weight: .black)).foregroundStyle(.black.opacity(0.75))
                                    }
                                }
                                .frame(width: 68, height: 68)
                            }
                        }
                        VStack(alignment: .leading, spacing: 8) {
                            if d.daily, d.streak > 0 { Text("\(d.streak) day streak") }
                            if let r = d.rank { Text("#\(r) \(d.daily ? "today" : "all time")") }
                        }
                        .font(.system(size: 32, weight: .bold)).opacity(0.9)
                    }
                    .frame(width: 440, alignment: .leading)
                }
                Spacer()
                HStack {
                    Text(d.name).font(.system(size: 30, weight: .bold))
                    Spacer()
                    Text("guess the movie from one scene · midnite couch").font(.system(size: 26, weight: .medium)).opacity(0.45)
                }
            }
            .padding(.horizontal, 72)
            .padding(.vertical, 62)
        }
        .foregroundStyle(.white)
        .frame(width: 1200, height: 630)
        .environment(\.colorScheme, .dark)
    }

    /// The card as a picture.
    @MainActor func image() -> Image? {
        let r = ImageRenderer(content: self)
        r.scale = 1
        return r.uiImage.map { Image(uiImage: $0) }
    }
}

/// The card at whatever width there is, with a Share button under it.
struct ShareCardPanel: View {
    let d: GameCardData
    @State private var picture: Image?

    var body: some View {
        VStack(spacing: 12) {
            GeometryReader { g in
                ShareCard(d: d)
                    .scaleEffect(g.size.width / 1200, anchor: .topLeading)
                    .frame(width: g.size.width, height: g.size.width * 630 / 1200, alignment: .topLeading)
                    .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
            }
            .aspectRatio(1200 / 630, contentMode: .fit)
            if let picture {
                ShareLink(item: picture, preview: SharePreview("couch scenes · \(d.score) points", image: picture)) {
                    Label("Share", systemImage: "square.and.arrow.up")
                }
                .buttonStyle(PillButton(primary: false))
            }
        }
        .task { picture = ShareCard(d: d).image() }
    }
}
