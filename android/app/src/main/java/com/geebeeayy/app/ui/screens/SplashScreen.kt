package com.geebeeayyayy.app.ui.screens

import androidx.compose.animation.core.*
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.geebeeayyayy.app.ui.theme.*
import kotlinx.coroutines.delay

@Composable
fun SplashScreen(onTimeout: () -> Unit) {
    var startAnimation by remember { mutableStateOf(false) }

    val scale = animateFloatAsState(
        targetValue = if (startAnimation) 1f else 0.5f,
        animationSpec = tween(durationMillis = 500, easing = FastOutSlowInEasing),
        label = "scale"
    )

    val alpha = animateFloatAsState(
        targetValue = if (startAnimation) 1f else 0f,
        animationSpec = tween(durationMillis = 800),
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
