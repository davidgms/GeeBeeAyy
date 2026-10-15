package com.gbee.emulator.ui.screens

import android.provider.Settings
import androidx.compose.animation.core.*
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.Modifier
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.res.imageResource
import androidx.compose.ui.draw.scale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.gbee.emulator.R
import com.gbee.emulator.ui.theme.*

/**
 * The splash, up for exactly as long as the startup load takes. [progress] is
 * the real fraction done (0 to 1) and [loaded] says the load has finished; the
 * bar eases toward [progress] and the splash calls [onFinished] once the bar
 * has reached the end of a finished load. No fixed timer.
 */
@Composable
fun SplashScreen(progress: Float, loaded: Boolean, onFinished: () -> Unit) {
    var startAnimation by remember { mutableStateOf(false) }

    // Honour "Remove animations" in system accessibility settings. A scale-up
    // is exactly the kind of motion that triggers vestibular symptoms, and the
    // splash is the first thing the app shows.
    val context = LocalContext.current
    val reduceMotion = remember {
        Settings.Global.getFloat(
            context.contentResolver,
            Settings.Global.ANIMATOR_DURATION_SCALE,
            1f,
        ) == 0f
    }
    val scaleDuration = if (reduceMotion) 0 else 500
    val alphaDuration = if (reduceMotion) 0 else 800

    val scale = animateFloatAsState(
        targetValue = if (startAnimation) 1f else 0.5f,
        animationSpec = tween(durationMillis = scaleDuration, easing = FastOutSlowInEasing),
        label = "scale"
    )

    val alpha = animateFloatAsState(
        targetValue = if (startAnimation) 1f else 0f,
        animationSpec = tween(durationMillis = alphaDuration),
        label = "alpha"
    )

    // The bar eases toward the real progress so a jump reads as motion; with
    // animations removed in system settings it jumps straight there.
    val shown = remember { Animatable(0f) }

    // A new word from res/values/loading_words.xml on every splash.
    val loadingWord = remember { context.resources.getStringArray(R.array.loading_words).random() }

    LaunchedEffect(Unit) { startAnimation = true }

    LaunchedEffect(progress, loaded) {
        val target = if (loaded) 1f else progress.coerceIn(0f, 1f)
        shown.animateTo(target, tween(durationMillis = if (reduceMotion) 0 else 250, easing = LinearEasing))
        if (loaded) onFinished()
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(NightVoid),
        contentAlignment = Alignment.Center
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
            modifier = Modifier
                .scale(scale.value)
                .alpha(alpha.value)
        ) {
            // GB, the mascot. Rendered from temp/design-canvas/gb.mjs; see
            // docs/design/DESIGN-SYSTEM.md for how he is allowed to be used.
            Image(
                painter = painterResource(id = R.drawable.gb_face),
                contentDescription = null,
                modifier = Modifier.size(160.dp)
            )

            Spacer(modifier = Modifier.height(40.dp))

            // The wordmark is the one place the pixel face has to appear: it is
            // the app's identity, not running text. Press Start 2P is 1em per
            // glyph, so 22sp keeps "GBee Emulator" at 286dp, inside a 360dp
            // phone on one line.
            Text(
                text = "GBee Emulator",
                fontFamily = PressStart2P,
                fontSize = 22.sp,
                maxLines = 1,
                color = GoldenSaplight
            )

            Spacer(modifier = Modifier.height(20.dp))

            Text(
                text = "GBA Emulator",
                fontSize = 18.sp,
                fontWeight = FontWeight.Light,
                color = PineGlowMist
            )
        }

        // Loading bar, driven by the real startup load; the bee flies on the
        // leading edge of the fill.
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .padding(start = 48.dp, end = 48.dp, bottom = 48.dp)
                .fillMaxWidth()
                .alpha(alpha.value)
        ) {
            Text(
                text = loadingWord,
                fontSize = 16.sp,
                fontWeight = FontWeight.Bold,
                color = GoldenSaplight,
            )
            BoxWithConstraints(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(48.dp)
            ) {
                val markerWidth = 48.dp
                val track = Modifier
                    .align(Alignment.CenterStart)
                    .height(8.dp)
                    .clip(RoundedCornerShape(4.dp))
                Box(track.fillMaxWidth().background(NightRaised))
                Box(track.fillMaxWidth(shown.value).background(GoldenSaplight))
                Image(
                    bitmap = ImageBitmap.imageResource(R.drawable.loading_bee),
                    contentDescription = null,
                    filterQuality = FilterQuality.None,
                    modifier = Modifier
                        .align(Alignment.CenterStart)
                        .offset(x = (maxWidth - markerWidth) * shown.value)
                        .width(markerWidth)
                )
            }
            Text(
                text = "${(shown.value * 100).toInt()}%",
                fontSize = 14.sp,
                fontWeight = FontWeight.Bold,
                color = GoldenSaplight,
            )
        }
    }
}
