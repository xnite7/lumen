// My couch: what you're watching and what you saved.

import SwiftUI

struct LibraryView: View {
    @State private var lib = Library.shared
    @State private var crew = Crew.shared
    @State private var coming = ComingUp.shared
    private let cols = [GridItem(.adaptive(minimum: 104, maximum: 170), spacing: 10)]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 26) {
                if !crew.watching.isEmpty { WatchingNowRow(members: crew.watching) }
                if !crew.parties.isEmpty { PartiesSection(parties: crew.parties) }
                if lib.continueWatching.isEmpty && lib.myList.isEmpty && crew.watching.isEmpty && crew.parties.isEmpty {
                    ContentUnavailableView {
                        Label("Nothing here yet", systemImage: "sofa")
                    } description: {
                        Text("Titles you save or start watching show up here.")
                    }
                    .padding(.top, 80)
                }
                if !lib.continueWatching.isEmpty {
                    Shelf(title: "Continue Watching") {
                        ForEach(lib.continueWatching, id: \.meta.id) { ContinueCard(p: $0) }
                    }
                }
                if !coming.items.isEmpty { ComingUpSection(items: coming.items) }
                if !lib.myList.isEmpty {
                    VStack(alignment: .leading, spacing: 10) {
                        Text("My List").font(.rowTitle).padding(.horizontal, Theme.pad)
                        LazyVGrid(columns: cols, spacing: 12) {
                            ForEach(lib.myList) { m in
                                NavigationLink(value: m) {
                                    Art(url: m.posterURL)
                                        .aspectRatio(2 / 3, contentMode: .fill)
                                        .clipShape(RoundedRectangle(cornerRadius: Theme.posterRadius, style: .continuous))
                                }
                                .buttonStyle(PressScale())
                                .contextMenu { TitleMenu(meta: m) }
                            }
                        }
                        .padding(.horizontal, Theme.pad)
                    }
                }
            }
            .padding(.vertical, 12)
        }
        .background(Theme.bg)
        .navigationTitle("My couch")
        .task {
            await crew.refresh()
            await coming.refresh()
        }
        .refreshable {
            await crew.refresh(force: true)
            await coming.refresh(force: true)
            await Simkl.shared.sync(force: true)
        }
    }
}
