import SwiftUI

struct EmulationView: View {
    @Environment(\.dismiss) var dismiss
    @State private var isPaused = false
    @State private var isFastForward = false
    @State private var showingMenu = false

    let romName: String

    var body: some View {
        ZStack {
            Color.burntRoot
                .ignoresSafeArea()

            VStack(spacing: 0) {
                // Top bar
                HStack {
                    Button(action: { dismiss() }) {
                        Image(systemName: "chevron.left")
                            .foregroundColor(.pineGlowMist)
                    }

                    Spacer()

                    Text("GeeBeeAyy!")
                        .font(.system(size: GeeBeeAyyDesign.fontBody, weight: .bold))
                        .foregroundColor(.goldenSaplight)

                    Spacer()

                    Button(action: { showingMenu = true }) {
                        Image(systemName: "ellipsis")
                            .foregroundColor(.pineGlowMist)
                    }
                }
                .padding(.horizontal, GeeBeeAyyDesign.spacingMD)
                .padding(.vertical, GeeBeeAyyDesign.spacingSM)

                // GBA Screen
                ZStack {
                    RoundedRectangle(cornerRadius: GeeBeeAyyDesign.radiusSM)
                        .fill(Color.black)
                        .aspectRatio(240.0/160.0, contentMode: .fit)
                        .padding(.horizontal, GeeBeeAyyDesign.spacingLG)

                    // Frame buffer placeholder
                    Text("Loading...")
                        .font(.caption)
                        .foregroundColor(.amberResin)

                    // Pause overlay
                    if isPaused {
                        RoundedRectangle(cornerRadius: GeeBeeAyyDesign.radiusSM)
                            .fill(Color.black.opacity(0.7))
                            .padding(.horizontal, GeeBeeAyyDesign.spacingLG)

                        VStack {
                            Image(systemName: "pause.fill")
                                .font(.largeTitle)
                                .foregroundColor(.goldenSaplight)
                            Text("PAUSED")
                                .font(.system(size: GeeBeeAyyDesign.fontHeadline, weight: .bold))
                                .foregroundColor(.goldenSaplight)
                        }
                    }
                }

                Spacer()

                // Controls
                GameControlsView(
                    isPaused: isPaused,
                    isFastForward: isFastForward,
                    onTogglePause: { isPaused.toggle() },
                    onToggleFastForward: { isFastForward.toggle() }
                )
            }
        }
        .actionSheet(isPresented: $showingMenu) {
            ActionSheet(
                title: Text("Menu"),
                buttons: [
                    .default(Text("Save State")) { },
                    .default(Text("Load State")) { },
                    .default(Text("Settings")) { },
                    .cancel()
                ]
            )
        }
    }
}

struct GameControlsView: View {
    let isPaused: Bool
    let isFastForward: Bool
    let onTogglePause: () -> Void
    let onToggleFastForward: () -> Void

    var body: some View {
        HStack(spacing: GeeBeeAyyDesign.spacingXL) {
            // D-Pad
            VStack(spacing: 0) {
                DPadButton(direction: .up)
                HStack(spacing: 0) {
                    DPadButton(direction: .left)
                    Color.clear.frame(width: 48, height: 48)
                    DPadButton(direction: .right)
                }
                DPadButton(direction: .down)
            }

            // Action buttons
            ZStack {
                ActionButton(label: "B", alignment: .leading)
                ActionButton(label: "A", alignment: .trailing)
            }
            .frame(width: 120)

            // Control buttons
            VStack(spacing: GeeBeeAyyDesign.spacingSM) {
                ControlButton(
                    icon: isPaused ? "play.fill" : "pause.fill",
                    color: isPaused ? .goldenSaplight : .amberResin,
                    action: onTogglePause
                )
                ControlButton(
                    icon: "forward.fill",
                    color: isFastForward ? .goldenSaplight : .honeyMid,
                    action: onToggleFastForward
                )
            }
        }
        .padding(GeeBeeAyyDesign.spacingLG)
    }
}

enum DPadDirection {
    case up, down, left, right
}

struct DPadButton: View {
    let direction: DPadDirection

    var body: some View {
        Button(action: {}) {
            Image(systemName: icon)
                .foregroundColor(.pineGlowMist)
                .frame(width: 48, height: 48)
                .background(Color.honeyDark)
                .clipShape(Circle())
        }
    }

    var icon: String {
        switch direction {
        case .up: return "chevron.up"
        case .down: return "chevron.down"
        case .left: return "chevron.left"
        case .right: return "chevron.right"
        }
    }
}

struct ActionButton: View {
    let label: String
    let alignment: HorizontalAlignment

    var body: some View {
        Button(action: {}) {
            Text(label)
                .font(.system(size: 20, weight: .bold))
                .foregroundColor(.pineGlowMist)
                .frame(width: 56, height: 56)
                .background(Color.honeyDark)
                .clipShape(Circle())
        }
    }
}

struct ControlButton: View {
    let icon: String
    let color: Color
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: icon)
                .foregroundColor(.burntRoot)
                .frame(width: 48, height: 48)
                .background(color)
                .clipShape(Circle())
        }
    }
}
