package com.geebeeayy.app.ui.theme

import androidx.compose.ui.graphics.Color

// GeeBeeAyy palette. Two families, and the split between them is the system:
//
//   Night  - structure. Backgrounds, panels, dividers. Sampled from GB's room
//            in docs/design/gb-keyart.png, and close to the GBA's own violet.
//   Honey  - identity. Anything the eye should land on: actions, key text,
//            GB himself. Sampled from GB's body in the same reference.
//
// Night never carries meaning, honey never carries structure. See
// docs/design/DESIGN-SYSTEM.md.

// --- Night: structure -------------------------------------------------------
/** App background. The deepest violet in the room. */
val NightVoid = Color(0xFF150A2B)
/** Cards, sheets, list rows, control buttons. */
val NightPanel = Color(0xFF221046)
/** Dialogs and anything that sits above a panel. */
val NightRaised = Color(0xFF2E1660)
/** Dividers, borders, unchecked switch tracks. */
val NightEdge = Color(0xFF45268A)

// --- Honey: identity --------------------------------------------------------
/** Primary action. GB's body yellow. */
val GoldenSaplight = Color(0xFFFACC15)
/** Pressed and held states of a honey control. */
val AmberResin = Color(0xFFA16207)
/** Honey at half power: progress tracks, secondary fills. */
val HoneyLight = Color(0xFFD4A017)
/** Body text and icons on any night surface. */
val PineGlowMist = Color(0xFFFFF9C2)
/** Ink on a honey fill. GB's own outline colour. */
val BurntRoot = Color(0xFF1A0F00)
/** Teeth cream. Headings and large numerals. */
val BeeWing = Color(0xFFFDEEB7)

// --- Neon: accents from GB's room -------------------------------------------
/** The Game Boy Advance shell. Secondary actions, save-state chrome. */
val NeonViolet = Color(0xFF7C30BC)
/** The deepest neon in the room. Selected chips, active tabs. */
val NeonMagenta = Color(0xFF650EBA)
/** GB's lens glint. Focus rings, "this is live" state. */
val LensCyan = Color(0xFF55F6FD)
/** Wing lavender. Disabled controls and secondary text. */
val WingLavender = Color(0xFFB0A6CD)
/** GB's blush. Cute confirmations, favourite marks. */
val BlushPink = Color(0xFFFCA8CE)
/** The console power LED. Emulation running. */
val LedGreen = Color(0xFF7CE04A)

// --- Semantic ---------------------------------------------------------------
val Primary = GoldenSaplight
val OnPrimary = BurntRoot
val PrimaryVariant = AmberResin
val Secondary = NeonViolet
val OnSecondary = PineGlowMist
val SecondaryVariant = NeonMagenta
val Background = NightVoid
val OnBackground = PineGlowMist
val Surface = NightPanel
val OnSurface = PineGlowMist
val Error = Color(0xFFFF5C7A)
val OnError = NightVoid

object GeeBeeAyyDarkColors {
    val primary = Primary
    val onPrimary = OnPrimary
    val primaryVariant = PrimaryVariant
    val secondary = Secondary
    val onSecondary = OnSecondary
    val secondaryVariant = SecondaryVariant
    val background = Background
    val onBackground = OnBackground
    val surface = Surface
    val onSurface = OnSurface
    val error = Error
    val onError = OnError
    val isLight = false
}
