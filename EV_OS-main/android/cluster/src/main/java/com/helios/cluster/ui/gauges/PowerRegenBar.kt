package com.helios.cluster.ui.gauges

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.helios.core.vhal.ui.theme.PowerOrange
import com.helios.core.vhal.ui.theme.RegenGreen
import com.helios.core.vhal.ui.theme.TextSecondary

@Composable
fun PowerRegenBar(
    powerKw: Float,
    modifier: Modifier = Modifier
) {
    val animatedPower by animateFloatAsState(
        targetValue = powerKw,
        animationSpec = tween(durationMillis = 150),
        label = "powerAnimation"
    )

    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(16.dp)
    ) {
        val formattedPower = if (animatedPower >= 0) "+${animatedPower.toInt()} kW" else "${animatedPower.toInt()} kW"
        val powerLabel = if (animatedPower >= 0) "POWER OUTPUT" else "REGENERATION"
        val barColor = if (animatedPower >= 0) PowerOrange else RegenGreen

        Text(
            text = "$powerLabel: $formattedPower",
            style = MaterialTheme.typography.bodyMedium.copy(
                fontSize = 12.sp,
                fontWeight = FontWeight.Bold,
                color = TextSecondary
            )
        )
        Spacer(modifier = Modifier.height(6.dp))

        Canvas(
            modifier = Modifier
                .fillMaxWidth()
                .height(20.dp)
        ) {
            val width = size.width
            val height = size.height
            val centerX = width / 2f

            drawRoundRect(
                color = Color(0xFF1E222B),
                cornerRadius = CornerRadius(10f, 10f),
                size = Size(width, height)
            )

            drawLine(
                color = Color(0xFF475569),
                start = Offset(centerX, 0f),
                end = Offset(centerX, height),
                strokeWidth = 3f
            )

            val maxPower = 1000f
            val normalized = (animatedPower / maxPower).coerceIn(-1f, 1f)
            val barWidth = kotlin.math.abs(normalized) * (centerX)
            
            if (animatedPower > 0) {
                drawRoundRect(
                    color = barColor,
                    topLeft = Offset(centerX, 0f),
                    size = Size(barWidth, height),
                    cornerRadius = CornerRadius(10f, 10f)
                )
            } else if (animatedPower < 0) {
                drawRoundRect(
                    color = barColor,
                    topLeft = Offset(centerX - barWidth, 0f),
                    size = Size(barWidth, height),
                    cornerRadius = CornerRadius(10f, 10f)
                )
            }
        }
    }
}
