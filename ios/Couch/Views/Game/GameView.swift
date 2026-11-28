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
        .navigationTitle("Games")
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
        .alert("Games", isPresented: Binding(get: { error != nil && home != nil }, set: { if !$0 { error = nil } })) {
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
                Label { Text("Daily 5") } icon: { Image(systemName: "calendar").foregroundStyle(GameTint.gold) }.font(.headline)
                Spacer()
                Label("\(h.streak)", systemImage: "flame.fill")
                    .font(.headline)
                    .foregroundStyle(h.streak > 0 ? GameTint.flame : Theme.faint)
                    .accessibilityLabel("\(h.streak) day streak")
            }
            if h.daily.played {
                HStack(alignment: .firstTextBaseline) {
                    Text("\(h.daily.score ?? 0)").font(.system(size: 40, weight: .heavy)).monospacedDigit().foregroundStyle(GameTint.gold)
                    Text("points").foregroundStyle(Theme.dim)
                    Spacer()
                    if let r = h.ranks.daily { Text("#\(r) today").font(.subheadline.weight(.semibold)) }
                }
                ResultDots(results: h.daily.results ?? [])
                (Text("Next five in ") + Text(Date().addingTimeInterval(Double(h.nextIn)), style: .timer))
                    .font(.footnote).foregroundStyle(Theme.dim).monospacedDigit()
                ShareCardButton(d: GameCardData(daily: true, score: h.daily.score ?? 0, right: (h.daily.results ?? []).filter { $0 >= 0 }.count,
                                               rounds: (h.daily.results ?? []).count, results: h.daily.results ?? [], streak: h.streak,
                                               rank: h.ranks.daily, name: account.discord?.name ?? "", day: GameCardData.day(h.day)))
            } else if let r = h.daily.resume {
                Text("You're on scene \(r.n) of \(r.total) with \(r.score) points. Carry on here, or on any of your devices.")
                    .font(.subheadline).foregroundStyle(Theme.dim)
                Button { start(daily: true) } label: { Label("Continue today's five", systemImage: "play.fill") }
                    .buttonStyle(PillButton())
                    .disabled(starting)
            } else {
                Text("Five scenes, the same for everyone. Easy first, then nicher. One go a day.")
                    .font(.subheadline).foregroundStyle(Theme.dim)
                Button { start(daily: true) } label: { Label("Play today's five", systemImage: "play.fill") }
                    .buttonStyle(PillButton())
                    .disabled(starting)
            }
        }
        .card(GameTint.gold)
    }

    private func endless(_ h: GameHome) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Label { Text("Endless") } icon: { Image(systemName: "infinity").foregroundStyle(GameTint.violet) }.font(.headline)
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
        .card(GameTint.violet)
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
                        .foregroundStyle(i == 0 ? GameTint.gold : i < 3 ? .white : Theme.dim)
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
                    .fill(GameTint.result(r))
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
    func card(_ tint: Color? = nil) -> some View {
        padding(18).frame(maxWidth: .infinity, alignment: .leading)
            .background {
                RoundedRectangle(cornerRadius: 18, style: .continuous).fill(Theme.card)
                if let tint {
                    RoundedRectangle(cornerRadius: 18, style: .continuous)
                        .fill(LinearGradient(colors: [tint.opacity(0.2), .clear], startPoint: .topLeading, endPoint: .bottomTrailing))
                    RoundedRectangle(cornerRadius: 18, style: .continuous).strokeBorder(tint.opacity(0.22), lineWidth: 1)
                }
            }
    }
}
