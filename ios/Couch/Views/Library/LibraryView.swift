// My couch: what you're watching and what you saved.

import SwiftUI

struct LibraryView: View {
    @State private var lib = Library.shared
    @State private var crew = Crew.shared
    @State private var coming = ComingUp.shared
    @State private var friends = Friends.shared
    @State private var account = Account.shared
    @State private var month = false
    private let cols = [GridItem(.adaptive(minimum: 104, maximum: 170), spacing: 10)]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 26) {
                if account.isPerson {
                    NavigationLink { FriendsView() } label: {
                        HStack {
                            Label("Friends", systemImage: "person.2.fill").font(.headline)
                            Spacer()
                            if !friends.incoming.isEmpty {
                                Text("\(friends.incoming.count) new").font(.footnote.weight(.bold)).foregroundStyle(.black)
                                    .padding(.horizontal, 8).padding(.vertical, 3).background(GameTint.green, in: Capsule())
                            } else if !friends.friends.isEmpty {
                                Text("\(friends.friends.count)").font(.subheadline).foregroundStyle(Theme.dim)
                            }
                            Image(systemName: "chevron.right").font(.footnote.weight(.semibold)).foregroundStyle(Theme.faint)
                        }
                        .padding(14)
                        .background(Theme.card, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                    }
                    .buttonStyle(PressScale())
                    .padding(.horizontal, Theme.pad)
                }
                Button {
                    if account.pro { month = true } else { Router.shared.paywall = true }
                } label: {
                    HStack {
                        Label("Your month", systemImage: "calendar").font(.headline)
                        Spacer()
                        if !account.pro { Label("Pro", systemImage: "lock.fill").font(.footnote.weight(.bold)).foregroundStyle(GameTint.gold) }
                        Image(systemName: "chevron.right").font(.footnote.weight(.semibold)).foregroundStyle(Theme.faint)
                    }
                    .padding(14)
                    .background(Theme.card, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                }
                .buttonStyle(PressScale())
                .foregroundStyle(.white)
                .padding(.horizontal, Theme.pad)
                // friends who aren't already in the crew row
                let fw = friends.watching.filter { f in !crew.watching.contains { $0.id == f.id } }
                if !fw.isEmpty { WatchingNowRow(members: fw, title: "Friends watching now") }
                if !crew.watching.isEmpty { WatchingNowRow(members: crew.watching) }
                if !crew.parties.isEmpty { PartiesSection(parties: crew.parties) }
                if lib.continueWatching.isEmpty && lib.myList.isEmpty && crew.watching.isEmpty && crew.parties.isEmpty && !account.isPerson {
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
        .navigationDestination(isPresented: $month) { MonthView() }
        .task {
            await crew.refresh()
            await friends.refresh()
            await coming.refresh()
        }
        .refreshable {
            await crew.refresh(force: true)
            await friends.refresh(force: true)
            await coming.refresh(force: true)
            await Simkl.shared.sync(force: true)
        }
    }
}
