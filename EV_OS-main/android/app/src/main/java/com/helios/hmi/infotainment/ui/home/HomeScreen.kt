package com.helios.hmi.infotainment.ui.home

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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.helios.hmi.ui.theme.CarbonCard
import com.helios.hmi.ui.theme.CarbonSurface
import com.helios.hmi.ui.theme.ElectricCyan
import com.helios.hmi.vhal.TelemetryState

@Composable
fun HomeScreen(
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
        // Quick Status Bar
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .background(CarbonCard, RoundedCornerShape(12.dp))
                .padding(horizontal = 16.dp, vertical = 12.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                StatusItem(label = "MODE", value = telemetry.driveMode.name)
                StatusItem(label = "AERO", value = telemetry.aeroMode.name.replace("_", " "))
                StatusItem(label = "BATTERY", value = "${telemetry.stateOfCharge.toInt()}% (${telemetry.estimatedRangeKm} km)")
            }
            StatusItem(label = "SYSTEM", value = "ONLINE (ASIL D)")
        }

        // Split-Card Layout: Map/Navigation (Base Layer) & Persistent Media (Top Layer)
        Row(
            modifier = Modifier.fillMaxWidth().weight(1f),
            horizontalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            // Map / Navigation Card (Base Layer)
            Card(
                modifier = Modifier.weight(1.5f).fillMaxSize(),
                shape = RoundedCornerShape(16.dp),
                colors = CardDefaults.cardColors(containerColor = CarbonCard)
            ) {
                Box(
                    modifier = Modifier.fillMaxSize().padding(20.dp),
                    contentAlignment = Alignment.BottomStart
                ) {
                    Column {
                        Text(
                            text = "NAVIGATION (ACTIVE ROUTE)",
                            style = MaterialTheme.typography.labelLarge.copy(
                                fontSize = 11.sp,
                                fontWeight = FontWeight.Bold,
                                color = ElectricCyan
                            )
                        )
                        Spacer(modifier = Modifier.height(4.dp))
                        Text(
                            text = telemetry.navDestination,
                            style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.Bold)
                        )
                        Spacer(modifier = Modifier.height(2.dp))
                        Text(
                            text = "${telemetry.navDistanceRemaining} • ETA ${telemetry.navEta} • ${telemetry.nextManeuver}",
                            style = MaterialTheme.typography.bodyMedium
                        )
                    }
                }
            }

            // Persistent Media Card (Top Layer)
            Card(
                modifier = Modifier.weight(1f).fillMaxSize(),
                shape = RoundedCornerShape(16.dp),
                colors = CardDefaults.cardColors(containerColor = CarbonCard)
            ) {
                Column(
                    modifier = Modifier.fillMaxSize().padding(20.dp),
                    verticalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(
                        text = "NOW PLAYING",
                        style = MaterialTheme.typography.labelLarge.copy(
                            fontSize = 11.sp,
                            fontWeight = FontWeight.Bold,
                            color = ElectricCyan
                        )
                    )

                    Column {
                        Text(
                            text = telemetry.mediaTitle,
                            style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.Bold)
                        )
                        Spacer(modifier = Modifier.height(2.dp))
                        Text(
                            text = "${telemetry.mediaArtist} — ${telemetry.mediaAlbum}",
                            style = MaterialTheme.typography.bodyMedium
                        )
                    }

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = if (telemetry.isPlaying) "Playing" else "Paused",
                            style = MaterialTheme.typography.bodyMedium.copy(color = ElectricCyan)
                        )
                        androidx.compose.material3.Button(
                            onClick = onTogglePlay
                        ) {
                            Text(text = if (telemetry.isPlaying) "Pause" else "Play")
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun StatusItem(label: String, value: String) {
    Column {
        Text(
            text = label,
            style = MaterialTheme.typography.bodyMedium.copy(fontSize = 10.sp, color = Color.Gray)
        )
        Text(
            text = value,
            style = MaterialTheme.typography.bodyMedium.copy(fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
        )
    }
}
