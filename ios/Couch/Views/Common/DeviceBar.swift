// "Playing on <device>": a glass bar above the tab bar when another of your
// devices is playing. Pause/resume it from here, or move it to this device.

import SwiftUI

struct DeviceBar: View {
    let device: OtherDevice
    @State private var devices = Devices.shared

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: device.symbol).font(.system(size: 18, weight: .medium)).frame(width: 28)
            VStack(alignment: .leading, spacing: 2) {
                Text(device.title).font(.subheadline.weight(.semibold)).lineLimit(1)
                Text("Playing on \(device.name)" + (device.episode.isEmpty ? "" : " · \(device.episode)"))
                    .font(.caption).foregroundStyle(Theme.dim).lineLimit(1)
            }
            Spacer(minLength: 4)
            Button { devices.togglePause(device) } label: {
                Image(systemName: device.paused ? "play.fill" : "pause.fill")
                    .font(.system(size: 17, weight: .semibold))
                    .frame(width: 38, height: 38)
                    .contentTransition(.symbolEffect(.replace))
            }
            .accessibilityLabel(device.paused ? "Play" : "Pause")
            Button { devices.playHere(device) } label: {
                Text("Play Here").font(.footnote.weight(.semibold)).padding(.horizontal, 12).frame(height: 32)
                    .background(.white, in: Capsule()).foregroundStyle(.black)
            }
        }
        .foregroundStyle(.white)
        .padding(.leading, 14)
        .padding(.trailing, 10)
        .frame(height: 56)
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(alignment: .bottom) {
            if device.duration > 0 {
                ProgressBar(value: device.pos / device.duration).frame(height: 2).padding(.horizontal, 18).opacity(0.8)
            }
        }
        .padding(.horizontal, 14)
        .shadow(color: .black.opacity(0.4), radius: 14, y: 6)
    }
}
