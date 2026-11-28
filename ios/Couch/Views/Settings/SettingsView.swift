// Settings: your sources (the servers couch plays from), your account, Pro.

import AuthenticationServices
import SwiftUI

struct SettingsView: View {
    @State private var account = Account.shared
    @State private var sources = Sources.shared
    @State private var router = Router.shared
    @State private var signingIn = false
    @State private var error: String?
    @State private var simkl = Simkl.shared
    @State private var linking = false
    @State private var naming = false
    @State private var deleting = false

    var body: some View {
        List {
            Section {
                NavigationLink {
                    SourcesView()
                } label: {
                    LabeledContent {
                        Text(sources.list.isEmpty ? "None" : "\(sources.active.count)")
                    } label: {
                        Label("Sources", systemImage: "server.rack")
                    }
                }
            } footer: {
                Text("couch plays from servers you add. It doesn't come with any.")
            }

            Section("couch Pro") {
                if account.pro {
                    Label(account.storePro ? "Pro is on" : "Pro is on (coucher)", systemImage: "checkmark.seal.fill")
                } else {
                    Button { router.paywall = true } label: { Label("Get couch Pro", systemImage: "sparkles") }
                }
            }

            Section {
                if let c = account.couch {
                    Button { naming = true } label: {
                        LabeledContent {
                            Image(systemName: "pencil").foregroundStyle(Theme.dim)
                        } label: {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(c.name).font(.headline).foregroundStyle(.white)
                                Text(c.handle.map { "@\($0)" } ?? "pick a couch name so friends can add you").font(.caption).foregroundStyle(Theme.dim)
                            }
                        }
                    }
                    Toggle("Show friends what I'm watching", isOn: Binding(get: { Crew.shared.share }, set: { Crew.shared.share = $0 }))
                    Button("Sign out") { Task { await account.signOutCouch() } }
                    Button("Delete my account", role: .destructive) { deleting = true }
                } else if account.discord == nil {
                    SignInWithAppleButton(.signIn) { req in
                        req.requestedScopes = [.fullName]
                    } onCompletion: { result in
                        switch result {
                        case .success(let auth):
                            guard let cred = auth.credential as? ASAuthorizationAppleIDCredential else { return }
                            Task {
                                do {
                                    try await account.signInApple(cred)
                                    await Friends.shared.refresh(force: true)
                                    if Friends.shared.handle == nil { naming = true }
                                } catch { self.error = error.localizedDescription }
                            }
                        case .failure(let e):
                            if (e as? ASAuthorizationError)?.code != .canceled { self.error = e.localizedDescription }
                        }
                    }
                    .signInWithAppleButtonStyle(.white)
                    .frame(height: 46)
                    .listRowInsets(EdgeInsets())
                } else {
                    Button { naming = true } label: { Label("Your couch name (for friends)", systemImage: "at") }
                }
            } header: {
                Text("couch account")
            } footer: {
                Text("Optional. For friends and the game's leaderboard. couch keeps a name and a couch name, nothing else from Apple.")
            }

            Section {
                if let d = account.discord {
                    HStack(spacing: 12) {
                        AsyncImage(url: d.avatar.flatMap(URL.init(string:))) { $0.resizable() } placeholder: { Color(white: 0.2) }
                            .frame(width: 40, height: 40)
                            .clipShape(Circle())
                        VStack(alignment: .leading, spacing: 2) {
                            Text(d.name).font(.headline)
                            Text(d.coucher ? "coucher" : "not a coucher yet").font(.caption).foregroundStyle(Theme.dim)
                        }
                    }
                    if d.coucher {
                        Toggle("Show the crew what I'm watching", isOn: Binding(get: { Crew.shared.share }, set: { Crew.shared.share = $0 }))
                    }
                    Button("Check my role again") { Task { await account.refresh() } }
                    Button("Sign out", role: .destructive) { account.signOut() }
                } else {
                    Button {
                        signingIn = true
                        Task {
                            do { try await account.signIn() } catch { self.error = error.localizedDescription }
                            signingIn = false
                        }
                    } label: {
                        HStack {
                            Label("Sign in with Discord", systemImage: "person.crop.circle")
                            if signingIn { Spacer(); ProgressView() }
                        }
                    }
                    .disabled(signingIn)
                }
            } header: {
                Text("midnite")
            } footer: {
                Text("Optional. Couchers in the midnite Discord server get Pro included.")
            }

            Section {
                if simkl.connected {
                    LabeledContent {
                        if simkl.syncing { ProgressView() } else { Image(systemName: "checkmark.circle.fill") }
                    } label: {
                        Label(simkl.name.map { "Linked as \($0)" } ?? "Linked", systemImage: "list.star")
                    }
                    Button("Sync now") { Task { await simkl.sync() } }.disabled(simkl.syncing)
                    if simkl.needsReconnect {
                        Label("Simkl needs you to link again", systemImage: "exclamationmark.triangle")
                        Button("Link again") {
                            linking = true
                            Task {
                                do { try await simkl.connect() } catch { self.error = error.localizedDescription }
                                linking = false
                            }
                        }
                        .disabled(linking)
                    } else if simkl.waiting > 0 {
                        Label("\(simkl.waiting) change\(simkl.waiting == 1 ? "" : "s") waiting to reach Simkl", systemImage: "arrow.triangle.2.circlepath")
                    }
                    Button("Unlink Simkl", role: .destructive) { simkl.disconnect() }
                } else {
                    Button {
                        linking = true
                        Task {
                            do { try await simkl.connect() } catch { self.error = error.localizedDescription }
                            linking = false
                        }
                    } label: {
                        HStack {
                            Label("Link Simkl", systemImage: "list.star")
                            if linking { Spacer(); ProgressView() }
                        }
                    }
                    .disabled(linking)
                }
            } header: {
                Text("Simkl")
            } footer: {
                Text("Recommended. Your watchlist and what you've watched stay the same on every couch. Your Discord and Simkl links are kept in iCloud Keychain, so they follow your Apple ID to your other devices.")
            }

            Section {
                Link(destination: URL(string: "\(crewURL)/privacy")!) { Label("Privacy Policy", systemImage: "hand.raised") }
                Link(destination: URL(string: "\(crewURL)/terms")!) { Label("Terms", systemImage: "doc.text") }
                LabeledContent("Version", value: Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "")
            }
        }
        .navigationTitle("Settings")
        .sheet(isPresented: $naming) { NameSheet().task { await Friends.shared.refresh(force: true) } }
        .confirmationDialog("Delete your couch account?", isPresented: $deleting, titleVisibility: .visible) {
            Button("Delete account", role: .destructive) {
                Task { if !(await account.deleteAccount()) { self.error = "couldn't reach the couch server. Nothing was deleted." } }
            }
        } message: {
            Text("Your name, couch name and friends are removed for good. Pro and your library on this device stay.")
        }
        .alert("Sign-in", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(error ?? "")
        }
    }
}

