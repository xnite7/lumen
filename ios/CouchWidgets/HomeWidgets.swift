// Home-screen widgets (couch Pro): Continue Watching and My List. The app
// writes a snapshot into the App Group (WidgetSnapshot) and asks WidgetKit
// to reload; these only read it.

import SwiftUI
import WidgetKit

@main
struct CouchWidgetBundle: WidgetBundle {
    var body: some Widget {
        ContinueWidget()
        MyListWidget()
        NowPlayingActivity()
    }
}

struct SnapshotEntry: TimelineEntry {
    let date: Date
    let snap: WidgetSnapshot
}

struct SnapshotProvider: TimelineProvider {
    func placeholder(in context: Context) -> SnapshotEntry {
        SnapshotEntry(date: .now, snap: WidgetSnapshot(pro: true, continueWatching: [
            WidgetItem(id: "tt0110912", type: "movie", title: "Pulp Fiction", subtitle: "1994", progress: 0.4, imageFile: nil),
        ], myList: [], updated: .now))
    }
    func getSnapshot(in context: Context, completion: @escaping (SnapshotEntry) -> Void) {
        completion(context.isPreview ? placeholder(in: context) : SnapshotEntry(date: .now, snap: .load()))
    }
    func getTimeline(in context: Context, completion: @escaping (Timeline<SnapshotEntry>) -> Void) {
        // the app reloads us when something changes; this is just a backstop
        completion(Timeline(entries: [SnapshotEntry(date: .now, snap: .load())], policy: .after(.now.addingTimeInterval(3600))))
    }
}

private func image(_ item: WidgetItem) -> UIImage? {
    guard let f = item.imageFile else { return nil }
    return UIImage(contentsOfFile: Shared.folder.appendingPathComponent(f).path)
}

private func link(_ item: WidgetItem) -> URL {
    URL(string: "couch://title/\(item.type)/\(item.id.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? item.id)")!
}

/// One backdrop card with the title and a progress line.
struct Card: View {
    let item: WidgetItem
    var big = false

    var body: some View {
        ZStack(alignment: .bottomLeading) {
            if let ui = image(item) {
                Image(uiImage: ui).resizable().scaledToFill()
            } else {
                Color(white: 0.12)
            }
            LinearGradient(colors: [.clear, .black.opacity(0.85)], startPoint: .center, endPoint: .bottom)
            VStack(alignment: .leading, spacing: 3) {
                Text(item.title).font(big ? .headline : .caption.weight(.semibold)).lineLimit(1)
                if !item.subtitle.isEmpty {
                    Text(item.subtitle).font(.caption2).foregroundStyle(.white.opacity(0.7)).lineLimit(1)
                }
                if item.progress > 0 {
                    GeometryReader { g in
                        ZStack(alignment: .leading) {
                            Capsule().fill(.white.opacity(0.25))
                            Capsule().fill(.white).frame(width: g.size.width * item.progress)
                        }
                    }
                    .frame(height: 3)
                }
            }
            .padding(big ? 12 : 8)
        }
        .foregroundStyle(.white)
        .clipShape(RoundedRectangle(cornerRadius: big ? 0 : 10, style: .continuous))
    }
}

struct Locked: View {
    var body: some View {
        VStack(spacing: 6) {
            Image(systemName: "sparkles").font(.title3)
            Text("couch Pro").font(.headline)
            Text("unlock widgets in the app").font(.caption2).foregroundStyle(.secondary).multilineTextAlignment(.center)
        }
        .foregroundStyle(.white)
        .widgetURL(URL(string: "couch://pro"))
    }
}

struct Empty: View {
    let text: String
    var body: some View {
        VStack(spacing: 6) {
            Image(systemName: "sofa.fill").font(.title).foregroundStyle(.white)
            Text(text).font(.caption).foregroundStyle(.secondary).multilineTextAlignment(.center)
        }
    }
}

struct ListWidgetView: View {
    let items: [WidgetItem]
    let pro: Bool
    let emptyText: String
    @Environment(\.widgetFamily) var family

    var body: some View {
        Group {
            if !pro {
                Locked()
            } else if items.isEmpty {
                Empty(text: emptyText)
            } else {
                switch family {
                case .systemSmall:
                    Card(item: items[0], big: true).widgetURL(link(items[0]))
                case .systemMedium:
                    HStack(spacing: 8) {
                        ForEach(items.prefix(2)) { i in Link(destination: link(i)) { Card(item: i) } }
                    }
                    .padding(10)
                default:
                    VStack(spacing: 8) {
                        ForEach(items.prefix(3)) { i in Link(destination: link(i)) { Card(item: i) } }
                    }
                    .padding(10)
                }
            }
        }
        .containerBackground(for: .widget) { Color.black }
    }
}

struct ContinueWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "continue", provider: SnapshotProvider()) { e in
            ListWidgetView(items: e.snap.continueWatching, pro: e.snap.pro, emptyText: "start something and it shows up here")
        }
        .configurationDisplayName("Continue Watching")
        .description("Pick up where you left off.")
        .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
        .contentMarginsDisabled()
    }
}

struct MyListWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "mylist", provider: SnapshotProvider()) { e in
            ListWidgetView(items: e.snap.myList, pro: e.snap.pro, emptyText: "add titles to My List")
        }
        .configurationDisplayName("My List")
        .description("What you saved for later.")
        .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
        .contentMarginsDisabled()
    }
}
