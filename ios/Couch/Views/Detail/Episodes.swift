// Episodes by season: a season menu, then wide cards with the still, the
// story and how far you got. Tap plays; long-press picks a stream or marks it.

import SwiftUI
import WebKit

struct EpisodesSection: View {
    let meta: MetaDetail
    @Binding var season: Int
    var pickStream: (Episode) -> Void
    @State private var lib = Library.shared
    @Environment(\.horizontalSizeClass) private var size

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Menu {
                Picker("Season", selection: $season) {
                    ForEach(meta.seasons, id: \.self) { s in
                        Text(s == 0 ? "Specials" : "Season \(s)").tag(s)
                    }
                }
            } label: {
                HStack(spacing: 6) {
                    Text(season == 0 ? "Specials" : "Season \(season)").font(.rowTitle)
                    Image(systemName: "chevron.down").font(.system(size: 13, weight: .bold))
                }
                .foregroundStyle(.white)
            }
            .padding(.horizontal, Theme.pad)

            if size == .regular {
                ScrollView(.horizontal, showsIndicators: false) {
                    LazyHStack(alignment: .top, spacing: 14) {
                        ForEach(meta.episodes(season)) { e in card(e).frame(width: 300) }
                    }
                    .padding(.horizontal, Theme.pad)
                }
            } else {
                LazyVStack(spacing: 18) {
                    ForEach(meta.episodes(season)) { e in row(e) }
                }
                .padding(.horizontal, Theme.pad)
            }
        }
        .animation(.snappy, value: season)
    }

    private func progress(_ e: Episode) -> Double {
        if lib.isWatched(e.id) { return 1 }
        if let p = lib.resume(meta.id), p.videoId == e.id { return p.fraction }
        return 0
    }

    private func still(_ e: Episode) -> some View {
        ZStack(alignment: .bottom) {
            Art(url: e.thumbnail.flatMap(URL.init(string:)) ?? meta.backdropURL)
                .aspectRatio(16 / 9, contentMode: .fill)
            if e.isOut {
                Image(systemName: "play.fill")
                    .font(.system(size: 12, weight: .bold))
                    .frame(width: 30, height: 30)
                    .background(.ultraThinMaterial, in: Circle())
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            let p = progress(e)
            if p > 0 { ProgressBar(value: p).frame(height: 3) }
        }
        .clipShape(RoundedRectangle(cornerRadius: Theme.posterRadius, style: .continuous))
    }

    private func row(_ e: Episode) -> some View {
        Button { play(e) } label: {
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 12) {
                    still(e).frame(width: 140, height: 79)
                    VStack(alignment: .leading, spacing: 3) {
                        Text("\(e.episode ?? 0). \(e.label)").font(.subheadline.weight(.semibold)).lineLimit(2)
                        Text(when(e)).font(.caption).foregroundStyle(Theme.faint)
                    }
                    Spacer(minLength: 0)
                    if lib.isWatched(e.id) { Image(systemName: "checkmark.circle.fill").foregroundStyle(Theme.dim) }
                }
                if let a = e.about, !a.isEmpty {
                    Text(a).font(.footnote).foregroundStyle(Theme.dim).lineLimit(3)
                }
            }
            .multilineTextAlignment(.leading)
            .contentShape(Rectangle())
        }
        .buttonStyle(PressScale())
        .disabled(!e.isOut)
        .opacity(e.isOut ? 1 : 0.5)
        .contextMenu { menu(e) }
    }

    private func card(_ e: Episode) -> some View {
        Button { play(e) } label: {
            VStack(alignment: .leading, spacing: 6) {
                still(e).frame(height: 169)
                Text("\(e.episode ?? 0). \(e.label)").font(.subheadline.weight(.semibold)).lineLimit(1)
                if let a = e.about { Text(a).font(.caption).foregroundStyle(Theme.dim).lineLimit(3) }
            }
            .multilineTextAlignment(.leading)
        }
        .buttonStyle(PressScale())
        .disabled(!e.isOut)
        .opacity(e.isOut ? 1 : 0.5)
        .contextMenu { menu(e) }
    }

    @ViewBuilder
    private func menu(_ e: Episode) -> some View {
        Button { pickStream(e) } label: { Label("Choose stream", systemImage: "list.bullet") }
        if lib.isWatched(e.id) {
            Button { lib.markWatched(e.id, false) } label: { Label("Mark as unwatched", systemImage: "eye.slash") }
        } else {
            Button { lib.markWatched(e.id, true) } label: { Label("Mark as watched", systemImage: "checkmark") }
        }
    }

    private func when(_ e: Episode) -> String {
        guard let d = e.releaseDate else { return "" }
        let f = DateFormatter()
        f.dateStyle = .medium
        return e.isOut ? f.string(from: d) : "Coming \(f.string(from: d))"
    }

    private func play(_ e: Episode) {
        Task { await Playback.shared.play(meta, episode: e) }
    }
}

