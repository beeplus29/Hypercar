package com.helios.hmi.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import com.helios.hmi.vhal.DriveMode

private val DarkColorScheme = darkColorScheme(
    primary = ElectricCyan,
    secondary = NeonAmber,
    tertiary = TrackRed,
    background = ObsidianBackground,
    surface = CarbonSurface,
    surfaceVariant = CarbonCard,
    onPrimary = ObsidianBackground,
    onSecondary = ObsidianBackground,
    onBackground = TextPrimary,
    onSurface = TextPrimary
)

@Composable
fun HeliosHMITheme(
    driveMode: DriveMode = DriveMode.SPORT,
    content: @Composable () -> Unit
) {
    val primaryColor = when (driveMode) {
        DriveMode.RANGE -> ElectricCyan
        DriveMode.SPORT -> NeonAmber
        DriveMode.TRACK -> TrackRed
        DriveMode.CUSTOM -> ElectricCyan
    }

    val colorScheme = DarkColorScheme.copy(
        primary = primaryColor
    )

    MaterialTheme(
        colorScheme = colorScheme,
        typography = Typography,
        content = content
    )
}
