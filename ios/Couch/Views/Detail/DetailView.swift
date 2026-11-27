// A title's page, Apple TV style: the artwork (a scene clip with Pro)
// stretches under the top, then the logo, Play, My List and Trailer, the
// story, episodes by season, cast, and More Like This.

import SwiftUI

struct DetailView: View {
    let preview: MetaPreview
    @State private var meta: MetaDetail?
    @State private var failed = false
    @State private var season = 1
    @State private var similar: [MetaPreview] = []
    @State private var showStreams: StreamTarget?
    @State private var trailer: String?
    @State private var moreText = false
    @State private var lib = Library.shared
    @State private var playback = Playback.shared
    @State private var scrolled = false
    @Environment(\.horizontalSizeClass) private var size

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                header
                if let m = meta {
                    content(m).padding(.top, 6)
                } else if failed {
                    ContentUnavailableView("couldn't load this title", systemImage: "wifi.slash").padding(.top, 40)
                } else {
                    ProgressView().frame(maxWidth: .infinity).padding(.top, 40)
                }
            }
            .padding(.bottom, 40)
        }
        .onScrollGeometryChange(for: Bool.self) { $0.contentOffset.y > 380 } action: { _, v in scrolled = v }
        .background(Theme.bg)
        .ignoresSafeArea(edges: .top)
        .navigationTitle(scrolled ? (meta?.name ?? preview.name) : "")
        .navigationBarTitleDisplayMode(.inline)
        .toolbarBackground(scrolled ? .visible : .hidden, for: .navigationBar)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                ShareLink(item: URL(string: "\(crewURL)/t/\(preview.type.rawValue)/\(preview.id)")!) {
                    Image(systemName: "square.and.arrow.up")
                }
            }
        }
        .task(id: preview.id) { await load() }
        .sheet(item: $showStreams) { t in StreamPicker(target: t) }
        .sheet(item: Binding(get: { trailer.map { YT(id: $0) } }, set: { trailer = $0?.id })) { t in
            TrailerSheet(id: t.id)
        }
    }

    private var base: MetaPreview { meta?.preview ?? preview }

    // MARK: header

    private var header: some View {
        let h: CGFloat = size == .regular ? 560 : 470
        return GeometryReader { g in
            let pull = max(0, g.frame(in: .scrollView).minY)
            ClipBackdrop(meta: base, active: !scrolled)
                .frame(width: g.size.width, height: h + pull)
                .clipped()
                .bottomFade(0.5)
                .offset(y: -pull)
        }
        .frame(height: h)
        .overlay(alignment: .bottom) {
            TitleLogo(meta: base, maxHeight: size == .regular ? 130 : 96, maxWidth: size == .regular ? 400 : 280)
                .padding(.bottom, 10)
        }
    }

    // MARK: body

    @ViewBuilder
    private func content(_ m: MetaDetail) -> some View {
        VStack(alignment: .leading, spacing: 18) {
            facts(m).frame(maxWidth: .infinity)
            actions(m)
            if let d = m.description, !d.isEmpty {
                Text(d)
                    .font(.callout)
                    .foregroundStyle(.white.opacity(0.85))
                    .lineLimit(moreText ? nil : 4)
                    .onTapGesture { withAnimation(.snappy) { moreText.toggle() } }
            }
            people(m)
        }
        .padding(.horizontal, Theme.pad)
        .frame(maxWidth: 760)
        .frame(maxWidth: .infinity)

        if m.type == .series, !m.seasons.isEmpty {
            EpisodesSection(meta: m, season: $season) { e in
                showStreams = StreamTarget(meta: m, episode: e)
            }
            .padding(.top, 26)
        }
        if !similar.isEmpty {
            Shelf(title: "More Like This") { ForEach(similar) { PosterCard(meta: $0) } }
                .padding(.top, 30)
        }
    }

    private func facts(_ m: MetaDetail) -> some View {
        VStack(spacing: 6) {
            HStack(spacing: 8) {
                if let y = m.releaseInfo ?? m.year { Text(y) }
                if let r = m.runtime, !r.isEmpty { dot; Text(r) }
                if let r = m.imdbRating, !r.isEmpty { dot; Label(r, systemImage: "star.fill").labelStyle(.titleAndIcon) }
                if m.type == .series, m.seasons.filter({ $0 > 0 }).count > 0 { dot; Text("\(m.seasons.filter { $0 > 0 }.count) Season\(m.seasons.filter { $0 > 0 }.count == 1 ? "" : "s")") }
            }
            .font(.meta)
            .foregroundStyle(Theme.dim)
            if let g = m.genres, !g.isEmpty {
                Text(g.prefix(4).joined(separator: " · ")).font(.meta).foregroundStyle(Theme.faint)
            }
        }
    }
    private var dot: some View { Text("·").foregroundStyle(Theme.faint) }

    private func actions(_ m: MetaDetail) -> some View {
        let next = m.type == .series ? playback.upNext(m) : nil
        let resume = lib.resume(m.id)
        let resuming = resume.map { !$0.finished && $0.videoId == (next?.id ?? m.id) } ?? false
        return VStack(spacing: 10) {
            Button {
                Task { await playback.play(m, episode: next) }
            } label: {
                HStack(spacing: 8) {
                    Image(systemName: "play.fill")
                    Text(resuming ? "Resume" : "Play")
                    if let next { Text(next.code).foregroundStyle(.black.opacity(0.55)) }
                }
            }
            .buttonStyle(PillButton())
            if resuming, let r = resume {
                HStack(spacing: 10) {
                    ProgressBar(value: r.fraction).frame(height: 3).clipShape(Capsule())
                    Text(r.left).font(.caption).foregroundStyle(Theme.dim)
                }
            }
            HStack(spacing: 10) {
                Button {
                    withAnimation(.snappy) { lib.toggleList(m.preview) }
                } label: {
                    Label("My List", systemImage: lib.inList(m.id) ? "checkmark" : "plus").contentTransition(.symbolEffect(.replace))
                }
                .buttonStyle(PillButton(primary: false))
                if let yt = m.trailerStreams?.compactMap(\.ytId).first {
                    Button { trailer = yt } label: { Label("Trailer", systemImage: "film") }
                        .buttonStyle(PillButton(primary: false))
                }
                Button { showStreams = StreamTarget(meta: m, episode: next) } label: { Label("Streams", systemImage: "list.bullet") }
                    .buttonStyle(PillButton(primary: false))
            }
        }
    }

    @ViewBuilder
    private func people(_ m: MetaDetail) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            if let c = m.cast, !c.isEmpty {
                (Text("Starring ").foregroundStyle(Theme.faint) + Text(c.prefix(5).joined(separator: ", "))).font(.footnote)
            }
            if let d = m.director, !d.isEmpty {
                (Text("Director ").foregroundStyle(Theme.faint) + Text(d.joined(separator: ", "))).font(.footnote)
            }
        }
        .foregroundStyle(Theme.dim)
    }

    // MARK: data

    private func load() async {
        failed = false
        do {
            let m = try await Cinemeta.shared.meta(preview.type, preview.id)
            meta = m
            if m.type == .series {
                season = playback.upNext(m)?.season ?? m.seasons.first ?? 1
            }
            similar = await Cinemeta.shared.similar(m)
        } catch {
            failed = true
        }
    }
}

struct YT: Identifiable { let id: String }

/// What the stream picker is for: a movie, or one episode.
struct StreamTarget: Identifiable {
    let meta: MetaDetail
    let episode: Episode?
    var id: String { episode?.id ?? meta.id }
}
