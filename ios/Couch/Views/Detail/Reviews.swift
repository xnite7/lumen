// Reviews on a title's page: everyone's, the most liked first. Anyone can
// read; writing and liking need an account (Apple or Discord). Every review
// can be reported, and its writer blocked.

import SwiftUI

struct TitleReview: Identifiable, Hashable {
    var id: String
    var who: String
    var name: String
    var handle: String?
    var avatar: String?
    var rating: Int?
    var text: String?
    var at: Date
    var likes: Int
    var liked: Bool
    var mine: Bool
}

@MainActor @Observable
final class Reviews {
    let meta: MetaPreview
    private(set) var list: [TitleReview] = []
    private(set) var average: Double?
    private(set) var count = 0
    private(set) var admin = false
    private(set) var loaded = false

    init(_ m: MetaPreview) { meta = m }

    private func call(_ method: String, _ path: String, _ body: [String: Any]? = nil) async -> (Int, [String: Any]) {
        // (reading works signed out too: as a guest)
        guard let tok = await Account.shared.token(guest: true) else { return (0, [:]) }
        var r = URLRequest(url: URL(string: crewURL + "/v1/me" + path)!)
        r.httpMethod = method
        r.timeoutInterval = 12
        r.cachePolicy = .reloadIgnoringLocalCacheData
        r.setValue("Bearer \(tok)", forHTTPHeaderField: "authorization")
        if let body {
            r.setValue("application/json", forHTTPHeaderField: "content-type")
            r.httpBody = try? JSONSerialization.data(withJSONObject: body)
        }
        guard let (d, res) = try? await Net.session.data(for: r), let h = res as? HTTPURLResponse else { return (0, [:]) }
        return (h.statusCode, ((try? JSONSerialization.jsonObject(with: d)) as? [String: Any]) ?? [:])
    }

    private var path: String { "/reviews/\(meta.id)" }

    func load() async {
        guard meta.id.hasPrefix("tt") else { return }
        let (code, v) = await call("GET", path)
        guard code == 200 else { return }
        list = ((v["reviews"] as? [[String: Any]]) ?? []).compactMap { r in
            guard let id = r["id"] as? String, let who = r["who"] as? String else { return nil }
            return TitleReview(id: id, who: who, name: r["name"] as? String ?? "someone", handle: r["handle"] as? String, avatar: r["avatar"] as? String,
                               rating: (r["rating"] as? NSNumber)?.intValue, text: (r["text"] as? String).flatMap { $0.isEmpty ? nil : $0 },
                               at: Date(timeIntervalSince1970: ((r["at"] as? NSNumber)?.doubleValue ?? 0) / 1000),
                               likes: (r["likes"] as? NSNumber)?.intValue ?? 0, liked: r["liked"] as? Bool ?? false, mine: r["mine"] as? Bool ?? false)
        }
        average = (v["average"] as? NSNumber)?.doubleValue
        count = (v["count"] as? NSNumber)?.intValue ?? 0
        admin = v["admin"] as? Bool ?? false
        loaded = true
    }

    var mine: TitleReview? { list.first(where: \.mine) }

    /// Returns what went wrong, or nil.
    func post(rating: Int, text: String) async -> String? {
        let (code, v) = await call("POST", path, ["rating": rating, "text": text, "title": ["type": meta.type.rawValue, "name": meta.name, "poster": meta.posterURL?.absoluteString ?? ""]])
        if code == 200 {
            await load()
            return nil
        }
        return v["error"] as? String ?? (code == 0 ? "couldn't reach the couch server" : "that didn't work")
    }

    func deleteMine() async {
        _ = await call("DELETE", path)
        await load()
    }

    func like(_ r: TitleReview) async {
        guard let i = list.firstIndex(where: { $0.id == r.id }) else { return }
        // shown at once, corrected by what the server says
        list[i].liked.toggle()
        list[i].likes += list[i].liked ? 1 : -1
        let (code, v) = await call("POST", path + "/like", ["id": r.id, "on": list[i].liked])
        if code == 200, let j = list.firstIndex(where: { $0.id == r.id }) {
            list[j].likes = (v["likes"] as? NSNumber)?.intValue ?? list[j].likes
            list[j].liked = v["liked"] as? Bool ?? list[j].liked
        } else {
            await load()
        }
    }

    func report(_ r: TitleReview, why: String) async {
        _ = await call("POST", "/report", ["id": r.who, "what": "review", "why": why, "title": meta.id, "ref": r.id])
        // reported = out of your sight
        list.removeAll { $0.id == r.id }
    }

    func block(_ r: TitleReview) async {
        _ = await call("POST", "/friends/block", ["id": r.who])
        list.removeAll { $0.who == r.who }
    }

    /// (the server's owner) take it down for everyone
    func remove(_ r: TitleReview) async {
        _ = await call("POST", path + "/remove", ["id": r.id])
        await load()
    }
}

struct ReviewsSection: View {
    @State private var reviews: Reviews
    @State private var account = Account.shared
    @State private var writing = false
    @State private var reporting: TitleReview?
    @State private var needAccount = false

