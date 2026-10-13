package com.helios.hmi.cluster.ui

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
import com.helios.hmi.cluster.ClusterViewModel
import com.helios.hmi.cluster.ui.gauges.GForceMeter
import com.helios.hmi.cluster.ui.gauges.PowerRegenBar
import com.helios.hmi.cluster.ui.gauges.SoCRing
import com.helios.hmi.cluster.ui.gauges.SpeedometerArc
import com.helios.hmi.cluster.ui.gauges.ThermalGrid
import com.helios.hmi.ui.theme.HeliosHMITheme
import com.helios.hmi.ui.theme.ObsidianBackground

@Composable
fun ClusterScreen(viewModel: ClusterViewModel) {
    val telemetry by viewModel.telemetry.collectAsState()

    val accentColor = when (telemetry.driveMode) {
        com.helios.hmi.vhal.DriveMode.RANGE -> com.helios.hmi.ui.theme.ElectricCyan
        com.helios.hmi.vhal.DriveMode.SPORT -> com.helios.hmi.ui.theme.NeonAmber
        com.helios.hmi.vhal.DriveMode.TRACK -> com.helios.hmi.ui.theme.TrackRed
        com.helios.hmi.vhal.DriveMode.CUSTOM -> com.helios.hmi.ui.theme.ElectricCyan
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
                // Top Row: SoC, Mode Indicator, Thermal Summary
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

                // Center: Speedometer Arc & G-Force Meter
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

                // Bottom: Power / Regen Bar
                PowerRegenBar(
                    powerKw = telemetry.powerKw
                )
            }
        }
    }
}
