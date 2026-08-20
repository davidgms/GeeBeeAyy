import SwiftUI

struct ROM: Identifiable {
    let id = UUID()
    let name: String
    let fileName: String
    let size: String
    let lastPlayed: String?
    let isFavorite: Bool
}

struct ROMBrowserView: View {
    @State private var showingSettings = false
    @State private var showingFilePicker = false

    let sampleROMs = [
        ROM(name: "Pokemon Emerald", fileName: "pokemon_emerald.gba", size: "16 MB", lastPlayed: "2 hours ago", isFavorite: true),
        ROM(name: "Zelda: Minish Cap", fileName: "zelda_minish.gba", size: "16 MB", lastPlayed: "Yesterday", isFavorite: true),
        ROM(name: "Mario Kart", fileName: "mario_kart.gba", size: "8 MB", lastPlayed: nil, isFavorite: false),
        ROM(name: "Metroid Fusion", fileName: "metroid_fusion.gba", size: "16 MB", lastPlayed: nil, isFavorite: false),
        ROM(name: "Fire Emblem", fileName: "fire_emblem.gba", size: "16 MB", lastPlayed: "Last week", isFavorite: false),
    ]

    var body: some View {
        NavigationView {
            ZStack {
                Color.burntRoot
                    .ignoresSafeArea()

                ScrollView {
                    VStack(alignment: .leading, spacing: GeeBeeAyyDesign.spacingMD) {
                        // Header
                        HStack {
                            Text("Your Games")
                                .font(.system(size: GeeBeeAyyDesign.fontTitle, weight: .bold))
                                .foregroundColor(.goldenSaplight)
                            Spacer()
                        }
                        .padding(.horizontal, GeeBeeAyyDesign.spacingMD)

                        // Favorites
                        if sampleROMs.contains(where: { $0.isFavorite }) {
                            SectionHeader(title: "Favorites")
                            ForEach(sampleROMs.filter(\.isFavorite)) { rom in
                                ROMCard(rom: rom)
                            }
                        }

                        // All ROMs
                        SectionHeader(title: "All ROMs")
                        ForEach(sampleROMs) { rom in
                            ROMCard(rom: rom)
                        }
                    }
                    .padding(.vertical, GeeBeeAyyDesign.spacingMD)
                }
            }
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button(action: {}) {
                        HStack {
                            Text("GeeBeeAyy!")
                                .font(.system(size: GeeBeeAyyDesign.fontHeadline, weight: .bold))
                                .foregroundColor(.goldenSaplight)
                            Text("ROMs")
                                .font(.system(size: GeeBeeAyyDesign.fontHeadline, weight: .light))
                                .foregroundColor(.pineGlowMist)
                        }
                    }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    HStack {
                        Button(action: { showingSettings = true }) {
                            Image(systemName: "gear")
                                .foregroundColor(.amberResin)
                        }
                        Button(action: { showingFilePicker = true }) {
                            Image(systemName: "plus")
                                .foregroundColor(.amberResin)
                        }
                    }
                }
            }
            .sheet(isPresented: $showingSettings) {
                SettingsView()
            }
        }
    }
}

struct SectionHeader: View {
    let title: String

    var body: some View {
        Text(title)
            .font(.system(size: GeeBeeAyyDesign.fontBody, weight: .semibold))
            .foregroundColor(.amberResin)
            .padding(.horizontal, GeeBeeAyyDesign.spacingMD)
            .padding(.top, GeeBeeAyyDesign.spacingSM)
    }
}

struct ROMCard: View {
    let rom: ROM

    var body: some View {
        Button(action: {}) {
            HStack(spacing: GeeBeeAyyDesign.spacingMD) {
                // Game icon placeholder
                ZStack {
                    RoundedRectangle(cornerRadius: GeeBeeAyyDesign.radiusSM)
                        .fill(Color.amberResin.opacity(0.3))
                        .frame(width: 56, height: 56)

                    Image(systemName: "gamecontroller")
                        .foregroundColor(.goldenSaplight)
                        .font(.title2)
                }

                VStack(alignment: .leading, spacing: GeeBeeAyyDesign.spacingXS) {
                    Text(rom.name)
                        .font(.system(size: GeeBeeAyyDesign.fontBody, weight: .semibold))
                        .foregroundColor(.pineGlowMist)
                        .lineLimit(1)

                    Text(rom.fileName)
                        .font(.system(size: GeeBeeAyyDesign.fontCaption))
                        .foregroundColor(.amberResin)
                        .lineLimit(1)

                    if let lastPlayed = rom.lastPlayed {
                        Text("Last played: \(lastPlayed)")
                            .font(.system(size: 11))
                            .foregroundColor(.pineGlowMist.opacity(0.6))
                    }
                }

                Spacer()

                if rom.isFavorite {
                    Image(systemName: "heart.fill")
                        .foregroundColor(.goldenSaplight)
                        .font(.caption)
                }

                Image(systemName: "play.fill")
                    .foregroundColor(.amberResin)
                    .font(.title3)
            }
            .padding(GeeBeeAyyDesign.spacingMD)
        }
        .buttonStyle(PlainButtonStyle())
        .background(Color.honeyDark)
        .cornerRadius(GeeBeeAyyDesign.radiusMD)
        .padding(.horizontal, GeeBeeAyyDesign.spacingMD)
    }
}
