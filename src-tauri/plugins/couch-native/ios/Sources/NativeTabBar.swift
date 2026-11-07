import Tauri
import UIKit
import WebKit

class TabItemArgs: Decodable {
    let id: String
    let title: String
    let symbol: String
    /// a picture URL instead of a symbol (your Discord avatar)
    let avatar: String?
    /// the system Search tab (its own round button on iOS 26)
    let search: Bool?
}

class TabBarArgs: Decodable {
    let tabs: [TabItemArgs]?
    let selected: String?
    let visible: Bool?
}

/// A transparent page behind each tab: the real content is the web view
/// underneath, so every tab is just a see-through placeholder.
private final class Blank: UIViewController {
    override func loadView() {
        view = UIView()
        view.backgroundColor = .clear
        view.isUserInteractionEnabled = false
    }
}

/// couch is always dark: light status bar, dark glass.
private final class BarController: UITabBarController {
    override var preferredStatusBarStyle: UIStatusBarStyle { .lightContent }
}

/// A window that only keeps touches landing on the tab bar; everything else
/// falls through to the app's own window (the web view) below.
private final class PassWindow: UIWindow {
    weak var bar: UIView?
    override func hitTest(_ p: CGPoint, with e: UIEvent?) -> UIView? {
        guard let v = super.hitTest(p, with: e), let bar = bar, !bar.isHidden, bar.alpha > 0.01 else { return nil }
        let f = bar.convert(bar.bounds, to: self).insetBy(dx: 0, dy: -6)
        return f.contains(p) ? v : nil
    }
}

/// The system tab bar (Liquid Glass on iOS 26) floating over couch's page.
/// It lives in its own window above the web view and reports taps back.
@available(iOS 18.0, *)
final class NativeTabBar: NSObject, UITabBarControllerDelegate {
    private var window: PassWindow?
    private let tbc: UITabBarController = BarController()
    private var tabsById: [String: UITab] = [:]
    private var order: [String] = []
    private var wantVisible = true
    private var poll: Timer?
    private var quiet = false
    private var avatars: [String: UIImage] = [:]
    var onTap: ((String) -> Void)?

    func install(over host: UIWindow) {
        guard window == nil, let scene = host.windowScene else { return }
        let w = PassWindow(windowScene: scene)
        w.backgroundColor = .clear
        w.isOpaque = false
        w.windowLevel = host.windowLevel + 1
        tbc.delegate = self
        tbc.view.backgroundColor = .clear
        tbc.overrideUserInterfaceStyle = .dark
        tbc.tabBar.tintColor = UIColor(red: 1.0, green: 0.698, blue: 0.361, alpha: 1)
        w.rootViewController = tbc
        w.bar = tbc.tabBar
        w.isHidden = false
        window = w
        // a sheet or the player presented from the app's window sits under
        // this one, so step aside whenever something is presented there
        poll = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { [weak self, weak host] _ in
            guard let self = self else { return }
            self.apply(animated: true, covered: host?.rootViewController?.presentedViewController != nil)
        }
    }

    func update(_ a: TabBarArgs) {
        if let items = a.tabs {
            let ids = items.map { $0.id }
            if ids != order {
                order = ids
                tabsById = [:]
                let tabs: [UITab] = items.map { it in
                    let t: UITab
                    if it.search == true {
                        t = UISearchTab { _ in Blank() }
                    } else {
                        t = UITab(title: it.title, image: UIImage(systemName: it.symbol), identifier: it.id) { _ in Blank() }
                    }
                    tabsById[it.id] = t
                    return t
                }
                quiet = true
                tbc.tabs = tabs
                quiet = false
            }
            for it in items {
                guard let t = tabsById[it.id], it.search != true else { continue }
                t.title = it.title
                if let url = it.avatar, !url.isEmpty {
                    if let img = avatars[url] {
                        t.image = img
                    } else {
                        t.image = UIImage(systemName: it.symbol)
                        loadAvatar(url) { [weak t] img in t?.image = img }
                    }
                } else {
                    t.image = UIImage(systemName: it.symbol)
                }
            }
        }
        if let s = a.selected, let t = tabsById[s], tbc.selectedTab !== t {
            quiet = true
            tbc.selectedTab = t
            quiet = false
        }
        if let v = a.visible { wantVisible = v }
        apply(animated: true, covered: false)
    }

    private var shown = true
    private func apply(animated: Bool, covered: Bool) {
        let on = wantVisible && !covered
        guard on != shown else { return }
        shown = on
        let bar = tbc.tabBar
        // a hidden window takes no part in rotation or the status bar
        if on {
            window?.isHidden = false
            bar.isHidden = false
        }
        UIView.animate(withDuration: animated ? 0.28 : 0, delay: 0, options: [.beginFromCurrentState, .curveEaseInOut]) {
            bar.alpha = on ? 1 : 0
            bar.transform = on ? .identity : CGAffineTransform(translationX: 0, y: 24)
        } completion: { _ in
            if !self.shown {
                bar.isHidden = true
                self.window?.isHidden = true
            }
        }
    }

    private func loadAvatar(_ url: String, done: @escaping (UIImage) -> Void) {
        guard let u = URL(string: url) else { return }
        URLSession.shared.dataTask(with: u) { [weak self] d, _, _ in
            guard let d = d, let src = UIImage(data: d) else { return }
            let side: CGFloat = 28
            let img = UIGraphicsImageRenderer(size: CGSize(width: side, height: side)).image { _ in
                UIBezierPath(ovalIn: CGRect(x: 0, y: 0, width: side, height: side)).addClip()
                src.draw(in: CGRect(x: 0, y: 0, width: side, height: side))
            }.withRenderingMode(.alwaysOriginal)
            DispatchQueue.main.async {
                self?.avatars[url] = img
                done(img)
            }
        }.resume()
    }

    // taps, including tapping the tab you're already on (scroll to top / back to root)
    func tabBarController(_ tabBarController: UITabBarController, shouldSelectTab tab: UITab) -> Bool {
        if !quiet, let id = tabsById.first(where: { $0.value === tab })?.key {
            UISelectionFeedbackGenerator().selectionChanged()
            onTap?(id)
        }
        return true
    }
}
