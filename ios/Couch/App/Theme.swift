// couch's look on iPhone and iPad: black, big artwork, white type, glass.
// Apple TV's calm with Netflix's rows. No neon.

import SwiftUI

enum Theme {
    static let bg = Color.black
    static let card = Color(white: 0.11)
    static let line = Color.white.opacity(0.1)
    static let dim = Color.white.opacity(0.6)
    static let faint = Color.white.opacity(0.38)
    static let posterRadius: CGFloat = 8
    static let pad: CGFloat = 20
}

extension Font {
    static let billboardTitle = Font.system(size: 34, weight: .heavy, design: .default)
    static let rowTitle = Font.system(size: 20, weight: .bold)
    static let meta = Font.system(size: 13, weight: .medium)
}

/// Primary (white) and secondary (glass) buttons, Apple TV style.
struct PillButton: ButtonStyle {
    var primary = true
    func makeBody(configuration c: Configuration) -> some View {
        c.label
            .font(.system(size: 16, weight: .semibold))
            .padding(.horizontal, 22)
            .frame(height: 46)
            .frame(maxWidth: .infinity)
            .foregroundStyle(primary ? Color.black : Color.white)
            .background {
                if primary {
                    RoundedRectangle(cornerRadius: 12, style: .continuous).fill(.white)
                } else {
                    RoundedRectangle(cornerRadius: 12, style: .continuous).fill(.ultraThinMaterial)
                }
            }
            .opacity(c.isPressed ? 0.75 : 1)
            .scaleEffect(c.isPressed ? 0.98 : 1)
            .animation(.easeOut(duration: 0.15), value: c.isPressed)
    }
}

/// Little round icon button on glass (My List, share, close).
struct RoundIcon: View {
    let symbol: String
    var size: CGFloat = 46
    var body: some View {
        Image(systemName: symbol)
            .font(.system(size: size * 0.38, weight: .semibold))
            .frame(width: size, height: size)
            .background(.ultraThinMaterial, in: Circle())
            .foregroundStyle(.white)
    }
}

/// Cards shrink a touch while pressed.
struct PressScale: ButtonStyle {
    func makeBody(configuration c: Configuration) -> some View {
        c.label
            .scaleEffect(c.isPressed ? 0.96 : 1)
            .animation(.spring(response: 0.25, dampingFraction: 0.7), value: c.isPressed)
    }
}

/// Artwork that fades in, with a quiet placeholder while it loads.
struct Art: View {
    let url: URL?
    var mode: ContentMode = .fill

    var body: some View {
        // the image is an overlay so a filled picture never widens its parent
        Theme.card.overlay {
            AsyncImage(url: url, transaction: .init(animation: .easeOut(duration: 0.25))) { phase in
                if case .success(let img) = phase {
                    img.resizable().aspectRatio(contentMode: mode)
                }
            }
        }
        .clipped()
    }
}

/// A title's logo (falls back to the name in bold type).
struct TitleLogo: View {
    let meta: MetaPreview
    var maxHeight: CGFloat = 90
    var maxWidth: CGFloat = 260

    var body: some View {
        AsyncImage(url: meta.logoURL) { phase in
            if case .success(let img) = phase {
                img.resizable().scaledToFit().frame(maxWidth: maxWidth, maxHeight: maxHeight)
                    .shadow(color: .black.opacity(0.5), radius: 12)
            } else if case .failure = phase {
                Text(meta.name).font(.billboardTitle).multilineTextAlignment(.center).lineLimit(3)
                    .frame(maxWidth: maxWidth).shadow(color: .black.opacity(0.6), radius: 10)
            } else {
                Color.clear.frame(width: maxWidth * 0.6, height: maxHeight * 0.6)
            }
        }
    }
}

extension View {
    /// Fades the bottom part of an image into the black page.
    func bottomFade(_ part: CGFloat = 0.45) -> some View {
        overlay {
            LinearGradient(stops: [
                .init(color: .clear, location: 1 - part),
                .init(color: Theme.bg.opacity(0.75), location: 1 - part / 3),
                .init(color: Theme.bg, location: 1),
            ], startPoint: .top, endPoint: .bottom)
        }
    }
}