    init(meta: MetaPreview) { _reviews = State(initialValue: Reviews(meta)) }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .firstTextBaseline) {
                Text("Reviews").font(.rowTitle)
                if let a = reviews.average, reviews.count > 0 {
                    Label(String(format: "%.1f", a / 2), systemImage: "star.fill").font(.subheadline.weight(.semibold)).foregroundStyle(GameTint.gold)
                    Text("(\(reviews.count))").font(.footnote).foregroundStyle(Theme.faint)
                }
                Spacer()
                Button {
                    if account.isPerson { writing = true } else { needAccount = true }
                } label: {
                    Label(reviews.mine == nil ? "Write" : "Edit yours", systemImage: "square.and.pencil").font(.subheadline.weight(.semibold))
                }
            }
            if reviews.loaded, reviews.list.isEmpty {
                Text("No reviews yet. Be the first.").font(.subheadline).foregroundStyle(Theme.dim)
            }
            ForEach(reviews.list) { r in row(r) }
        }
        .task { await reviews.load() }
        .sheet(isPresented: $writing) { WriteReview(reviews: reviews) }
        .confirmationDialog("Report this review?", isPresented: Binding(get: { reporting != nil }, set: { if !$0 { reporting = nil } }), titleVisibility: .visible) {
            ForEach(["Hateful or abusive", "Spam", "Spoilers without warning", "Something else"], id: \.self) { why in
                Button(why) {
                    if let r = reporting { Task { await reviews.report(r, why: why) } }
                    reporting = nil
                }
            }
        } message: {
            Text("It's hidden for you right away and sent to the moderators.")
        }
        .alert("Sign in first", isPresented: $needAccount) {
            Button("OK", role: .cancel) {}
        } message: {
            Text("Reviews and likes need a couch account. Settings → Sign in with Apple (or Discord).")
        }
    }

    private func row(_ r: TitleReview) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 10) {
                AsyncImage(url: r.avatar.flatMap(URL.init(string:))) { $0.resizable() } placeholder: {
                    Circle().fill(Color(white: 0.2)).overlay(Text(String(r.name.prefix(1)).uppercased()).font(.caption.weight(.bold)))
                }
                .frame(width: 30, height: 30).clipShape(Circle())
                VStack(alignment: .leading, spacing: 1) {
                    Text(r.name).font(.subheadline.weight(.semibold)).lineLimit(1)
                    Text(r.at, format: .relative(presentation: .named)).font(.caption2).foregroundStyle(Theme.faint)
                }
                Spacer()
                if let s = r.rating, s > 0 { Stars(score: s) }
            }
            if let t = r.text { Text(t).font(.subheadline).foregroundStyle(.white.opacity(0.9)).fixedSize(horizontal: false, vertical: true) }
            HStack {
                Button {
                    if account.isPerson { Task { await reviews.like(r) } } else { needAccount = true }
                } label: {
                    Label(r.likes > 0 ? "\(r.likes)" : "Like", systemImage: r.liked ? "heart.fill" : "heart")
                        .font(.footnote.weight(.semibold)).foregroundStyle(r.liked ? GameTint.red : Theme.dim)
                        .contentTransition(.numericText())
                }
                .disabled(r.mine)
                .sensoryFeedback(.impact(weight: .light), trigger: r.liked)
                Spacer()
                Menu {
                    if r.mine {
                        Button("Delete my review", systemImage: "trash", role: .destructive) { Task { await reviews.deleteMine() } }
                    } else {
                        Button("Report", systemImage: "exclamationmark.bubble") { reporting = r }
                        if account.isPerson {
                            Button("Block \(r.name)", systemImage: "hand.raised", role: .destructive) { Task { await reviews.block(r) } }
                        }
                        if reviews.admin {
                            Button("Remove for everyone", systemImage: "trash", role: .destructive) { Task { await reviews.remove(r) } }
                        }
                    }
                } label: {
                    Image(systemName: "ellipsis").font(.footnote.weight(.bold)).foregroundStyle(Theme.dim).frame(width: 32, height: 24)
                }
                .accessibilityLabel("More")
            }
        }
        .padding(14)
        .background(Theme.card, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
    }
}

/// five stars for a score out of ten
struct Stars: View {
    let score: Int
    var size: CGFloat = 11
    var body: some View {
        HStack(spacing: 1) {
            ForEach(1...5, id: \.self) { i in
                Image(systemName: score >= i * 2 ? "star.fill" : score == i * 2 - 1 ? "star.leadinghalf.filled" : "star")
                    .font(.system(size: size)).foregroundStyle(score >= i * 2 - 1 ? GameTint.gold : Theme.faint)
            }
        }
        .accessibilityLabel("\(Double(score) / 2, specifier: "%.1f") out of 5 stars")
    }
}

struct WriteReview: View {
    let reviews: Reviews
    @State private var stars = 0
    @State private var text = ""
    @State private var error: String?
    @State private var busy = false
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    HStack(spacing: 10) {
                        Spacer()
                        ForEach(1...5, id: \.self) { i in
                            Button { stars = stars == i ? 0 : i } label: {
                                Image(systemName: i <= stars ? "star.fill" : "star").font(.title).foregroundStyle(i <= stars ? GameTint.gold : Theme.faint)
                            }
                            .buttonStyle(.plain)
                            .accessibilityLabel("\(i) star\(i == 1 ? "" : "s")")
                        }
                        Spacer()
                    }
                    .padding(.vertical, 4)
                }
                Section {
                    TextField("What did you think?", text: $text, axis: .vertical).lineLimit(4...10)
                } footer: {
                    Text(error ?? "Everyone can read this. Be decent: reviews can be reported and removed.").foregroundStyle(error == nil ? Theme.dim : GameTint.red)
                }
            }
            .navigationTitle(reviews.meta.name)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Post") {
                        busy = true
                        Task {
                            error = await reviews.post(rating: stars * 2, text: String(text.trimmingCharacters(in: .whitespacesAndNewlines).prefix(1200)))
                            busy = false
                            if error == nil { dismiss() }
                        }
                    }
                    .disabled(busy || (stars == 0 && text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty))
                }
            }
            .onAppear {
                if let m = reviews.mine {
                    stars = ((m.rating ?? 0) + 1) / 2
                    text = m.text ?? ""
                }
            }
        }
        .presentationDetents([.medium, .large])
    }
}
