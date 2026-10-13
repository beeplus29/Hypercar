package com.helios.cluster.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.helios.cluster.ClusterViewModel
import com.helios.cluster.ui.gauges.GForceMeter
import com.helios.cluster.ui.gauges.PowerRegenBar
import com.helios.cluster.ui.gauges.SoCRing
import com.helios.cluster.ui.gauges.SpeedometerArc
import com.helios.cluster.ui.gauges.ThermalGrid
import com.helios.core.vhal.DriveMode
import com.helios.core.vhal.ui.theme.ElectricCyan
import com.helios.core.vhal.ui.theme.HeliosHMITheme
import com.helios.core.vhal.ui.theme.ObsidianBackground
import com.helios.core.vhal.ui.theme.TrackRed
import com.helios.core.vhal.ui.theme.NeonAmber

@Composable
fun ClusterScreen(viewModel: ClusterViewModel) {
    val telemetry by viewModel.telemetry.collectAsState()

    val accentColor = when (telemetry.driveMode) {
        DriveMode.RANGE -> ElectricCyan
        DriveMode.SPORT -> NeonAmber
        DriveMode.TRACK -> TrackRed
        DriveMode.CUSTOM -> ElectricCyan
    }

    HeliosHMITheme(driveMode = telemetry.driveMode) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(ObsidianBackground)
                .padding(24.dp),
            contentAlignment = Alignment.Center
        ) {
            Column(
                modifier = Modifier.fillMaxSize(),
                verticalArrangement = Arrangement.SpaceBetween,
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    SoCRing(
                        socPercent = telemetry.stateOfCharge,
                        rangeKm = telemetry.estimatedRangeKm
                    )

                    ThermalGrid(
                        frontMotorTemp = telemetry.frontMotorTemp,
                        rearMotorTemp = telemetry.rearMotorTemp,
                        inverterTemp = telemetry.inverterTemp,
                        batteryTemp = telemetry.batteryPackTemp,
                        fuelCellTemp = telemetry.fuelCellTemp
                    )
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceAround,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    GForceMeter(
                        gX = telemetry.gForceX,
                        gY = telemetry.gForceY,
                        accentColor = accentColor
                    )

                    SpeedometerArc(
                        speedKph = telemetry.speedKph,
                        accentColor = accentColor
                    )

                    GForceMeter(
                        gX = -telemetry.gForceX,
                        gY = telemetry.gForceY,
                        accentColor = accentColor
                    )
                }

                PowerRegenBar(
                    powerKw = telemetry.powerKw
                )
            }
        }
    }
}
