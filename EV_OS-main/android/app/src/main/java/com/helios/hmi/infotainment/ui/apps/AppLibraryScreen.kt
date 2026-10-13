package com.helios.hmi.infotainment.ui.apps

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.helios.hmi.ui.theme.CarbonCard
import com.helios.hmi.ui.theme.ElectricCyan
import com.helios.hmi.ui.theme.TextMuted

data class HmiAppItem(
    val name: String,
    val packageName: String,
    val category: String,
    val description: String
)

@Composable
fun AppLibraryScreen(
    onLaunchApp: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    val apps = remember {
        listOf(
            HmiAppItem("Google Maps", "com.google.android.apps.maps", "Navigation", "High-definition 3D satellite navigation & EV routing"),
            HmiAppItem("PlugShare AAOS", "com.xatori.Plugshare_AAOS", "Charging", "Global EV charging station network & station reviews"),
            HmiAppItem("EV Charger Locator", "com.crosschasm.evchargerlocator", "Charging", "Real-time DC fast charger availability & status"),
            HmiAppItem("Amazon Music Auto", "com.amazon.mp3.automotiveOS", "Media", "High-res spatial audio & curated hypercar playlists"),
            HmiAppItem("YouTube Music", "com.google.android.apps.youtube.music", "Media", "On-demand music streaming & personalized mixes"),
            HmiAppItem("TuneIn Radio", "tunein.player", "Media", "Global live radio, podcasts, and motorsports broadcasts"),
            HmiAppItem("OpenRadio", "com.yuriy.openradio", "Media", "Internet radio player optimized for Android Automotive"),
            HmiAppItem("Anghami", "com.anghami", "Media", "Middle East & international music streaming catalog"),
            HmiAppItem("MyRadar Weather", "com.acmeaom.android.myradar", "Utilities", "High-definition live weather radar & track conditions"),
            HmiAppItem("Home Assistant", "io.homeassistant.companion.android", "Smart Home", "Automotive smart home integration & garage door triggers"),
            HmiAppItem("Google Chrome", "com.android.chrome", "Utilities", "Full web browsing experience on center console"),
            HmiAppItem("ParkWhiz", "com.parkwhiz.driverApp", "Utilities", "Parking reservation & automated garage ticketing"),
            HmiAppItem("Fermata Auto", "me.aap.fermata.auto", "Media", "Digital media player & web video streaming for car"),
            HmiAppItem("Amazon Prime Video", "com.amazon.avod.thirdpartyclient", "Media", "Cinematic video streaming while parked"),
            HmiAppItem("YouTube Automotive", "com.google.android.apps.automotive.youtube", "Media", "In-car video entertainment")
        )
    }

    var selectedCategory by remember { mutableStateOf("All") }
    val categories = listOf("All", "Navigation", "Charging", "Media", "Utilities", "Smart Home")

    val filteredApps = if (selectedCategory == "All") apps else apps.filter { it.category == selectedCategory }

    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                text = "AAOS INTEGRATED APP LAUNCHER",
                style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.Bold)
            )
            Text(
                text = "${apps.size} Prebuilt Apps Installed",
                style = MaterialTheme.typography.bodyMedium.copy(color = ElectricCyan)
            )
        }

        // Category Filter Chips
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            categories.forEach { cat ->
                Button(
                    onClick = { selectedCategory = cat },
                    colors = androidx.compose.material3.ButtonDefaults.buttonColors(
                        containerColor = if (selectedCategory == cat) ElectricCyan else CarbonCard,
                        contentColor = if (selectedCategory == cat) MaterialTheme.colorScheme.background else MaterialTheme.colorScheme.onSurface
                    )
                ) {
                    Text(text = cat, fontSize = 12.sp)
                }
            }
        }

        // App Grid
        LazyVerticalGrid(
            columns = GridCells.Fixed(3),
            modifier = Modifier.fillMaxWidth().weight(1f),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            items(filteredApps) { app ->
                AppCard(
                    app = app,
                    onClick = { onLaunchApp(app.packageName) }
                )
            }
        }
    }
}

@Composable
fun AppCard(
    app: HmiAppItem,
    onClick: () -> Unit
) {
    Card(
        modifier = Modifier.fillMaxWidth().height(140.dp),
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = CarbonCard),
        onClick = onClick
    ) {
        Column(
            modifier = Modifier.fillMaxSize().padding(16.dp),
            verticalArrangement = Arrangement.SpaceBetween
        ) {
            Column {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(
                        text = app.name,
                        style = MaterialTheme.typography.titleLarge.copy(fontSize = 16.sp, fontWeight = FontWeight.Bold)
                    )
                    Text(
                        text = app.category,
                        style = MaterialTheme.typography.bodyMedium.copy(fontSize = 10.sp, color = ElectricCyan)
                    )
                }
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = app.description,
                    style = MaterialTheme.typography.bodyMedium.copy(fontSize = 12.sp, color = TextMuted),
                    maxLines = 2
                )
            }

            Text(
                text = "Launch app →",
                style = MaterialTheme.typography.labelLarge.copy(fontSize = 12.sp, color = ElectricCyan, fontWeight = FontWeight.SemiBold)
            )
        }
    }
}
