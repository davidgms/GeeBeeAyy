package com.geebeeayy.app.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

private val DarkColorScheme = darkColorScheme(
    primary = AmberResin,
    onPrimary = BurntRoot,
    primaryContainer = HoneyDark,
    onPrimaryContainer = PineGlowMist,
    secondary = GoldenSaplight,
    onSecondary = BurntRoot,
    secondaryContainer = PineGlowMist,
    onSecondaryContainer = BurntRoot,
    background = BurntRoot,
    onBackground = PineGlowMist,
    surface = HoneyDark,
    onSurface = PineGlowMist,
    error = Error,
    onError = BurntRoot,
)

private val LightColorScheme = lightColorScheme(
    primary = AmberResin,
    onPrimary = PineGlowMist,
    primaryContainer = HoneyDark,
    onPrimaryContainer = PineGlowMist,
    secondary = GoldenSaplight,
    onSecondary = BurntRoot,
    secondaryContainer = PineGlowMist,
    onSecondaryContainer = BurntRoot,
    background = PineGlowMist,
    onBackground = BurntRoot,
    surface = PineGlowMist,
    onSurface = BurntRoot,
    error = Error,
    onError = PineGlowMist,
)

private val GeeBeeAyyTypography = Typography(
    displayLarge = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Bold,
        fontSize = 36.sp,
        lineHeight = 44.sp,
        letterSpacing = 0.sp,
    ),
    headlineLarge = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Bold,
        fontSize = 28.sp,
        lineHeight = 36.sp,
        letterSpacing = 0.sp,
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
