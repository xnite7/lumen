// couch Pro: what it adds and how to get it (App Store, or the coucher role
// in the midnite Discord server).

import StoreKit
import SwiftUI

struct ProView: View {
    @State private var store = Store.shared
    @State private var account = Account.shared
    @Environment(\.dismiss) private var dismiss

    private let perks: [(String, String, String)] = [
        ("play.rectangle.fill", "Scene previews", "Clips play behind the billboard and on every title page."),
        ("infinity", "Endless Scenes", "Play the guessing game as long as you can last, and climb the best-runs board."),
        ("square.grid.2x2.fill", "Home-screen widgets", "Continue Watching and My List, right on your Home Screen."),
        ("heart.fill", "Support couch", "Pro keeps the lights on and the updates coming."),
    ]

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 22) {
                    Image("Mark").resizable().scaledToFit().frame(width: 110).foregroundStyle(.white).padding(.top, 10)
                    Text("couch Pro").font(.system(size: 34, weight: .heavy))
                    VStack(alignment: .leading, spacing: 16) {
                        ForEach(perks.indices, id: \.self) { i in
                            let p = perks[i]
                            HStack(alignment: .top, spacing: 14) {
                                Image(systemName: p.0).font(.title3).frame(width: 30)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(p.1).font(.headline)
                                    Text(p.2).font(.subheadline).foregroundStyle(Theme.dim)
                                }
                            }
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(18)
                    .background(Theme.card, in: RoundedRectangle(cornerRadius: 18, style: .continuous))

                    if account.pro {
                        Label("You have Pro", systemImage: "checkmark.seal.fill").font(.headline)
                    } else {
                        VStack(spacing: 10) {
                            ForEach(store.products) { p in
                                Button { Task { await store.buy(p) } } label: {
                                    HStack {
                                        Text(p.displayName)
                                        Spacer()
                                        Text(p.displayPrice + (p.subscription.map { "/" + unit($0.subscriptionPeriod) } ?? ""))
                                    }
                                }
                                .buttonStyle(PillButton(primary: p.id == store.products.last?.id))
                            }
                            if store.products.isEmpty {
                                Text("Pro isn't available to buy right now.").font(.footnote).foregroundStyle(Theme.dim)
                            }
                            if let e = store.lastError { Text(e).font(.footnote).foregroundStyle(.red) }
                            Button("Restore purchases") { Task { await store.restore() } }
                                .font(.footnote)
                                .padding(.top, 4)
                        }
                        .disabled(store.busy)
                        .overlay { if store.busy { ProgressView() } }
                        Text("Couchers in the midnite Discord server get Pro included: sign in with Discord in Settings.")
                            .font(.footnote)
                            .foregroundStyle(Theme.faint)
                            .multilineTextAlignment(.center)
                        Text("Subscriptions renew until cancelled in your App Store account settings.")
                            .font(.caption2)
                            .foregroundStyle(Theme.faint)
                            .multilineTextAlignment(.center)
                        HStack(spacing: 16) {
                            Link("Terms", destination: URL(string: "\(crewURL)/terms")!)
                            Link("Privacy", destination: URL(string: "\(crewURL)/privacy")!)
                        }
                        .font(.caption)
                    }
                }
                .padding(Theme.pad)
                .frame(maxWidth: 520)
                .frame(maxWidth: .infinity)
            }
            .background(Theme.bg)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } } }
        }
    }

    private func unit(_ p: Product.SubscriptionPeriod) -> String {
        switch p.unit {
        case .day: p.value == 7 ? "week" : "day"
        case .week: "week"
        case .month: p.value == 1 ? "month" : "\(p.value) months"
        case .year: "year"
        @unknown default: ""
        }
    }
}
