// midnite™ couch for iPhone and iPad.

import SwiftUI

@main
struct CouchApp: App {
    init() {
        Store.shared.start()
        // the tab bar and nav bars sit on black glass
        let tab = UITabBarAppearance()
        tab.configureWithDefaultBackground()
        tab.backgroundEffect = UIBlurEffect(style: .systemChromeMaterialDark)
        UITabBar.appearance().standardAppearance = tab
        UITabBar.appearance().scrollEdgeAppearance = tab
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .preferredColorScheme(.dark)
                .tint(.white)
        }
    }
}

enum Tab: Hashable { case home, search, library, settings }

/// Where the app is: the tab and each tab's stack (deep links push here).
@MainActor @Observable
final class Router {
    static let shared = Router()
    var tab: Tab = .home
    var home = NavigationPath()
    var search = NavigationPath()
    var library = NavigationPath()
    var paywall = false

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
        case "tab":
            switch parts.first {
            case "search": tab = .search
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

    var body: some View {
        TabView(selection: $router.tab) {
            NavigationStack(path: $router.home) {
                HomeView().withDestinations()
            }
            .tabItem { Label("Home", systemImage: "house.fill") }
            .tag(Tab.home)

            NavigationStack(path: $router.search) {
                SearchView().withDestinations()
            }
            .tabItem { Label("Search", systemImage: "magnifyingglass") }
            .tag(Tab.search)

            NavigationStack(path: $router.library) {
                LibraryView().withDestinations()
            }
            .tabItem { Label("My couch", systemImage: "sofa.fill") }
            .tag(Tab.library)

            NavigationStack {
                SettingsView()
            }
            .tabItem { Label("Settings", systemImage: "gearshape.fill") }
            .tag(Tab.settings)
        }
        .onOpenURL { router.handle($0) }
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
