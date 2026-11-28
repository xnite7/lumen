// One game: scene after scene. A blink first; reveal more for fewer points;
// one guess from four titles.

import SwiftUI

struct GameRoundView: View {
    let first: GameRound
    @State private var round: GameRound?
    @State private var stage = 0
    @State private var picked: Int?
    @State private var result: GameAnswer?
    @State private var done: GameDone?
    @State private var busy = false
    @State private var error: String?
    @State private var snippet = Snippet()
    @State private var haptic = 0
    @State private var party = 0
    @State private var row = 0   // right answers in a row, this sitting
    @Environment(\.dismiss) private var dismiss
    @Environment(\.horizontalSizeClass) private var size

    var body: some View {
        ZStack {
            Theme.bg.ignoresSafeArea()
            // the room's light: warm while you guess, green or red once you have
            RadialGradient(colors: [glow.opacity(0.26), .clear], center: .init(x: 0.5, y: 0), startRadius: 0, endRadius: 520)
                .ignoresSafeArea()
                .animation(.easeInOut(duration: 0.45), value: result?.correct)
            if let d = done {
                summary(d)
            } else if let r = round {
                play(r)
            }
            Confetti(trigger: party).ignoresSafeArea()
        }
        .task {
            if round == nil { await show(first) }
            // `-revealAll` at launch (CI screenshots): show the whole scene
            if ProcessInfo.processInfo.arguments.contains("-revealAll"), let r = round {
                for _ in 0..<(r.stages.count - 1) { if let s = try? await GameAPI.reveal(r.game) { stage = s } }
                replay(r)
            }
        }
        .onDisappear { snippet.close() }
        .sensoryFeedback(.impact, trigger: stage)
        .sensoryFeedback(trigger: haptic) { _, _ in result?.correct == true ? .success : .error }
        .alert("Games", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) {
            Button("OK", role: .cancel) {}
        } message: { Text(error ?? "") }
    }

    private var glow: Color {
        if done != nil { return GameTint.gold }
        guard let res = result else { return round.map { GameTint.level($0.level) } ?? GameTint.gold }
        return res.correct ? GameTint.green : GameTint.red
    }

    // MARK: a round

    private func play(_ r: GameRound) -> some View {
        VStack(spacing: 16) {
            HStack {
                Button {
                    Task {
                        // today's five is only paused; an endless run ends here
                        if let d = await GameAPI.quit(r.game) { withAnimation { done = d } } else { dismiss() }
                    }
                } label: { RoundIcon(symbol: "xmark", size: 36) }
                    .accessibilityLabel("Quit")
                Spacer()
                VStack(spacing: 1) {
                    Text(r.total > 0 ? "Scene \(r.n) of \(r.total)" : "Scene \(r.n)").font(.subheadline.weight(.semibold))
                    Text(["", "easy", "film fan", "deep cut"][min(3, r.level)]).font(.caption2.weight(.semibold)).foregroundStyle(GameTint.level(r.level))
                }
                Spacer()
                VStack(alignment: .trailing, spacing: 2) {
                    CountUp(value: result?.score ?? r.score).font(.headline).foregroundStyle(GameTint.gold)
                    if let l = result?.lives ?? r.lives {
                        HStack(spacing: 2) {
                            ForEach(0..<3, id: \.self) { i in
                                Image(systemName: i < l ? "heart.fill" : "heart").font(.system(size: 10))
                                    .foregroundStyle(i < l ? GameTint.red : Theme.faint)
                            }
                        }
                        .accessibilityLabel("\(l) lives")
                    }
                }
                .frame(minWidth: 36, alignment: .trailing)
            }

            screen(r)

            if result == nil {
                HStack(spacing: 6) {
                    ForEach(r.stages.indices, id: \.self) { i in
                        Capsule().fill(i <= stage ? GameTint.gold : Color.white.opacity(0.18)).frame(height: 4)
                    }
                }
                HStack {
                    Text(label(r.stages[stage])).font(.footnote).foregroundStyle(Theme.dim)
                    Spacer()
                    Text("worth \(r.points[stage])").font(.footnote.weight(.bold)).monospacedDigit().foregroundStyle(GameTint.gold)
                        .contentTransition(.numericText())
                }
            }

            VStack(spacing: 8) {
                ForEach(Array(r.options.enumerated()), id: \.offset) { i, o in
                    Button { answer(r, i) } label: { option(o, i) }
                        .buttonStyle(PressScale())
                        .disabled(result != nil || busy)
                }
            }

            Spacer(minLength: 0)

            if let res = result {
                Button {
                    Task {
                        if let d = res.done { withAnimation { done = d } } else if let n = res.next { await show(n) }
                    }
                } label: { Text(res.done != nil ? "See how you did" : "Next scene") }
                    .buttonStyle(PillButton())
            } else if stage < r.stages.count - 1 {
                Button { reveal(r) } label: {
                    Label("Show more  (−\(r.points[stage] - r.points[stage + 1]))", systemImage: "eye")
                }
                .buttonStyle(PillButton(primary: false))
                .disabled(busy)
            }
        }
        .padding(.horizontal, Theme.pad)
        .padding(.vertical, 12)
        .frame(maxWidth: 620)
        .animation(.snappy, value: stage)
        .animation(.snappy, value: result?.answer.index)
    }

