// The pieces of the home screen: rows of posters, wide Continue Watching
// cards and the Top 10 with big numbers.

import SwiftUI

/// A titled horizontal row.
struct Shelf<Content: View>: View {
    let title: String
    var subtitle: String? = nil
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(title).font(.rowTitle)
                if let subtitle { Text(subtitle).font(.meta).foregroundStyle(Theme.faint) }
            }
            .padding(.horizontal, Theme.pad)
            ScrollView(.horizontal, showsIndicators: false) {
                LazyHStack(spacing: 10) { content }
                    .padding(.horizontal, Theme.pad)
                    .scrollTargetLayout()
            }
            .scrollTargetBehavior(.viewAligned)
        }
    }
}

/// A 2:3 poster that opens the title.
struct PosterCard: View {
    let meta: MetaPreview
    var width: CGFloat = 116
    @Environment(\.horizontalSizeClass) private var size

    var body: some View {
        NavigationLink(value: meta) {
            Art(url: meta.posterURL)
                .frame(width: w, height: w * 1.5)
                .clipShape(RoundedRectangle(cornerRadius: Theme.posterRadius, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: Theme.posterRadius, style: .continuous).strokeBorder(Theme.line, lineWidth: 0.5))
                .contextMenu { TitleMenu(meta: meta) } preview: {
                    Art(url: meta.backdropURL).frame(width: 340, height: 191)
                }
        }
        .buttonStyle(PressScale())
        .accessibilityLabel(meta.name)
    }
    private var w: CGFloat { size == .regular ? width * 1.35 : width }
}

/// Long-press actions on any title.
struct TitleMenu: View {
    let meta: MetaPreview
    @State private var lib = Library.shared

    var body: some View {
        Button {
            lib.toggleList(meta)
        } label: {
            lib.inList(meta.id) ? Label("Remove from My List", systemImage: "checkmark") : Label("Add to My List", systemImage: "plus")
        }
        ShareLink(item: URL(string: "\(crewURL)/t/\(meta.type.rawValue)/\(meta.id)")!) { Label("Share", systemImage: "square.and.arrow.up") }
        if lib.resume(meta.id) != nil {
            Button(role: .destructive) { lib.forget(meta.id) } label: { Label("Remove from Continue Watching", systemImage: "xmark") }
        }
    }
}

/// Wide card with a progress line: picks up where you stopped.
struct ContinueCard: View {
    let p: Progress
    @Environment(\.horizontalSizeClass) private var size

    var body: some View {
        let w: CGFloat = size == .regular ? 320 : 250
        Button {
            Task {
                if let m = try? await Cinemeta.shared.meta(p.meta.type, p.meta.id) {
                    await Playback.shared.play(m, episode: m.videos?.first { $0.id == p.videoId })
                }
            }
        } label: {
            VStack(alignment: .leading, spacing: 6) {
                ZStack(alignment: .bottomLeading) {
                    Color.clear
                        .frame(width: w, height: w * 9 / 16)
                        .overlay { Art(url: p.meta.backdropURL) }
                        .clipped()
                        .bottomFade(0.5)
                    Image(systemName: "play.fill")
                        .font(.system(size: 13, weight: .bold))
                        .frame(width: 34, height: 34)
                        .background(.ultraThinMaterial, in: Circle())
                        .padding(10)
                    VStack { Spacer(); ProgressBar(value: p.fraction).frame(height: 3) }
                }
                .clipShape(RoundedRectangle(cornerRadius: Theme.posterRadius, style: .continuous))
                Text(p.meta.name).font(.subheadline.weight(.semibold)).lineLimit(1)
                Text(p.season != nil ? p.subtitle : p.left).font(.caption).foregroundStyle(Theme.dim).lineLimit(1)
            }
            .frame(width: w)
        }
        .buttonStyle(PressScale())
        .contextMenu {
            NavigationLink(value: p.meta) { Label("Details", systemImage: "info.circle") }
            TitleMenu(meta: p.meta)
        }
    }
}

struct ProgressBar: View {
    let value: Double
    var body: some View {
        GeometryReader { g in
            ZStack(alignment: .leading) {
                Rectangle().fill(.white.opacity(0.25))
                Rectangle().fill(.white).frame(width: g.size.width * min(1, max(0, value)))
            }
        }
    }
}

/// Top 10: the rank as a big outlined number behind the poster.
struct RankCard: View {
    let rank: Int
    let meta: MetaPreview

    var body: some View {
        NavigationLink(value: meta) {
            HStack(alignment: .bottom, spacing: -18) {
                Text("\(rank)")
                    .font(.system(size: 110, weight: .black, design: .rounded))
                    .foregroundStyle(.black)
                    .shadow(color: .white.opacity(0.9), radius: 0.6)
                    .shadow(color: .white.opacity(0.9), radius: 0.6)
                    .offset(y: 16)
                    .frame(minWidth: 64)
                Art(url: meta.posterURL)
                    .frame(width: 104, height: 156)
                    .clipShape(RoundedRectangle(cornerRadius: Theme.posterRadius, style: .continuous))
            }
            .clipped()
        }
        .buttonStyle(PressScale())
        .contextMenu { TitleMenu(meta: meta) }
    }
}
