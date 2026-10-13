package com.helios.infotainment.ui.media

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.helios.core.vhal.TelemetryState
import com.helios.core.vhal.ui.theme.CarbonCard
import com.helios.core.vhal.ui.theme.ElectricCyan

@Composable
fun MediaPlayerScreen(
    telemetry: TelemetryState,
    onTogglePlay: () -> Unit,
    modifier: Modifier = Modifier
) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Text(
            text = "MEDIA CENTER",
            style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.Bold)
        )

        Row(
            modifier = Modifier.fillMaxWidth().weight(1f),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Box(
                modifier = Modifier
                    .size(220.dp)
                    .clip(RoundedCornerShape(16.dp))
                    .background(CarbonCard),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    text = "HELIOS ART",
                    style = MaterialTheme.typography.titleLarge.copy(color = ElectricCyan, fontWeight = FontWeight.Bold)
                )
            }

            Column(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxSize()
                    .background(CarbonCard, RoundedCornerShape(16.dp))
                    .padding(24.dp),
                verticalArrangement = Arrangement.SpaceBetween
            ) {
                Column {
                    Text(
                        text = "NOW STREAMING",
                        style = MaterialTheme.typography.labelLarge.copy(color = ElectricCyan, fontWeight = FontWeight.Bold)
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = telemetry.mediaTitle,
                        style = MaterialTheme.typography.headlineMedium.copy(fontWeight = FontWeight.Bold)
                    )
                    Spacer(modifier = Modifier.height(4.dp))
                    Text(
                        text = "${telemetry.mediaArtist} — ${telemetry.mediaAlbum}",
                        style = MaterialTheme.typography.bodyLarge
                    )
                }

                Column {
                    Slider(
                        value = telemetry.playbackProgress,
                        onValueChange = {}
                    )
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Text(text = "1:24", style = MaterialTheme.typography.bodyMedium.copy(fontSize = 12.sp))
                        Text(text = "3:45", style = MaterialTheme.typography.bodyMedium.copy(fontSize = 12.sp))
                    }
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.Center
                ) {
                    Button(onClick = onTogglePlay) {
                        Text(text = if (telemetry.isPlaying) "Pause Playback" else "Resume Playback", fontWeight = FontWeight.Bold)
                    }
                }
            }
        }
    }
}
