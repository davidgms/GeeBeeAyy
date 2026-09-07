import SwiftUI

// GeeBeeAyy palette. Mirrors android ui/theme/Color.kt - keep the two in step.
// Night carries structure, honey carries identity. See docs/design/DESIGN-SYSTEM.md.
extension Color {
    // Night: structure
    static let nightVoid = Color(hex: 0x150A2B)
    static let nightPanel = Color(hex: 0x221046)
    static let nightRaised = Color(hex: 0x2E1660)
    static let nightEdge = Color(hex: 0x45268A)

    // Honey: identity
    static let goldenSaplight = Color(hex: 0xFACC15)
    static let amberResin = Color(hex: 0xA16207)
    static let honeyLight = Color(hex: 0xD4A017)
    static let pineGlowMist = Color(hex: 0xFFF9C2)
    static let burntRoot = Color(hex: 0x1A0F00)
    static let beeWing = Color(hex: 0xFDEEB7)

    // Neon: accents from GB's room
    static let neonViolet = Color(hex: 0x7C30BC)
    static let neonMagenta = Color(hex: 0x650EBA)
    static let lensCyan = Color(hex: 0x55F6FD)
    static let wingLavender = Color(hex: 0xB0A6CD)
    static let blushPink = Color(hex: 0xFCA8CE)
    static let ledGreen = Color(hex: 0x7CE04A)

    init(hex: UInt32, alpha: Double = 1.0) {
        let r = Double((hex >> 16) & 0xFF) / 255.0
        let g = Double((hex >> 8) & 0xFF) / 255.0
        let b = Double(hex & 0xFF) / 255.0
        self.init(.sRGB, red: r, green: g, blue: b, opacity: alpha)
    }
}

// MARK: - Design Tokens
struct GeeBeeAyyDesign {
    // Spacing
    static let spacingXS: CGFloat = 4
    static let spacingSM: CGFloat = 8
    static let spacingMD: CGFloat = 16
    static let spacingLG: CGFloat = 24
    static let spacingXL: CGFloat = 32
    static let spacingXXL: CGFloat = 48

    // Corner Radius
    static let radiusSM: CGFloat = 8
    static let radiusMD: CGFloat = 12
    static let radiusLG: CGFloat = 16
    static let radiusXL: CGFloat = 24

    // Font Sizes
    static let fontDisplay: CGFloat = 36
    static let fontTitle: CGFloat = 24
    static let fontHeadline: CGFloat = 20
    static let fontBody: CGFloat = 16
    static let fontCaption: CGFloat = 12
}