/// Every stream your sources have for a title, best first, by source.
struct StreamPicker: View {
    let target: StreamTarget
    struct Found: Identifiable { let source: Source; let streams: [Stream]; var id: String { source.id } }
    @State private var results: [Found] = []
    @State private var errors: [String] = []
    @State private var done = false
    @State private var sources = Sources.shared
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                if !sources.any {
                    ContentUnavailableView {
                        Label("No sources yet", systemImage: "server.rack")
                    } description: {
                        Text("Add your own server in Settings → Sources.")
                    }
                }
                ForEach(results) { f in
                    Section(f.source.name) {
                        ForEach(f.streams) { st in
                            Button {
                                dismiss()
                                Task { await Playback.shared.play(target.meta, episode: target.episode, stream: st) }
                            } label: { StreamRow(stream: st) }
                        }
                    }
                }
                if done && results.isEmpty && sources.any {
                    Text("Nothing found for this one.").foregroundStyle(Theme.dim)
                }
                ForEach(errors, id: \.self) { Text($0).font(.footnote).foregroundStyle(Theme.faint) }
            }
            .overlay { if !done && results.isEmpty && sources.any { ProgressView("asking your sources…") } }
            .navigationTitle(target.episode.map { "\(target.meta.name) \($0.code)" } ?? target.meta.name)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } } }
        }
        .presentationDetents([.medium, .large])
        .task {
            for await (s, r) in sources.streams(target.meta.type, target.episode?.id ?? target.meta.id) {
                switch r {
                case .success(let l) where !l.isEmpty:
                    results.append(Found(source: s, streams: l.sorted { $0.rank > $1.rank }))
                    let order = sources.active.map(\.id)
                    results.sort { (order.firstIndex(of: $0.id) ?? 0) < (order.firstIndex(of: $1.id) ?? 0) }
                case .failure(let e):
                    errors.append("\(s.name): \(e.localizedDescription)")
                default: break
                }
            }
            done = true
        }
    }
}

struct StreamRow: View {
    let stream: Stream
    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            HStack(spacing: 6) {
                if let q = stream.quality { Tag(q) }
                if let h = stream.hdr { Tag(h) }
                if let s = stream.size { Tag(s) }
                Spacer()
            }
            Text(stream.behaviorHints?.filename ?? stream.label)
                .font(.subheadline.weight(.medium))
                .lineLimit(2)
            if !stream.text.isEmpty {
                Text(stream.text).font(.caption).foregroundStyle(Theme.dim).lineLimit(3)
            }
        }
        .foregroundStyle(.white)
        .padding(.vertical, 2)
    }
}

struct Tag: View {
    let text: String
    init(_ t: String) { text = t }
    var body: some View {
        Text(text)
            .font(.system(size: 11, weight: .bold))
            .padding(.horizontal, 6)
            .padding(.vertical, 3)
            .background(Color.white.opacity(0.12), in: RoundedRectangle(cornerRadius: 5))
    }
}

/// The trailer (YouTube), in a sheet.
struct TrailerSheet: View {
    let id: String
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        ZStack(alignment: .topTrailing) {
            Color.black.ignoresSafeArea()
            YouTubePlayer(id: id).aspectRatio(16 / 9, contentMode: .fit).frame(maxHeight: .infinity)
            Button { dismiss() } label: { RoundIcon(symbol: "xmark", size: 36) }.padding()
        }
        .presentationBackground(.black)
    }
}

struct YouTubePlayer: UIViewRepresentable {
    let id: String
    func makeUIView(context: Context) -> WKWebView {
        let c = WKWebViewConfiguration()
        c.allowsInlineMediaPlayback = true
        c.mediaTypesRequiringUserActionForPlayback = []
        let w = WKWebView(frame: .zero, configuration: c)
        w.isOpaque = false
        w.backgroundColor = .black
        w.scrollView.isScrollEnabled = false
        let html = """
        <html><head><meta name="viewport" content="width=device-width,initial-scale=1"><style>html,body{margin:0;background:#000;height:100%}iframe{border:0;width:100%;height:100%}</style></head>
        <body><iframe src="https://www.youtube-nocookie.com/embed/\(id)?autoplay=1&playsinline=1&rel=0&modestbranding=1" allow="autoplay; encrypted-media; picture-in-picture" allowfullscreen></iframe></body></html>
        """
        w.loadHTMLString(html, baseURL: URL(string: "https://couch.midnite.app"))
        return w
    }
    func updateUIView(_ v: WKWebView, context: Context) {}
}
