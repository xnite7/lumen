// midnite™ couch for iPhone and iPad.

import SwiftUI

@main
struct CouchApp: App {
    init() {
        Store.shared.start()
        Devices.shared.start()
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .preferredColorScheme(.dark)
                .tint(.white)
        }
    }
}

enum AppTab: Hashable { case home, search, play, library, settings }

/// Where the app is: the tab and each tab's stack (deep links push here).
@MainActor @Observable
final class Router {
    static let shared = Router()
    var tab: AppTab = .home
    var home = NavigationPath()
    var search = NavigationPath()
    var library = NavigationPath()
    var paywall = false
    /// the scenes game is switched on (the couch server says; remembered)
    var scenes = UserDefaults.standard.object(forKey: "config.scenes") as? Bool ?? false

    /// couch://play/daily: open Scenes (and start today's five)
    var wantsPlay = false
    var autoStart: String?

    func loadConfig() async {
        struct C: Decodable { let scenes: Bool }
        guard let c = try? await Net.json(URL(string: crewURL + "/config")!, as: C.self, fresh: true) else { return }
        scenes = c.scenes
        UserDefaults.standard.set(c.scenes, forKey: "config.scenes")
        if c.scenes, wantsPlay { tab = .play }
    }

    func open(_ m: MetaPreview) {
        switch tab {
        case .search: search.append(m)
        case .library: library.append(m)
        default:
            tab = .home
            home.append(m)
        }
    }

    /// couch://title/<type>/<id>, couch://pro
    func handle(_ url: URL) {
        guard url.scheme == "couch" else { return }
        let parts = url.pathComponents.filter { $0 != "/" }
        switch url.host {
        case "title":
            if parts.count >= 2, let k = Kind(rawValue: parts[0]) {
                let id = parts[1].removingPercentEncoding ?? parts[1]
                open(MetaPreview(id: id.components(separatedBy: ":")[0], type: k, name: ""))
            }
        case "pro":
            paywall = true
        case "play":
            wantsPlay = true
            autoStart = parts.first ?? "daily"
            if scenes { tab = .play }
        case "tab":
            switch parts.first {
            case "search": tab = .search
            case "play": wantsPlay = true; if scenes { tab = .play }
            case "library": tab = .library
            case "settings": tab = .settings
            default: tab = .home
            }
        default:
            break
        }
    }
}

struct RootView: View {
    @State private var router = Router.shared
    @State private var playback = Playback.shared
    @State private var devices = Devices.shared

    var body: some View {
        TabView(selection: $router.tab) {
            Tab("Home", systemImage: "house.fill", value: AppTab.home) {
                NavigationStack(path: $router.home) { HomeView().withDestinations() }
            }
            if router.scenes {
                Tab("Games", systemImage: "gamecontroller.fill", value: AppTab.play) {
                    NavigationStack { GameHomeView() }
                }
            }
            Tab("My couch", systemImage: "sofa.fill", value: AppTab.library) {
                NavigationStack(path: $router.library) { LibraryView().withDestinations() }
            }
            Tab("Settings", systemImage: "gearshape.fill", value: AppTab.settings) {
                NavigationStack { SettingsView() }
            }
            // the system's search tab (its own spot in the tab bar)
            Tab(value: AppTab.search, role: .search) {
                NavigationStack(path: $router.search) { SearchView().withDestinations() }
            }
        }
        .overlay(alignment: .bottom) {
            // above the tab bar: what your other device is playing
            if let d = devices.playingElsewhere {
                DeviceBar(device: d)
                    .padding(.bottom, 64)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .animation(.snappy, value: devices.playingElsewhere?.id)
        .onOpenURL { router.handle($0) }
        .task { await router.loadConfig() }
        .task {
            await Account.shared.restore()
            await Simkl.shared.sync()
        }
        .onReceive(NotificationCenter.default.publisher(for: UIApplication.didBecomeActiveNotification)) { _ in
            // back from another app (or another device): catch up
            Task { await Simkl.shared.sync() }
        }
        .task {
            // `-demo` at launch (CI screenshots): with the demo source added
            if ProcessInfo.processInfo.arguments.contains("-demo") { _ = try? await Sources.shared.add(demoSource) }
            await Sources.shared.refresh()
        }
        .task {
            // `-open couch://…` at launch (used by the CI screenshots)
            let a = ProcessInfo.processInfo.arguments
            if let i = a.firstIndex(of: "-open"), i + 1 < a.count, let u = URL(string: a[i + 1]) { router.handle(u) }
        }
        .sheet(isPresented: $router.paywall) { ProView() }
        .alert("couch", isPresented: Binding(get: { playback.problem != nil }, set: { if !$0 { playback.problem = nil } })) {
            if !Sources.shared.any {
                Button("Add a source") {
                    router.tab = .settings
                }
            }
            Button("OK", role: .cancel) {}
        } message: {
            Text(playback.problem ?? "")
        }
        .overlay {
            if playback.finding != nil {
                ProgressView("finding a stream…")
                    .padding(22)
                    .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
                    .transition(.opacity)
            }
        }
        .animation(.easeOut(duration: 0.2), value: playback.finding)
    }
}

extension View {
    func withDestinations() -> some View {
        navigationDestination(for: MetaPreview.self) { DetailView(preview: $0) }
    }
}