let demoSource = "\(crewURL)/demo"

/// Your sources: add a server by its address, reorder (top = preferred), switch off, remove.
struct SourcesView: View {
    @State private var sources = Sources.shared
    @State private var adding = false
    @State private var link = ""
    @State private var busy = false
    @State private var error: String?

    var body: some View {
        List {
            Section {
                ForEach(sources.list) { s in
                    Toggle(isOn: Binding(get: { s.on }, set: { _ in sources.toggle(s) })) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(s.name).font(.headline)
                            Text(s.host).font(.caption).foregroundStyle(Theme.dim)
                        }
                    }
                }
                .onDelete { idx in idx.map { sources.list[$0] }.forEach(sources.remove) }
                .onMove(perform: sources.move)
            } footer: {
                Text(sources.list.isEmpty
                     ? "Add the address of your own media server or addon. couch asks it for streams when you press Play."
                     : "Play uses the first source that has the title. Drag to reorder.")
            }
            Section {
                Button { adding = true } label: { Label("Add a source", systemImage: "plus") }
                if !sources.list.contains(where: { $0.base == demoSource }) {
                    Button {
                        busy = true
                        Task {
                            do { _ = try await sources.add(demoSource) } catch { self.error = error.localizedDescription }
                            busy = false
                        }
                    } label: {
                        Label("Try the demo source", systemImage: "play.circle")
                    }
                }
            } footer: {
                Text("The demo source has two open movies (Big Buck Bunny and Sintel) so you can see how sources work.")
            }
        }
        .navigationTitle("Sources")
        .toolbar { if !sources.list.isEmpty { EditButton() } }
        .alert("Add a source", isPresented: $adding) {
            TextField("https://…", text: $link)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .keyboardType(.URL)
            Button("Add") {
                let l = link
                link = ""
                busy = true
                Task {
                    do { _ = try await sources.add(l) } catch { self.error = error.localizedDescription }
                    busy = false
                }
            }
            Button("Cancel", role: .cancel) { link = "" }
        } message: {
            Text("Paste the server's address (its manifest link works too).")
        }
        .alert("Couldn't add it", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(error ?? "")
        }
        .overlay { if busy { ProgressView().padding(20).background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 14)) } }
    }
}
