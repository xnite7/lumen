// Search: type a title, get a poster grid. Before you type: genres to browse.

import SwiftUI

struct SearchView: View {
    @State private var query = ""
    @State private var results: [MetaPreview] = []
    @State private var searching = false
    @State private var genre: String?
    @State private var genreItems: [MetaPreview] = []
    @State private var kind: Kind = .movie

    private let cols = [GridItem(.adaptive(minimum: 104, maximum: 170), spacing: 10)]

    var body: some View {
        ScrollView {
            if query.trimmingCharacters(in: .whitespaces).isEmpty {
                browse
            } else if results.isEmpty && !searching {
                ContentUnavailableView.search(text: query).padding(.top, 60)
            } else {
                grid(results)
            }
        }
        .background(Theme.bg)
        .navigationTitle("Search")
        .searchable(text: $query, prompt: "Movies and series")
        .autocorrectionDisabled()
        .overlay { if searching && results.isEmpty { ProgressView() } }
        .task(id: query) {
            let q = query.trimmingCharacters(in: .whitespaces)
            guard q.count >= 2 else { results = []; return }
            searching = true
            try? await Task.sleep(for: .milliseconds(300)) // wait for typing to settle
            guard !Task.isCancelled else { return }
            let r = await Cinemeta.shared.search(q)
            guard !Task.isCancelled else { return }
            results = r
            searching = false
        }
    }

    private func grid(_ items: [MetaPreview]) -> some View {
        LazyVGrid(columns: cols, spacing: 12) {
            ForEach(items) { m in
                NavigationLink(value: m) {
                    Art(url: m.posterURL)
                        .aspectRatio(2 / 3, contentMode: .fill)
                        .clipShape(RoundedRectangle(cornerRadius: Theme.posterRadius, style: .continuous))
                }
                .buttonStyle(PressScale())
                .contextMenu { TitleMenu(meta: m) }
                .accessibilityLabel(m.name)
            }
        }
        .padding(.horizontal, Theme.pad)
        .padding(.bottom, 30)
    }

    private var browse: some View {
        VStack(alignment: .leading, spacing: 16) {
            Picker("", selection: $kind) {
                Text("Movies").tag(Kind.movie)
                Text("Series").tag(Kind.series)
            }
            .pickerStyle(.segmented)
            .padding(.horizontal, Theme.pad)

            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(genreList, id: \.self) { g in
                        Button { withAnimation(.snappy) { genre = genre == g ? nil : g } } label: {
                            Text(g)
                                .font(.subheadline.weight(.semibold))
                                .padding(.horizontal, 14)
                                .frame(height: 34)
                                .foregroundStyle(genre == g ? .black : .white)
                                .background(genre == g ? AnyShapeStyle(.white) : AnyShapeStyle(Theme.card), in: Capsule())
                        }
                    }
                }
                .padding(.horizontal, Theme.pad)
            }
            if genreItems.isEmpty {
                ProgressView().frame(maxWidth: .infinity).padding(.top, 40)
            } else {
                grid(genreItems)
            }
        }
        .padding(.top, 8)
        .task(id: "\(kind.rawValue)|\(genre ?? "")") {
            genreItems = []
            genreItems = (try? await Cinemeta.shared.catalog(kind, "top", genre: genre)) ?? []
        }
    }
}
