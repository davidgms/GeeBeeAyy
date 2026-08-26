package com.geebeeayy.app.ui.theme

import androidx.compose.ui.graphics.Color

// GeeBeeAyy Color Palette — from design/color-pallete.jpg
// Primary Base
val BurntRoot = Color(0xFF1A0F00)
// Primary Action
val AmberResin = Color(0xFFA16207)
// Primary Background
val GoldenSaplight = Color(0xFFFACC15)
// Secondary Base
val PineGlowMist = Color(0xFFFFF9C2)

// Extended palette
val HoneyDark = Color(0xFF2D1A00)
val HoneyMid = Color(0xFF6B4A00)
val HoneyLight = Color(0xFFD4A017)
val BeeWing = Color(0xFFE8DCC8)
val FlowerPink = Color(0xFFF5A0B5)
val LeafGreen = Color(0xFF7CB342)
val SkyBlue = Color(0xFF90CAF9)

// Semantic colors
val Primary = AmberResin
val OnPrimary = BurntRoot
val PrimaryVariant = HoneyDark
val Secondary = GoldenSaplight
val OnSecondary = BurntRoot
val SecondaryVariant = PineGlowMist
val Background = BurntRoot
val OnBackground = PineGlowMist
val Surface = HoneyDark
val OnSurface = PineGlowMist
val Error = Color(0xFFCF6679)
val OnError = BurntRoot

// Dark theme specific
object GeeBeeAyyDarkColors {
    val primary = AmberResin
    val onPrimary = BurntRoot
    val primaryVariant = HoneyDark
    val secondary = GoldenSaplight
    val onSecondary = BurntRoot
    val secondaryVariant = PineGlowMist
    val background = BurntRoot
    val onBackground = PineGlowMist
    val surface = HoneyDark
    val onSurface = PineGlowMist
    val error = Error
    val onError = BurntRoot
    val isLight = false
}
