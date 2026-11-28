// Friends: add by couch name, answer requests, see who's watching what.

import SwiftUI

struct FriendsView: View {
    @State private var friends = Friends.shared
    @State private var adding = ""
    @State private var busy = false
    @State private var note: String?
    @State private var picking = false

    var body: some View {
        List {
            if friends.loaded, friends.handle == nil {
                Section {
                    Button { picking = true } label: { Label("Pick your couch name", systemImage: "at") }
                } footer: {
                    Text("Friends find you by this name. You need one before you can add anyone.")
                }
            } else {
                Section {
                    HStack {
                        Image(systemName: "at").foregroundStyle(Theme.dim)
                        TextField("a friend's couch name", text: $adding)
                            .textInputAutocapitalization(.never).autocorrectionDisabled().submitLabel(.send)
                            .onSubmit(add)
                        if busy { ProgressView() } else {
                            Button("Add", action: add).disabled(adding.trimmingCharacters(in: .whitespaces).count < 3)
                        }
                    }
                } header: {
                    Text("Add a friend")
                } footer: {
                    if let h = friends.handle {
                        Text("You're @\(h). ") + Text(note ?? "")
                    }
                }
            }

            if !friends.incoming.isEmpty {
                Section("Wants to be friends") {
                    ForEach(friends.incoming) { f in
                        HStack {
                            person(f)
                            Spacer()
                            Button("Accept") { Task { await friends.accept(f) } }.buttonStyle(.borderedProminent).tint(GameTint.green).foregroundStyle(.black)
                            Button { Task { await friends.remove(f) } } label: { Image(systemName: "xmark") }.buttonStyle(.bordered)
                        }
                        .controlSize(.small)
                        .contextMenu { menu(f) }
                    }
                }
            }

            Section(friends.friends.isEmpty ? "" : "Friends") {
                if friends.friends.isEmpty, friends.loaded {
                    ContentUnavailableView("No friends here yet", systemImage: "person.2", description: Text("Add someone by their couch name."))
                }
                ForEach(friends.friends) { f in
                    VStack(alignment: .leading, spacing: 4) {
                        person(f)
                        if let w = f.watching {
                            Label([w.title, w.episode].compactMap { $0 }.joined(separator: " · ") + (w.paused ? "  (paused)" : ""), systemImage: "play.fill")
                                .font(.caption).foregroundStyle(GameTint.green).lineLimit(1)
                        }
                    }
                    .contextMenu { menu(f) }
                    .swipeActions { Button("Remove", role: .destructive) { Task { await friends.remove(f) } } }
                }
            }

            if !friends.outgoing.isEmpty {
                Section("Asked") {
                    ForEach(friends.outgoing) { f in
                        HStack {
                            person(f)
                            Spacer()
                            Button("Take back") { Task { await friends.remove(f) } }.buttonStyle(.bordered).controlSize(.small)
                        }
                    }
                }
            }

            if friends.admin, !friends.reports.isEmpty {
                Section {
                    ForEach(friends.reports) { r in
                        VStack(alignment: .leading, spacing: 3) {
                            Text(r.line).font(.subheadline.weight(.semibold))
                            if !r.why.isEmpty { Text(r.why).font(.caption).foregroundStyle(Theme.dim) }
                            if let t = r.text, !t.isEmpty { Text("“\(t)”").font(.caption).italic().lineLimit(6) }
                            Text(Date(timeIntervalSince1970: r.at / 1000), format: .dateTime.day().month().hour().minute()).font(.caption2).foregroundStyle(Theme.faint)
                        }
                        .swipeActions {
                            Button("Done") { Task { await friends.clear(r) } }.tint(GameTint.green)
                            if r.ref != nil { Button("Remove review", role: .destructive) { Task { await friends.removeReview(r) } } }
                        }
                    }
                } header: {
                    Text("Reports")
                } footer: {
                    Text("Only you see these. Swipe one away when it's dealt with.")
                }
            }
        }
        .navigationTitle("Friends")
        .task { await friends.refresh(force: true) }
        .refreshable { await friends.refresh(force: true) }
        .sheet(isPresented: $picking) { NameSheet() }
    }

    private func person(_ f: Friend) -> some View {
        HStack(spacing: 10) {
            AsyncImage(url: f.avatar.flatMap(URL.init(string:))) { $0.resizable() } placeholder: {
                Circle().fill(Color(white: 0.2)).overlay(Text(String(f.name.prefix(1)).uppercased()).font(.footnote.weight(.bold)))
            }
            .frame(width: 34, height: 34).clipShape(Circle())
            VStack(alignment: .leading, spacing: 1) {
                Text(f.name).font(.subheadline.weight(.semibold)).lineLimit(1)
                if let h = f.handle { Text("@\(h)").font(.caption).foregroundStyle(Theme.dim) }
            }
        }
    }

    @ViewBuilder private func menu(_ f: Friend) -> some View {
        Button("Remove", systemImage: "person.badge.minus", role: .destructive) { Task { await friends.remove(f) } }
        Button("Block", systemImage: "hand.raised", role: .destructive) { Task { await friends.block(f) } }
        Button("Report and block", systemImage: "exclamationmark.bubble", role: .destructive) {
            Task {
                await friends.report(f, why: "reported from the friends list")
                await friends.block(f)
            }
        }
    }

    private func add() {
        let h = adding.trimmingCharacters(in: .whitespaces)
        guard h.count >= 3, !busy else { return }
        busy = true
        Task {
            if let e = await friends.add(h) { note = e } else { note = "Asked @\(h.lowercased())."; adding = "" }
            busy = false
        }
    }
}

/// Your name and couch name.
struct NameSheet: View {
    @State private var friends = Friends.shared
    @State private var name = ""
    @State private var handle = ""
    @State private var error: String?
    @State private var busy = false
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Form {
                Section("Name") { TextField("what friends see", text: $name) }
                Section {
                    HStack(spacing: 2) {
                        Text("@").foregroundStyle(Theme.dim)
                        TextField("couchname", text: $handle).textInputAutocapitalization(.never).autocorrectionDisabled()
                    }
                } header: {
                    Text("couch name")
                } footer: {
                    Text(error ?? "3 to 20 letters, numbers, _ or . Friends add you with this.").foregroundStyle(error == nil ? Theme.dim : GameTint.red)
                }
            }
            .navigationTitle("Your name")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        busy = true
                        Task {
                            let n = name.trimmingCharacters(in: .whitespaces)
                            error = await friends.save(name: n.isEmpty ? nil : n, handle: handle.trimmingCharacters(in: .whitespaces))
                            busy = false
                            if error == nil { dismiss() }
                        }
                    }
                    .disabled(busy || handle.trimmingCharacters(in: .whitespaces).count < 3)
                }
            }
            .onAppear {
                name = friends.name
                handle = friends.handle ?? ""
            }
        }
        .presentationDetents([.medium])
    }
}
