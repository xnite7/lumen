// Home: one big billboard, then rows. Continue Watching and My List first,
// then what's popular, the Top 10 and a few genres. A Movies / Series switch
// at the top narrows everything, like Netflix.

import SwiftUI

struct HomeView: View {
    enum Filter: String, CaseIterable { case all = "All", movies = "Movies", series = "Series" }

    @State private var filter: Filter = .all
    @State private var featured: MetaPreview?
    struct Row: Identifiable { let title: String; let items: [MetaPreview]; var id: String { title } }
    @State private var rows: [Row] = []
    @State private var top: [MetaPreview] = []
    @State private var loading = true
    @State private var failed = false
    @State private var lib = Library.shared
    @State private var scrolled = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 28) {
                if let f = featured {
                    Billboard(meta: f, active: !scrolled)
                } else if loading {
                    Theme.card.frame(height: 520).overlay(ProgressView())
                }
                if filter == .all, !lib.continueWatching.isEmpty {
                    Shelf(title: "Continue Watching") {
                        ForEach(lib.continueWatching, id: \.meta.id) { ContinueCard(p: $0) }
                    }
                }
                let mine = lib.myList.filter(matches)
                if !mine.isEmpty {
                    Shelf(title: "My List") { ForEach(mine) { PosterCard(meta: $0) } }
                }
                if !top.isEmpty {
                    Shelf(title: "Top 10 \(filter == .series ? "Series" : "Movies") Today") {
                        ForEach(Array(top.prefix(10).enumerated()), id: \.element.id) { i, m in RankCard(rank: i + 1, meta: m) }
                    }
                }
                ForEach(rows) { r in
                    Shelf(title: r.title) { ForEach(r.items) { PosterCard(meta: $0) } }
                }
                if failed {
                    ContentUnavailableView {
                        Label("couldn't load", systemImage: "wifi.slash")
                    } actions: {
                        Button("Try again") { Task { await load() } }.buttonStyle(.bordered)
                    }
                }
            }
            .padding(.bottom, 30)
        }
        .onScrollGeometryChange(for: Bool.self) { $0.contentOffset.y > 300 } action: { _, v in scrolled = v }
        .background(Theme.bg)
        .ignoresSafeArea(edges: .top)
        .toolbar {
            ToolbarItem(placement: .principal) {
                Picker("", selection: $filter) {
                    ForEach(Filter.allCases, id: \.self) { Text($0.rawValue) }
                }
                .pickerStyle(.segmented)
                .frame(width: 240)
            }
        }
        .toolbarBackground(scrolled ? .visible : .hidden, for: .navigationBar)
        .navigationBarTitleDisplayMode(.inline)
        .task(id: filter) { await load() }
        .refreshable { await load() }
    }

    private func matches(_ m: MetaPreview) -> Bool {
        switch filter {
        case .all: true
        case .movies: m.type == .movie
        case .series: m.type == .series
        }
    }

    private func load() async {
        loading = true
        failed = false
        defer { loading = false }
        let c = Cinemeta.shared
        let kinds: [Kind] = filter == .series ? [.series] : filter == .movies ? [.movie] : [.movie, .series]
        do {
            async let topMovies = c.catalog(.movie)
            async let topSeries = c.catalog(.series)
            let (tm, ts) = try await (topMovies, topSeries)
            top = filter == .series ? ts : tm
            // the billboard: something popular with good art, a new pick each visit
            let pool = (kinds.contains(.movie) ? Array(tm.prefix(8)) : []) + (kinds.contains(.series) ? Array(ts.prefix(8)) : [])
            if featured == nil || !matches(featured!) { featured = pool.randomElement() }

            var out: [Row] = []
            if kinds.contains(.series) { out.append(Row(title: "Popular Series", items: Array(ts.dropFirst(filter == .series ? 10 : 0)))) }
            if kinds.contains(.movie) { out.append(Row(title: "Popular Movies", items: Array(tm.dropFirst(filter == .series ? 0 : 10)))) }
            for k in kinds {
                if let best = try? await c.catalog(k, "imdbRating") { out.append(Row(title: "Best Rated \(k.label)", items: best)) }
            }
            rows = out
            let genres = ["Action", "Comedy", "Thriller", "Animation", "Sci-Fi", "Horror", "Romance", "Documentary"]
            for g in genres {
                let k = kinds.count == 1 ? kinds[0] : (["Animation", "Documentary"].contains(g) ? Kind.series : .movie)
                if let l = try? await c.catalog(k, "top", genre: g), !l.isEmpty { rows.append(Row(title: "\(g) \(k.label)", items: l)) }
            }
        } catch {
            if rows.isEmpty { failed = true }
        }
    }
}

/// The big artwork at the top: backdrop (or a muted scene clip with Pro),
/// the title's logo, genres, Play and My List.
struct Billboard: View {
    let meta: MetaPreview
    var active = true
    @State private var lib = Library.shared
    @State private var playback = Playback.shared
    @Environment(\.horizontalSizeClass) private var size

    var body: some View {
        let h: CGFloat = size == .regular ? 640 : 560
        ZStack(alignment: .bottom) {
            // the artwork fills the space without widening the page
            Color.clear
                .frame(maxWidth: .infinity)
                .frame(height: h)
                .overlay { ClipBackdrop(meta: meta, active: active) }
                .clipped()
                .bottomFade(0.55)
                .overlay(LinearGradient(colors: [.black.opacity(0.5), .clear], startPoint: .top, endPoint: .init(x: 0.5, y: 0.25)))
            VStack(spacing: 14) {
                NavigationLink(value: meta) {
                    TitleLogo(meta: meta, maxHeight: size == .regular ? 130 : 100, maxWidth: size == .regular ? 380 : 270)
                }
                if let g = meta.genres, !g.isEmpty {
                    Text(g.prefix(3).joined(separator: "  ·  ")).font(.meta).foregroundStyle(.white.opacity(0.85))
                }
                HStack(spacing: 10) {
                    Button {
                        Task {
                            if let m = try? await Cinemeta.shared.meta(meta.type, meta.id) { await playback.play(m) }
                        }
                    } label: {
                        Label(lib.resume(meta.id) != nil ? "Resume" : "Play", systemImage: "play.fill")
                    }
                    .buttonStyle(PillButton())
                    Button {
                        withAnimation(.snappy) { lib.toggleList(meta) }
                    } label: {
                        Label("My List", systemImage: lib.inList(meta.id) ? "checkmark" : "plus")
                            .contentTransition(.symbolEffect(.replace))
                    }
                    .buttonStyle(PillButton(primary: false))
                }
                .frame(maxWidth: 420)
                .padding(.horizontal, Theme.pad)
            }
            .padding(.bottom, 8)
        }
        .frame(height: h)
    }
}
