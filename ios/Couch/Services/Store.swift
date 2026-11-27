// couch Pro in the App Store (StoreKit 2). Create these products in App
// Store Connect (any you skip are simply not offered):
//   com.midnite.couch.pro.monthly   auto-renewing, group "couch Pro"
//   com.midnite.couch.pro.yearly    auto-renewing, group "couch Pro"
//   com.midnite.couch.pro.lifetime  non-consumable

import Foundation
import Observation
import StoreKit

@MainActor @Observable
final class Store {
    static let shared = Store()
    static let ids = ["com.midnite.couch.pro.monthly", "com.midnite.couch.pro.yearly", "com.midnite.couch.pro.lifetime"]

    private(set) var products: [Product] = []
    private(set) var busy = false
    private(set) var lastError: String?
    /// the signed transaction that proves Pro to the couch server
    private var proJWS: String?
    private var updates: Task<Void, Never>?

    private init() {}

    func start() {
        updates = Task {
            for await u in Transaction.updates {
                if case .verified(let t) = u { await t.finish() }
                await refresh()
            }
        }
        Task {
            await refresh()
            products = ((try? await Product.products(for: Self.ids)) ?? []).sorted { $0.price < $1.price }
        }
    }

    /// Re-read what you own.
    func refresh() async {
        var jws: String?
        for await e in Transaction.currentEntitlements {
            if case .verified(let t) = e, Self.ids.contains(t.productID), t.revocationDate == nil {
                if let x = t.expirationDate, x < .now { continue }
                jws = e.jwsRepresentation
            }
        }
        proJWS = jws
        Account.shared.storePro = jws != nil
    }

    func proTransaction() async -> String? { proJWS }

    func buy(_ p: Product) async {
        busy = true
        lastError = nil
        defer { busy = false }
        do {
            switch try await p.purchase() {
            case .success(let v):
                if case .verified(let t) = v { await t.finish() }
                await refresh()
            case .pending:
                lastError = "waiting for approval"
            default:
                break
            }
        } catch {
            lastError = error.localizedDescription
        }
    }

    func restore() async {
        busy = true
        defer { busy = false }
        try? await AppStore.sync()
        await refresh()
    }
}
