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
    @Environment(\.dismiss) private var dismiss
    @Environment(\.horizontalSizeClass) private var size

    var body: some View {
        ZStack {
            Theme.bg.ignoresSafeArea()
            if let d = done {
                summary(d)
            } else if let r = round {
                play(r)
            }
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
        .alert("Scenes", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) {
            Button("OK", role: .cancel) {}
        } message: { Text(error ?? "") }
    }

    // MARK: a round

    private func play(_ r: GameRound) -> some View {
        VStack(spacing: 16) {
            HStack {
                Button {
                    Task { await GameAPI.quit(r.game); dismiss() }
                } label: { RoundIcon(symbol: "xmark", size: 36) }
                    .accessibilityLabel("Quit")
                Spacer()
                VStack(spacing: 1) {
                    Text(r.total > 0 ? "Scene \(r.n) of \(r.total)" : "Scene \(r.n)").font(.subheadline.weight(.semibold))
                    Text(["", "easy", "film fan", "deep cut"][min(3, r.level)]).font(.caption2).foregroundStyle(Theme.dim)
                }
                Spacer()
                VStack(alignment: .trailing, spacing: 2) {
                    Text("\(result?.score ?? r.score)").font(.headline).monospacedDigit().contentTransition(.numericText())
                    if let l = result?.lives ?? r.lives {
                        HStack(spacing: 2) {
                            ForEach(0..<3, id: \.self) { i in
                                Image(systemName: i < l ? "heart.fill" : "heart").font(.system(size: 10))
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
                        Capsule().fill(i <= stage ? Color.white : Color.white.opacity(0.18)).frame(height: 4)
                    }
                }
                HStack {
                    Text(label(r.stages[stage])).font(.footnote).foregroundStyle(Theme.dim)
                    Spacer()
                    Text("worth \(r.points[stage])").font(.footnote.weight(.semibold)).monospacedDigit()
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
                        Label("couldn't load this scene", systemImage: "exclamationmark.triangle").font(.footnote).foregroundStyle(Theme.dim)
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
            .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(Theme.line, lineWidth: 0.5))
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
        .foregroundStyle(right ? Color.black : Color.white)
        .background(right ? AnyShapeStyle(.white) : AnyShapeStyle(Theme.card), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay { if wrong { RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(.white.opacity(0.6), lineWidth: 1.5) } }
        .opacity(result != nil && !right && !wrong ? 0.4 : 1)
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
                withAnimation(.snappy) { result = a }
                haptic += 1
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
        VStack(spacing: 18) {
            Spacer()
            Text(d.mode == "daily" ? "Today's five" : "Run over").font(.headline).foregroundStyle(Theme.dim)
            Text("\(d.score)").font(.system(size: 72, weight: .heavy)).monospacedDigit()
            Text("\(d.right) of \(d.rounds) right").foregroundStyle(Theme.dim)
            if d.mode == "daily" {
                ResultDots(results: d.results)
                Label("\(d.streak) day streak", systemImage: "flame.fill").font(.headline)
            } else if d.score >= d.bestRun, d.score > 0 {
                Label("Your best run", systemImage: "trophy.fill").font(.headline)
            }
            if let r = d.rank { Text("#\(r) on the leaderboard").font(.subheadline.weight(.semibold)) }
            Spacer()
            if d.mode == "daily" {
                ShareLink(item: shareText(day: String(ISO8601DateFormatter().string(from: .now).prefix(10)), score: d.score, results: d.results, streak: d.streak)) {
                    Label("Share", systemImage: "square.and.arrow.up")
                }
                .buttonStyle(PillButton(primary: false))
            } else {
                Button {
                    Task {
                        do {
                            let r = try await GameAPI.start(daily: false)
                            done = nil
                            await show(r)
                        } catch { self.error = error.localizedDescription }
                    }
                } label: { Label("Go again", systemImage: "arrow.clockwise") }
                    .buttonStyle(PillButton(primary: false))
            }
            Button("Done") { dismiss() }.buttonStyle(PillButton())
        }
        .padding(Theme.pad)
        .frame(maxWidth: 480)
        .onAppear { snippet.close() }
    }
}
