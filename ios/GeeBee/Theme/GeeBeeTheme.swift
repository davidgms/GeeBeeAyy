import SwiftUI

// GeeBee Color Palette — from design/color-pallete.jpg
extension Color {
    // Primary Base
    static let burntRoot = Color(hex: 0x1A0F00)
    // Primary Action
    static let amberResin = Color(hex: 0xA16207)
    // Primary Background
    static let goldenSaplight = Color(hex: 0xFACC15)
    // Secondary Base
    static let pineGlowMist = Color(hex: 0xFFF9C2)

    // Extended
    static let honeyDark = Color(hex: 0x2D1A00)
    static let honeyMid = Color(hex: 0x6B4A00)
    static let honeyLight = Color(hex: 0xD4A017)
    static let beeWing = Color(hex: 0xE8DCC8)
    static let flowerPink = Color(hex: 0xF5A0B5)
    static let leafGreen = Color(hex: 0x7CB342)

    init(hex: UInt32, alpha: Double = 1.0) {
        let r = Double((hex >> 16) & 0xFF) / 255.0
        let g = Double((hex >> 8) & 0xFF) / 255.0
        let b = Double(hex & 0xFF) / 255.0
        self.init(.sRGB, red: r, green: g, blue: b, opacity: alpha)
    }
}

// MARK: - Design Tokens
struct GeeBeeDesign {
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
