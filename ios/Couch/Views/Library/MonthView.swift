// "Your month" (couch Pro): what you watched this month, and a card of it
// to share. From your Simkl history when it's linked, else this device.

import SwiftUI

struct MonthView: View {
    @State private var month = Date()
    @State private var history = History.shared
    @State private var card: Image?
    @State private var making = false

    private var items: [WatchedTitle] { history.month(month) }
    private var films: Int { items.filter { $0.type == .movie }.count }
    private var shows: Int { items.filter { $0.type == .series }.count }
    /// a rough count: a film's length, or an episode's length for a show
    private var hours: Int {
        let mins = items.reduce(0) { $0 + ($1.minutes ?? ($1.type == .movie ? 110 : 45)) }
        return Int((Double(mins) / 60).rounded())
    }
    private var title: String { month.formatted(.dateTime.month(.wide).year()) }
    private let cols = [GridItem(.adaptive(minimum: 96, maximum: 150), spacing: 10)]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                HStack {
                    Button { step(-1) } label: { Image(systemName: "chevron.left") }
                    Spacer()
                    Text(title).font(.title2.weight(.heavy))
                    Spacer()
                    Button { step(1) } label: { Image(systemName: "chevron.right") }
                        .disabled(Calendar.current.isDate(month, equalTo: .now, toGranularity: .month))
                }
                .font(.headline)
                HStack(spacing: 10) {
                    stat("\(items.count)", "titles", GameTint.gold)
                    stat("\(films)", films == 1 ? "film" : "films", GameTint.green)
                    stat("\(shows)", shows == 1 ? "show" : "shows", GameTint.violet)
                    stat("~\(hours)", "hours", GameTint.flame)
                }
                if items.isEmpty {
                    ContentUnavailableView("Nothing watched yet", systemImage: "calendar",
                                           description: Text(Simkl.shared.connected ? "What you watch shows up here." : "Link Simkl in Settings to bring in your whole history."))
                        .padding(.top, 30)
                } else {
                    LazyVGrid(columns: cols, spacing: 10) {
                        ForEach(items) { w in
                            NavigationLink(value: MetaPreview(id: w.id, type: w.type, name: w.name)) {
                                Art(url: MetaPreview(id: w.id, type: w.type, name: w.name).posterURL)
                                    .aspectRatio(2 / 3, contentMode: .fill)
                                    .clipShape(RoundedRectangle(cornerRadius: Theme.posterRadius, style: .continuous))
                            }
                            .buttonStyle(PressScale())
                        }
                    }
                    if let card {
                        ShareLink(item: card, preview: SharePreview("My \(title) on couch", image: card)) {
                            Label("Share your month", systemImage: "square.and.arrow.up")
                        }
                        .buttonStyle(PillButton())
                    } else {
                        Button { Task { await makeCard() } } label: {
                            HStack { if making { ProgressView().tint(.black) }; Text(making ? "Making your card" : "Make a card to share") }
                        }
                        .buttonStyle(PillButton())
                        .disabled(making)
                    }
                }
            }
            .padding(Theme.pad)
            .frame(maxWidth: 760)
            .frame(maxWidth: .infinity)
        }
        .background(Theme.bg)
        .navigationTitle("Your month")
        .navigationBarTitleDisplayMode(.inline)
    }

    private func stat(_ n: String, _ label: String, _ tint: Color) -> some View {
        VStack(spacing: 2) {
            Text(n).font(.title2.weight(.heavy)).monospacedDigit().foregroundStyle(tint).lineLimit(1).minimumScaleFactor(0.6)
            Text(label).font(.caption).foregroundStyle(Theme.dim)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 12)
        .background(tint.opacity(0.12), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
    }

    private func step(_ by: Int) {
        if let m = Calendar.current.date(byAdding: .month, value: by, to: month), m <= .now { month = m }
        card = nil
    }

    /// the card needs the posters in hand (a picture can't wait for the network)
    private func makeCard() async {
        making = true
        defer { making = false }
        var posters: [UIImage] = []
        for w in items.prefix(10) {
            guard let u = MetaPreview(id: w.id, type: w.type, name: w.name).posterURL,
                  let (d, _) = try? await Net.session.data(from: u), let img = UIImage(data: d) else { continue }
            posters.append(img)
        }
        let r = ImageRenderer(content: MonthCard(title: title, titles: items.count, films: films, shows: shows, hours: hours,
                                                 posters: posters, name: Account.shared.discord?.name ?? Account.shared.couch?.name ?? ""))
        r.scale = 1
        card = r.uiImage.map { Image(uiImage: $0) }
    }
}

/// The picture: 1200 × 630, like the game's card.
struct MonthCard: View {
    let title: String
    let titles: Int, films: Int, shows: Int, hours: Int
    let posters: [UIImage]
    let name: String

    var body: some View {
        ZStack {
            Color.black
            RoundedRectangle(cornerRadius: 44, style: .continuous)
                .fill(RadialGradient(colors: [Color(red: 0.24, green: 0.17, blue: 0.08), Color(white: 0.05), .black], center: .init(x: 0.2, y: 0.1), startRadius: 0, endRadius: 1100))
                .padding(24)
            VStack(alignment: .leading, spacing: 0) {
                HStack(spacing: 18) {
                    Image("Mark").resizable().scaledToFit().frame(width: 78)
                    Text("my month").font(.system(size: 34, weight: .black))
                    Text(title).font(.system(size: 26, weight: .medium)).opacity(0.5)
                }
                Spacer()
                HStack(alignment: .bottom, spacing: 44) {
                    VStack(alignment: .leading, spacing: 0) {
                        Text("\(titles)").font(.system(size: 170, weight: .black)).tracking(-5).foregroundStyle(GameTint.gold)
                        Text("titles · ~\(hours) hours").font(.system(size: 34, weight: .medium)).opacity(0.6)
                        Text("\(films) film\(films == 1 ? "" : "s") · \(shows) show\(shows == 1 ? "" : "s")").font(.system(size: 28, weight: .medium)).opacity(0.45).padding(.top, 6)
                    }
                    HStack(spacing: -34) {
                        ForEach(Array(posters.prefix(6).enumerated()), id: \.offset) { i, p in
                            Image(uiImage: p).resizable().aspectRatio(2 / 3, contentMode: .fill)
                                .frame(width: 150, height: 225)
                                .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                                .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(.black.opacity(0.6), lineWidth: 3))
                                .rotationEffect(.degrees(Double(i - 2) * 3))
                                .zIndex(Double(6 - i))
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .trailing)
                }
                Spacer()
                HStack {
                    Text(name).font(.system(size: 30, weight: .bold))
                    Spacer()
                    Text("midnite couch").font(.system(size: 26, weight: .medium)).opacity(0.45)
                }
            }
            .padding(.horizontal, 72)
            .padding(.vertical, 62)
        }
        .foregroundStyle(.white)
        .frame(width: 1200, height: 630)
        .environment(\.colorScheme, .dark)
    }
}