    /// the scene: dark until you play your bit of it
    private func screen(_ r: GameRound) -> some View {
        Color(white: 0.06)
            .aspectRatio(16 / 9, contentMode: .fit)
            .overlay {
                SnippetView(player: snippet.player).opacity(snippet.showing ? 1 : 0)
                if !snippet.showing {
                    if snippet.failed {
                        VStack(spacing: 10) {
                            Label("couldn't load this scene", systemImage: "exclamationmark.triangle").font(.footnote).foregroundStyle(Theme.dim)
                            Button("Try again") { Task { await reload(r) } }.buttonStyle(.bordered)
                        }
                    } else if !snippet.ready {
                        ProgressView()
                    } else {
                        Button { replay(r) } label: {
                            VStack(spacing: 8) {
                                Image(systemName: "play.fill").font(.system(size: 22, weight: .bold))
                                    .frame(width: 58, height: 58).background(.ultraThinMaterial, in: Circle())
                                Text(result != nil ? "watch the scene" : "play it again").font(.caption).foregroundStyle(Theme.dim)
                            }
                        }
                        .foregroundStyle(.white)
                    }
                }
            }
            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous)
                .strokeBorder(result.map { $0.correct ? GameTint.green : GameTint.red } ?? Theme.line, lineWidth: result == nil ? 0.5 : 2))
            .overlay(alignment: .top) {
                if let res = result { verdict(res).padding(10).transition(.scale(scale: 0.6).combined(with: .opacity)) }
            }
    }

    /// what just happened, said loudly
    private func verdict(_ res: GameAnswer) -> some View {
        let praise = ["In a blink!", "One second!", "Three seconds", "Five seconds", "Got it"][min(4, stage)]
        return HStack(spacing: 8) {
            Image(systemName: res.correct ? "checkmark.seal.fill" : "xmark.seal.fill")
            if res.correct {
                Text(praise)
                if res.gained > 0 { Text("+\(res.gained)").monospacedDigit() }
                if row >= 2 {
                    Label("\(row)", systemImage: "flame.fill").labelStyle(.titleAndIcon).foregroundStyle(.black.opacity(0.7))
                }
            } else {
                Text("Not this time")
            }
        }
        .font(.subheadline.weight(.heavy))
        .foregroundStyle(.black)
        .padding(.horizontal, 14).padding(.vertical, 8)
        .background(res.correct ? GameTint.green : GameTint.red, in: Capsule())
        .shadow(color: .black.opacity(0.4), radius: 8, y: 3)
        .accessibilityElement(children: .combine)
    }

    private func option(_ o: GameRound.Option, _ i: Int) -> some View {
        let right = result?.answer.index == i
        let wrong = result != nil && picked == i && !right
        return HStack {
            Text(o.title).font(.callout.weight(.semibold)).lineLimit(2).multilineTextAlignment(.leading)
            if !o.year.isEmpty { Text(o.year).font(.footnote).opacity(0.6) }
            Spacer(minLength: 4)
            if right { Image(systemName: "checkmark.circle.fill") }
            if wrong { Image(systemName: "xmark.circle.fill") }
            if right, let g = result?.gained, g > 0 { Text("+\(g)").font(.subheadline.weight(.bold)).monospacedDigit() }
        }
        .padding(.horizontal, 16)
        .frame(maxWidth: .infinity, minHeight: 52)
        .foregroundStyle(right ? Color.black : wrong ? GameTint.red : Color.white)
        .background(right ? AnyShapeStyle(GameTint.green) : wrong ? AnyShapeStyle(GameTint.red.opacity(0.16)) : AnyShapeStyle(Theme.card),
                    in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay { if wrong { RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(GameTint.red.opacity(0.8), lineWidth: 1.5) } }
        .opacity(result != nil && !right && !wrong ? 0.4 : 1)
        .scaleEffect(right ? 1.03 : 1)
        .animation(.spring(duration: 0.35, bounce: 0.5), value: right)
    }

    private func label(_ s: Double) -> String {
        s == 0 ? "the whole scene" : s < 1 ? "a blink" : "the first \(Int(s)) second\(s == 1 ? "" : "s")"
    }

    // MARK: moves

    private func show(_ r: GameRound) async {
        result = nil
        picked = nil
        stage = r.stage
        round = r
        guard let u = await GameAPI.clip(r) else { error = "couldn't reach the couch server"; return }
        snippet.load(u)
        snippet.play(r.stages[r.stage] == 0 ? nil : r.stages[r.stage])
    }

    /// the scene again from the server (a fresh link), at the stage you're on
    private func reload(_ r: GameRound) async {
        guard let u = await GameAPI.clip(r) else { return }
        snippet.load(u)
        replay(r)
    }

    private func replay(_ r: GameRound) {
        let s = result != nil ? 0 : r.stages[stage]
        snippet.play(s == 0 ? nil : s)
    }

    private func reveal(_ r: GameRound) {
        busy = true
        Task {
            do {
                stage = try await GameAPI.reveal(r.game)
                replay(r)
            } catch { self.error = error.localizedDescription }
            busy = false
        }
    }

    private func answer(_ r: GameRound, _ i: Int) {
        busy = true
        picked = i
        Task {
            do {
                let a = try await GameAPI.answer(r.game, pick: i)
                row = a.correct ? row + 1 : 0
                withAnimation(.spring(duration: 0.4, bounce: 0.45)) { result = a }
                haptic += 1
                if a.correct { party += 1 }
                snippet.play(nil) // now the whole scene
            } catch {
                picked = nil
                self.error = error.localizedDescription
            }
            busy = false
        }
    }

    // MARK: the end

    private func summary(_ d: GameDone) -> some View {
        let best = d.mode != "daily" && d.score >= d.bestRun && d.score > 0
        return VStack(spacing: 18) {
            Spacer()
            Text(d.mode == "daily" ? "Today's five" : "Run over").font(.headline).foregroundStyle(Theme.dim)
            CountUp(value: d.score)
                .font(.system(size: 76, weight: .black))
                .foregroundStyle(LinearGradient(colors: [.white, GameTint.gold], startPoint: .top, endPoint: .bottom))
                .minimumScaleFactor(0.5).lineLimit(1)
            Text("points · \(d.right) of \(d.rounds) right").font(.subheadline.weight(.medium)).foregroundStyle(Theme.dim)
            ResultDots(results: d.results)
            HStack(spacing: 10) {
                if d.mode == "daily", d.streak > 0 {
                    Label("\(d.streak) day streak", systemImage: "flame.fill").chip(GameTint.flame)
                }
                if let r = d.rank {
                    Label("#\(r) \(d.mode == "daily" ? "today" : "all time")", systemImage: "trophy.fill").chip(GameTint.gold)
                }
                if best { Label("Your best run", systemImage: "star.fill").chip(GameTint.violet) }
            }
            Spacer()
            ShareCardButton(d: GameCardData(daily: d.mode == "daily", score: d.score, right: d.right, rounds: d.rounds, results: d.results,
                                            streak: d.streak, rank: d.rank, name: Account.shared.discord?.name ?? "", day: GameCardData.day()))
            if d.mode != "daily" {
                Button {
                    Task {
                        do {
                            let r = try await GameAPI.start(daily: false)
                            done = nil
                            row = 0
                            await show(r)
                        } catch { self.error = error.localizedDescription }
                    }
                } label: { Label("Go again", systemImage: "arrow.clockwise") }
                    .buttonStyle(PillButton(primary: false))
            }
            Button("Done") { dismiss() }.buttonStyle(PillButton())
        }
        .padding(Theme.pad)
        .frame(maxWidth: 560)
        .onAppear {
            snippet.close()
            if best || d.right * 5 >= d.rounds * 3, d.right > 0 { party += 1 }
        }
    }
}

extension View {
    /// a small tinted tag
    func chip(_ tint: Color) -> some View {
        font(.footnote.weight(.bold)).foregroundStyle(tint)
            .padding(.horizontal, 10).padding(.vertical, 6)
            .background(tint.opacity(0.15), in: Capsule())
    }
}
