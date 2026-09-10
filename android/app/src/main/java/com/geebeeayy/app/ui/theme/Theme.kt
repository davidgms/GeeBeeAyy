package com.geebeeayy.app.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.*
import androidx.compose.runtime.staticCompositionLocalOf
import com.geebeeayy.app.data.ControlTint
import androidx.compose.runtime.Composable
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import com.geebeeayy.app.R

private val DarkColorScheme = darkColorScheme(
    primary = GoldenSaplight,
    onPrimary = BurntRoot,
    primaryContainer = AmberResin,
    onPrimaryContainer = PineGlowMist,
    secondary = NeonViolet,
    onSecondary = PineGlowMist,
    secondaryContainer = NeonMagenta,
    onSecondaryContainer = PineGlowMist,
    tertiary = LensCyan,
    onTertiary = NightVoid,
    tertiaryContainer = NightEdge,
    onTertiaryContainer = LensCyan,
    background = NightVoid,
    onBackground = PineGlowMist,
    surface = NightPanel,
    onSurface = PineGlowMist,
    surfaceVariant = NightRaised,
    onSurfaceVariant = WingLavender,
    outline = NightEdge,
    outlineVariant = NightRaised,
    error = Error,
    onError = NightVoid,
)

// The app is a night-themed emulator: there is no light scheme, only the dark
// one. This alias keeps `GeeBeeAyyTheme(darkTheme = false)` from silently
// producing an unreadable screen.
private val LightColorScheme = DarkColorScheme

// GB's own lettering. Pixelify Sans is the only face allowed to carry the
// app's identity, and it is allowed only at 28sp and above - the wordmark and
// screen titles. Below that the platform default takes over: a pixel face
// loses its grid under font scaling and stops being readable at body sizes.
// See docs/design/DESIGN-SYSTEM.md.
val Pixelify = FontFamily(
    Font(R.font.pixelify_sans_regular, FontWeight.Normal),
    Font(R.font.pixelify_sans_bold, FontWeight.Bold),
)

private val GeeBeeAyyTypography = Typography(
    displayLarge = TextStyle(
        fontFamily = Pixelify,
        fontWeight = FontWeight.Bold,
        fontSize = 36.sp,
        lineHeight = 48.sp,
        letterSpacing = 0.5.sp,
    ),
    headlineLarge = TextStyle(
        fontFamily = Pixelify,
        fontWeight = FontWeight.Bold,
        fontSize = 28.sp,
        lineHeight = 38.sp,
        letterSpacing = 0.5.sp,
    ),
    headlineMedium = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.SemiBold,
        fontSize = 24.sp,
        lineHeight = 32.sp,
        letterSpacing = 0.sp,
    ),
    titleLarge = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.SemiBold,
        fontSize = 20.sp,
        lineHeight = 28.sp,
        letterSpacing = 0.sp,
    ),
    bodyLarge = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Normal,
        fontSize = 16.sp,
        lineHeight = 24.sp,
        letterSpacing = 0.5.sp,
    ),
    bodyMedium = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Normal,
        fontSize = 14.sp,
        lineHeight = 20.sp,
        letterSpacing = 0.25.sp,
    ),
    labelLarge = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Medium,
        fontSize = 14.sp,
        lineHeight = 20.sp,
        letterSpacing = 0.1.sp,
    ),
)

@Composable
fun GeeBeeAyyTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit
) {
    val colorScheme = if (darkTheme) DarkColorScheme else LightColorScheme

    MaterialTheme(
        colorScheme = colorScheme,
        typography = GeeBeeAyyTypography,
        content = content
    )
}

/**
 * The colours the on-screen controls draw themselves in.
 *
 * A composition local rather than a parameter: the D-pad, the face buttons,
 * the pills and a player's own custom buttons all need the same five colours,
 * and threading them through six signatures - most of which already carry
 * eight arguments for drag handling - buys nothing.
 */
val LocalControlPalette = staticCompositionLocalOf { ControlTint.NIGHT.palette }

/**
 * Multiplier on the on-screen buttons' label sizes. See
 * [com.geebeeayy.app.data.ControlFontSize].
 */
val LocalControlFontScale = staticCompositionLocalOf { 1f }
