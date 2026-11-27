// The Play tab: today's five scenes (with your streak), an endless run, and
// the leaderboards.

import SwiftUI

struct GameHomeView: View {
    @State private var home: GameHome?
    @State private var error: String?
    @State private var round: GameRound?
    @State private var starting = false
    @State private var board = 0
    @State private var account = Account.shared
    @State private var router = Router.shared

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                Text("Guess the movie from its most famous scene. The less you watch, the more you score.")
                    .font(.subheadline).foregroundStyle(Theme.dim)
                if let h = home, h.paused == true {
                    Label("Scenes is taking a break while we sort out where the clips come from. Back soon.", systemImage: "pause.circle")
                        .font(.subheadline).card()
                    boards(h)
                } else if let h = home {
                    daily(h)
                    endless(h)
                    boards(h)
                } else if let error {
                    ContentUnavailableView {
                        Label("couldn't load", systemImage: "wifi.slash")
                    } description: { Text(error) } actions: {
                        Button("Try again") { Task { await load() } }.buttonStyle(.bordered)
                    }
                } else {
                    ProgressView().frame(maxWidth: .infinity).padding(.top, 60)
                }
            }
            .padding(Theme.pad)
            .frame(maxWidth: 640)
            .frame(maxWidth: .infinity)
        }
        .background(Theme.bg)
        .navigationTitle("Scenes")
        .task {
            await load()
            // opened by a link (couch://play/daily): straight into the game
            if let m = router.autoStart, home?.paused != true {
                router.autoStart = nil
                if m == "daily", home?.daily.played == true { return }
                start(daily: m == "daily")
            }
        }
        .refreshable { await load() }
        .task(id: account.pro) { await load() }
        .fullScreenCover(item: $round, onDismiss: { Task { await load() } }) { r in
            GameRoundView(first: r)
        }
        .alert("Scenes", isPresented: Binding(get: { error != nil && home != nil }, set: { if !$0 { error = nil } })) {
            Button("OK", role: .cancel) {}
        } message: { Text(error ?? "") }
    }

    private func load() async {
        do { home = try await GameAPI.home(); error = nil } catch { self.error = error.localizedDescription }
    }

    private func start(daily: Bool) {
        guard !starting else { return }
        starting = true
        Task {
            do { round = try await GameAPI.start(daily: daily) } catch { self.error = error.localizedDescription }
            starting = false
        }
    }

    // MARK: cards

    private func daily(_ h: GameHome) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Label("Daily 5", systemImage: "calendar").font(.headline)
                Spacer()
                Label("\(h.streak)", systemImage: "flame.fill")
                    .font(.headline)
                    .foregroundStyle(h.streak > 0 ? .white : Theme.faint)
                    .accessibilityLabel("\(h.streak) day streak")
            }
            if h.daily.played {
                HStack(alignment: .firstTextBaseline) {
                    Text("\(h.daily.score ?? 0)").font(.system(size: 40, weight: .heavy)).monospacedDigit()
                    Text("points").foregroundStyle(Theme.dim)
                    Spacer()
                    if let r = h.ranks.daily { Text("#\(r) today").font(.subheadline.weight(.semibold)) }
                }
                ResultDots(results: h.daily.results ?? [])
                HStack {
                    Text("Next five in ") + Text(Date().addingTimeInterval(Double(h.nextIn)), style: .timer)
                    Spacer()
                    ShareLink(item: shareText(day: h.day, score: h.daily.score ?? 0, results: h.daily.results ?? [], streak: h.streak)) {
                        Label("Share", systemImage: "square.and.arrow.up")
                    }
                }
                .font(.footnote).foregroundStyle(Theme.dim).monospacedDigit()
            } else {
                Text("Five scenes, the same for everyone. Easy first, then nicher. One go a day.")
                    .font(.subheadline).foregroundStyle(Theme.dim)
                Button { start(daily: true) } label: { Label("Play today's five", systemImage: "play.fill") }
                    .buttonStyle(PillButton())
                    .disabled(starting)
            }
        }
        .card()
    }

    private func endless(_ h: GameHome) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Label("Endless", systemImage: "infinity").font(.headline)
                Spacer()
                if h.bestRun > 0 { Text("best \(h.bestRun)").font(.subheadline).foregroundStyle(Theme.dim).monospacedDigit() }
            }
            Text("Play whenever. It starts easy and gets nicher. Three misses and the run is over.")
                .font(.subheadline).foregroundStyle(Theme.dim)
            if h.pro ?? account.pro {
                Button { start(daily: false) } label: { Label("Start a run", systemImage: "play.fill") }
                    .buttonStyle(PillButton(primary: false))
                    .disabled(starting)
            } else {
                Button { router.paywall = true } label: { Label("Unlock with couch Pro", systemImage: "lock.fill") }
                    .buttonStyle(PillButton(primary: false))
            }
        }
        .card()
    }

    private func boards(_ h: GameHome) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Leaderboard").font(.rowTitle)
            Picker("", selection: $board) {
                Text("Today").tag(0)
                Text("Best runs").tag(1)
            }
            .pickerStyle(.segmented)
            let rows = board == 0 ? h.boards.daily : h.boards.runs
            if rows.isEmpty {
                Text(board == 0 ? "Nobody has played today yet. Be first." : "No runs yet.").font(.subheadline).foregroundStyle(Theme.dim).padding(.vertical, 8)
            }
            ForEach(Array(rows.enumerated()), id: \.element.id) { i, r in
                HStack(spacing: 12) {
                    Text("\(i + 1)").font(.subheadline.weight(.bold)).monospacedDigit().frame(width: 26, alignment: .trailing)
                        .foregroundStyle(i < 3 ? .white : Theme.dim)
                    AsyncImage(url: r.avatar.flatMap(URL.init(string:))) { $0.resizable() } placeholder: { Color(white: 0.2) }
                        .frame(width: 30, height: 30).clipShape(Circle())
                    Text(r.name).font(.subheadline.weight(r.id == account.discord?.id ? .bold : .regular)).lineLimit(1)
                    Spacer()
                    Text("\(r.score)").font(.subheadline.weight(.semibold)).monospacedDigit()
                }
                .padding(.vertical, 2)
            }
            if !h.ranked {
                Button { router.tab = .settings } label: {
                    Label("Sign in with Discord to get on the leaderboard", systemImage: "person.crop.circle.badge.plus")
                        .font(.footnote.weight(.semibold))
                }
                .padding(.top, 4)
            }
        }
        .padding(.top, 6)
    }
}

/// One square per scene: brighter = got it sooner, hollow = missed.
struct ResultDots: View {
    let results: [Int]
    var body: some View {
        HStack(spacing: 6) {
            ForEach(Array(results.enumerated()), id: \.offset) { _, r in
                RoundedRectangle(cornerRadius: 5, style: .continuous)
                    .fill(r < 0 ? Color.clear : Color.white.opacity(1 - Double(r) * 0.18))
                    .overlay { if r < 0 { RoundedRectangle(cornerRadius: 5, style: .continuous).strokeBorder(Theme.faint, lineWidth: 1.5) } }
                    .frame(width: 26, height: 26)
            }
        }
    }
}

func shareText(day: String, score: Int, results: [Int], streak: Int) -> String {
    let marks = results.map { $0 < 0 ? "⬛" : ["🟩", "🟩", "🟨", "🟧", "🟥"][min(4, $0)] }.joined()
    return "couch scenes \(day)\n\(marks)  \(score) pts" + (streak > 1 ? "  🔥\(streak)" : "") + "\n\(crewURL)"
}

extension View {
    func card() -> some View {
        padding(18).frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.card, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
    }
}
