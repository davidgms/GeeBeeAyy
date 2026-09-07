package com.geebeeayy.app.ui.screens

import android.provider.Settings
import androidx.compose.animation.core.*
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.geebeeayy.app.ui.theme.*
import kotlinx.coroutines.delay

@Composable
fun SplashScreen(onTimeout: () -> Unit) {
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

    LaunchedEffect(Unit) {
        startAnimation = true
        delay(2000)
        onTimeout()
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(BurntRoot),
        contentAlignment = Alignment.Center
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
            modifier = Modifier
                .scale(scale.value)
                .alpha(alpha.value)
        ) {
            // Bee character placeholder (will use actual asset)
            Box(
                modifier = Modifier
                    .size(120.dp)
                    .background(GoldenSaplight, shape = MaterialTheme.shapes.extraLarge)
            )

            Spacer(modifier = Modifier.height(24.dp))

            Text(
                text = "GeeBeeAyy!",
                fontSize = 48.sp,
                fontWeight = FontWeight.Bold,
                color = GoldenSaplight
            )

            Spacer(modifier = Modifier.height(8.dp))

            Text(
                text = "GBA Emulator",
                fontSize = 18.sp,
                fontWeight = FontWeight.Light,
                color = PineGlowMist
            )
        }

        // Bottom tagline
        Text(
            text = "Bzzt! Let's play!",
            fontSize = 14.sp,
            color = AmberResin,
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .padding(bottom = 48.dp)
                .alpha(alpha.value)
        )
    }
}
