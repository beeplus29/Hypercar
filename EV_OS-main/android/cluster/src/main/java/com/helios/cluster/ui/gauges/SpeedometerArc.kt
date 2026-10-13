package com.helios.cluster.ui.gauges

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.helios.core.vhal.ui.theme.TextMuted
import com.helios.core.vhal.ui.theme.TextPrimary
import kotlin.math.cos
import kotlin.math.sin

@Composable
fun SpeedometerArc(
    speedKph: Float,
    maxSpeed: Float = 350f,
    accentColor: Color,
    modifier: Modifier = Modifier
) {
    val animatedSpeed by animateFloatAsState(
        targetValue = speedKph,
        animationSpec = tween(durationMillis = 200),
        label = "speedAnimation"
    )

    val sweepAngle = 240f
    val startAngle = 150f
    val currentProgress = (animatedSpeed / maxSpeed).coerceIn(0f, 1f)
    val currentSweep = sweepAngle * currentProgress

    Box(
        modifier = modifier.size(320.dp),
        contentAlignment = Alignment.Center
    ) {
        Canvas(modifier = Modifier.size(300.dp)) {
            val strokeWidth = 24.dp.toPx()
            val diameter = size.minDimension - strokeWidth
            val topLeft = Offset((size.width - diameter) / 2f, (size.height - diameter) / 2f)
            val arcSize = Size(diameter, diameter)

            drawArc(
                color = Color(0xFF1E222B),
                startAngle = startAngle,
                sweepAngle = sweepAngle,
                useCenter = false,
                topLeft = topLeft,
                size = arcSize,
                style = Stroke(width = strokeWidth, cap = StrokeCap.Round)
            )

            drawArc(
                brush = Brush.sweepGradient(
                    listOf(accentColor.copy(alpha = 0.3f), accentColor, accentColor)
                ),
                startAngle = startAngle,
                sweepAngle = currentSweep,
                useCenter = false,
                topLeft = topLeft,
                size = arcSize,
                style = Stroke(width = strokeWidth, cap = StrokeCap.Round)
            )

            for (i in 0..7) {
                val angleDeg = startAngle + (sweepAngle / 7f) * i
                val angleRad = Math.toRadians(angleDeg.toDouble())
                val radius = (diameter / 2f) - 12.dp.toPx()
                val centerX = size.width / 2f
                val centerY = size.height / 2f
                val startX = centerX + (radius - 8.dp.toPx()) * cos(angleRad).toFloat()
                val startY = centerY + (radius - 8.dp.toPx()) * sin(angleRad).toFloat()
                val endX = centerX + radius * cos(angleRad).toFloat()
                val endY = centerY + radius * sin(angleRad).toFloat()

                drawLine(
                    color = TextMuted,
                    start = Offset(startX, startY),
                    end = Offset(endX, endY),
                    strokeWidth = 3f,
                    cap = StrokeCap.Round
                )
            }
        }

        Column(
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Text(
                text = "${animatedSpeed.toInt()}",
                style = MaterialTheme.typography.displayLarge.copy(
                    fontSize = 72.sp,
                    fontWeight = FontWeight.Bold,
                    color = TextPrimary
                )
            )
            Text(
                text = "KM/H",
                style = MaterialTheme.typography.labelLarge.copy(
                    fontSize = 14.sp,
                    fontWeight = FontWeight.SemiBold,
                    letterSpacing = 2.sp,
                    color = accentColor
                )
            )
        }
    }
}
