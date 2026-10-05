package com.helios.hmi.infotainment.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ScrollableTabRow
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.helios.hmi.infotainment.InfotainmentViewModel
import com.helios.hmi.infotainment.ui.apps.AppLibraryScreen
import com.helios.hmi.infotainment.ui.controls.VehicleControlsScreen
import com.helios.hmi.infotainment.ui.home.HomeScreen
import com.helios.hmi.infotainment.ui.hvac.HvacScreen
import com.helios.hmi.infotainment.ui.media.MediaPlayerScreen
import com.helios.hmi.ui.theme.CarbonSurface
import com.helios.hmi.ui.theme.ElectricCyan
import com.helios.hmi.ui.theme.ObsidianBackground

@Composable
fun InfotainmentScreen(viewModel: InfotainmentViewModel) {
    val telemetry by viewModel.telemetry.collectAsState()
    var selectedTab by remember { mutableIntStateOf(0) }
    val tabs = listOf("Home", "Vehicle Controls", "HVAC", "Media", "Apps")

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(ObsidianBackground)
    ) {
        // AAOS Reference Top Bar / Navigation Tabs
        ScrollableTabRow(
            selectedTabIndex = selectedTab,
            containerColor = CarbonSurface,
            contentColor = ElectricCyan,
            edgePadding = 16.dp
        ) {
            tabs.forEachIndexed { index, title ->
                Tab(
                    selected = selectedTab == index,
                    onClick = { selectedTab = index },
                    text = { Text(text = title, style = MaterialTheme.typography.titleLarge.copy(fontSize = 16.dp.value.sp)) }
                )
            }
        }

        // Content Area
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f)
                .padding(8.dp)
        ) {
            when (selectedTab) {
                0 -> HomeScreen(
                    telemetry = telemetry,
                    onTogglePlay = { viewModel.togglePlayPause() }
                )
                1 -> VehicleControlsScreen(
                    telemetry = telemetry,
                    onSelectDriveMode = { viewModel.setDriveMode(it) },
                    onSelectAero = { viewModel.setAeroMode(it) },
                    onSelectSuspension = { viewModel.setSuspensionStiffness(it) },
                    onSelectRegen = { viewModel.setRegenLevel(it) }
                )
                2 -> HvacScreen(
                    telemetry = telemetry,
                    onTogglePower = { viewModel.setHvacPower(it) },
                    onDriverTempChange = { viewModel.setDriverTemp(it) },
                    onPassengerTempChange = { viewModel.setPassengerTemp(it) },
                    onFanSpeedChange = { viewModel.setFanSpeed(it) }
                )
                3 -> MediaPlayerScreen(
                    telemetry = telemetry,
                    onTogglePlay = { viewModel.togglePlayPause() }
                )
                4 -> AppLibraryScreen(
                    onLaunchApp = { packageName ->
                        // Intent launch action for prebuilt AAOS APKs
                    }
                )
            }
        }
    }
}
