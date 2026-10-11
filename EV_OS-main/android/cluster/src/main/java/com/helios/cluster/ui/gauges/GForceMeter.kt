package com.helios.cluster.ui.gauges

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.helios.core.vhal.ui.theme.CarbonCard
import com.helios.core.vhal.ui.theme.TextMuted

@Composable
fun GForceMeter(
    gX: Float,
    gY: Float,
    accentColor: Color,
    modifier: Modifier = Modifier
) {
    val animX by animateFloatAsState(targetValue = gX, animationSpec = tween(100), label = "gX")
    val animY by animateFloatAsState(targetValue = gY, animationSpec = tween(100), label = "gY")

    Box(
        modifier = modifier
            .clip(RoundedCornerShape(12.dp))
            .background(CarbonCard)
            .size(130.dp),
        contentAlignment = Alignment.Center
    ) {
        Canvas(modifier = Modifier.size(110.dp)) {
            val center = Offset(size.width / 2f, size.height / 2f)
            val radius = size.minDimension / 2f - 4f

            drawCircle(color = Color(0xFF2C3342), radius = radius, center = center, style = Stroke(width = 2f))
            drawCircle(color = Color(0xFF2C3342), radius = radius * 0.66f, center = center, style = Stroke(width = 1.5f))
            drawCircle(color = Color(0xFF2C3342), radius = radius * 0.33f, center = center, style = Stroke(width = 1.5f))

            drawLine(color = Color(0xFF2C3342), start = Offset(center.x, 0f), end = Offset(center.x, size.height), strokeWidth = 1.5f)
            drawLine(color = Color(0xFF2C3342), start = Offset(0f, center.y), end = Offset(size.width, center.y), strokeWidth = 1.5f)

            val maxG = 2.0f
            val dotX = center.x + (animX / maxG) * (radius - 8f)
            val dotY = center.y - (animY / maxG) * (radius - 8f)

            drawCircle(
                color = accentColor,
                radius = 6f,
                center = Offset(dotX, dotY)
            )
        }

        Column(
            modifier = Modifier.align(Alignment.TopStart).padding(6.dp)
        ) {
            Text(
                text = "G-FORCE",
                style = MaterialTheme.typography.labelLarge.copy(
                    fontSize = 9.sp,
                    fontWeight = FontWeight.Bold,
                    color = TextMuted
                )
            )
        }
    }
}
