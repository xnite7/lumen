// My couch, the parts about other people and about what's next: who's
// watching now, watch parties, and the episodes coming up.

import SwiftUI

struct WatchingNowRow: View {
    let members: [CrewMember]
    var title = "On the couch now"
    var body: some View {
        Shelf(title: title) {
            ForEach(members) { m in
                let card = VStack(alignment: .leading, spacing: 8) {
                    Art(url: m.preview?.backdropURL)
                        .aspectRatio(16 / 9, contentMode: .fill)
                        .frame(width: 230)
                        .overlay(alignment: .bottom) {
                            if m.duration > 0 {
                                GeometryReader { g in
                                    Capsule().fill(.white).frame(width: g.size.width * min(1, m.pos / m.duration), height: 3)
                                        .frame(maxHeight: .infinity, alignment: .bottom)
                                }
                            }
                        }
                        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                    HStack(spacing: 8) {
                        AsyncImage(url: m.avatar.flatMap(URL.init(string:))) { $0.resizable() } placeholder: { Color(white: 0.2) }
                            .frame(width: 26, height: 26).clipShape(Circle())
                        VStack(alignment: .leading, spacing: 1) {
                            Text(m.name).font(.footnote.weight(.semibold)).lineLimit(1)
                            Text([m.title, m.episode].compactMap { $0 }.joined(separator: " · ") + (m.paused ? "  (paused)" : ""))
                                .font(.caption).foregroundStyle(Theme.dim).lineLimit(1)
                        }
                    }
                }
                .frame(width: 230, alignment: .leading)
                if let p = m.preview {
                    NavigationLink(value: p) { card }.buttonStyle(PressScale())
                } else {
                    card
                }
            }
        }
    }
}

struct PartiesSection: View {
    let parties: [WatchParty]
    @State private var busy: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Watch parties").font(.rowTitle)
            ForEach(parties) { p in
                HStack(alignment: .top, spacing: 12) {
                    NavigationLink(value: p.title) {
                        Art(url: p.title.posterURL).aspectRatio(2 / 3, contentMode: .fill).frame(width: 64)
                            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                    }
                    .buttonStyle(PressScale())
                    VStack(alignment: .leading, spacing: 4) {
                        Text(p.title.name).font(.headline).lineLimit(2)
                        Text(p.at, format: .dateTime.weekday(.wide).day().month().hour().minute())
                            .font(.subheadline.weight(.semibold)).foregroundStyle(GameTint.gold)
                        Text("hosted by \(p.hostName)" + (p.going.count > 1 ? " · \(p.going.count) going" : ""))
                            .font(.caption).foregroundStyle(Theme.dim)
                        if !p.note.isEmpty { Text(p.note).font(.caption).foregroundStyle(Theme.dim).lineLimit(2) }
                        HStack(spacing: 8) {
                            Button { answer(p, true) } label: {
                                Label(p.mine == true ? "You're in" : "I'm in", systemImage: p.mine == true ? "checkmark" : "ticket")
                            }
                            .buttonStyle(.borderedProminent).tint(p.mine == true ? GameTint.green : .white).foregroundStyle(.black)
                            if p.mine != false {
                                Button("Can't make it") { answer(p, false) }.buttonStyle(.bordered)
                            } else {
                                Text("You said no").font(.caption).foregroundStyle(Theme.faint)
                            }
                        }
                        .font(.footnote.weight(.semibold))
                        .controlSize(.small)
                        .disabled(busy == p.id)
                        .padding(.top, 2)
                    }
                    Spacer(minLength: 0)
                }
                .padding(12)
                .background(Theme.card, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
            }
        }
        .padding(.horizontal, Theme.pad)
    }

    private func answer(_ p: WatchParty, _ yes: Bool) {
        busy = p.id
        Task {
            await Crew.shared.rsvp(p, yes)
            busy = nil
        }
    }
}

struct ComingUpSection: View {
    let items: [Upcoming]

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Coming up").font(.rowTitle)
            ForEach(items.prefix(12)) { u in
                NavigationLink(value: u.show) {
                    HStack(spacing: 12) {
                        VStack(spacing: 0) {
                            Text(u.date, format: .dateTime.month(.abbreviated)).font(.caption2.weight(.bold)).foregroundStyle(GameTint.red)
                            Text(u.date, format: .dateTime.day()).font(.title3.weight(.heavy)).monospacedDigit()
                        }
                        .frame(width: 44)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(u.show.name).font(.subheadline.weight(.semibold)).lineLimit(1)
                            Text("\(u.episode.code)  \(u.episode.label)").font(.caption).foregroundStyle(Theme.dim).lineLimit(1)
                        }
                        Spacer(minLength: 4)
                        Text(u.date <= .now ? "Out now" : u.date.formatted(.relative(presentation: .named)))
                            .font(.caption.weight(.semibold)).foregroundStyle(u.date <= .now ? GameTint.green : Theme.dim)
                    }
                    .padding(.vertical, 8).padding(.horizontal, 12)
                    .background(Theme.card, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                }
                .buttonStyle(PressScale())
            }
        }
        .padding(.horizontal, Theme.pad)
    }
}
