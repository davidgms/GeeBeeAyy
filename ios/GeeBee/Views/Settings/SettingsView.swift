import SwiftUI

struct SettingsView: View {
    @Environment(\.dismiss) var dismiss

    var body: some View {
        NavigationView {
            ZStack {
                Color.burntRoot
                    .ignoresSafeArea()

                Form {
                    // Display
                    Section(header: SectionHeader(title: "Display")) {
                        SettingsRow(icon: "star.fill", title: "Screen Scale", value: "2x (Native)")
                        SettingsRow(icon: "paintbrush", title: "Screen Filter", value: "Pixel Perfect")
                        ToggleRow(icon: "rectangle.portrait", title: "Force Portrait", subtitle: "Lock orientation", isOn: true)
                    }
                    .listRowBackground(Color.honeyDark)

                    // Audio
                    Section(header: SectionHeader(title: "Audio")) {
                        ToggleRow(icon: "speaker.wave.2.fill", title: "Sound", subtitle: "Enable audio output", isOn: true)
                        SettingsRow(icon: "music.note", title: "Audio Backend", value: "AVAudioEngine")
                    }
                    .listRowBackground(Color.honeyDark)

                    // Controls
                    Section(header: SectionHeader(title: "Controls")) {
                        SettingsRow(icon: "gamecontroller", title: "Layout", value: "Default")
                        ToggleRow(icon: "antenna.radiowaves.left.and.right", title: "Bluetooth Controller", subtitle: "MFi support", isOn: false)
                    }
                    .listRowBackground(Color.honeyDark)

                    // Save States
                    Section(header: SectionHeader(title: "Save States")) {
                        SettingsRow(icon: "square.and.arrow.down", title: "Save State", value: "Slot 1")
                        SettingsRow(icon: "square.and.arrow.up", title: "Load State", value: "Slot 1")
                        SettingsRow(icon: "trash", title: "Manage Saves", value: "10 slots")
                    }
                    .listRowBackground(Color.honeyDark)

                    // Advanced
                    Section(header: SectionHeader(title: "Advanced")) {
                        ToggleRow(icon: "forward.fill", title: "Fast Forward", subtitle: "Hold button for 2x", isOn: false)
                        ToggleRow(icon: "chart.bar", title: "Show FPS", subtitle: "Frame rate overlay", isOn: false)
                    }
                    .listRowBackground(Color.honeyDark)

                    // About
                    Section(header: SectionHeader(title: "About")) {
                        SettingsRow(icon: "info.circle", title: "GeeBee-A!", value: "v0.1.0")
                        SettingsRow(icon: "chevron.left.forwardslash.chevron.right", title: "Credits", value: "Open Source")
                    }
                    .listRowBackground(Color.honeyDark)
                }
                .scrollContentBackground(.hidden)
            }
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Done") { dismiss() }
                        .foregroundColor(.amberResin)
                }
            }
        }
    }
}

struct SectionHeader: View {
    let title: String

    var body: some View {
        Text(title)
            .font(.system(size: GeeBeeDesign.fontCaption, weight: .semibold))
            .foregroundColor(.amberResin)
    }
}

struct SettingsRow: View {
    let icon: String
    let title: String
    let value: String

    var body: some View {
        HStack {
            Image(systemName: icon)
                .foregroundColor(.amberResin)
                .frame(width: 24)

            Text(title)
                .foregroundColor(.pineGlowMist)

            Spacer()

            Text(value)
                .foregroundColor(.pineGlowMist.opacity(0.6))
                .font(.subheadline)
        }
    }
}

struct ToggleRow: View {
    let icon: String
    let title: String
    let subtitle: String
    @State var isOn: Bool

    var body: some View {
        HStack {
            Image(systemName: icon)
                .foregroundColor(.amberResin)
                .frame(width: 24)

            VStack(alignment: .leading) {
                Text(title)
                    .foregroundColor(.pineGlowMist)
                Text(subtitle)
                    .font(.caption)
                    .foregroundColor(.pineGlowMist.opacity(0.6))
            }

            Spacer()

            Toggle("", isOn: $isOn)
                .tint(.amberResin)
        }
    }
}
