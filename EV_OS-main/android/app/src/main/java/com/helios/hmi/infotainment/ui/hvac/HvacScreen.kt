package com.helios.hmi.infotainment.ui.hvac

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.helios.hmi.ui.theme.CarbonCard
import com.helios.hmi.ui.theme.ElectricCyan
import com.helios.hmi.vhal.TelemetryState

@Composable
fun HvacScreen(
    telemetry: TelemetryState,
    onTogglePower: (Boolean) -> Unit,
    onDriverTempChange: (Float) -> Unit,
    onPassengerTempChange: (Float) -> Unit,
    onFanSpeedChange: (Int) -> Unit,
    modifier: Modifier = Modifier
) {
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
                text = "HVAC CLIMATE CONTROL",
                style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.Bold)
            )
            Button(onClick = { onTogglePower(!telemetry.hvacPowerOn) }) {
                Text(text = if (telemetry.hvacPowerOn) "HVAC ON" else "HVAC OFF")
            }
        }

        Row(
            modifier = Modifier.fillMaxWidth().weight(1f),
            horizontalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            // Driver Zone Climate
            ClimateZoneCard(
                title = "DRIVER ZONE",
                temp = telemetry.driverTemp,
                onTempChange = onDriverTempChange,
                modifier = Modifier.weight(1f)
            )

            // Passenger Zone Climate
            ClimateZoneCard(
                title = "PASSENGER ZONE",
                temp = telemetry.passengerTemp,
                onTempChange = onPassengerTempChange,
                modifier = Modifier.weight(1f)
            )
        }

        // Fan Speed & Airflow Controls
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(CarbonCard, RoundedCornerShape(16.dp))
                .padding(16.dp)
        ) {
            Text(
                text = "FAN SPEED: ${telemetry.fanSpeed}",
                style = MaterialTheme.typography.labelLarge.copy(color = ElectricCyan, fontWeight = FontWeight.Bold)
            )
            Spacer(modifier = Modifier.height(8.dp))
            Slider(
                value = telemetry.fanSpeed.toFloat(),
                onValueChange = { onFanSpeedChange(it.toInt()) },
                valueRange = 1f..5f,
                steps = 3
            )
        }
    }
}

@Composable
fun ClimateZoneCard(
    title: String,
    temp: Float,
    onTempChange: (Float) -> Unit,
    modifier: Modifier = Modifier
) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(CarbonCard, RoundedCornerShape(16.dp))
            .padding(20.dp),
        verticalArrangement = Arrangement.SpaceBetween
    ) {
        Text(
            text = title,
            style = MaterialTheme.typography.labelLarge.copy(color = ElectricCyan, fontWeight = FontWeight.Bold)
        )

        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                text = "%.1f°C".format(temp),
                style = MaterialTheme.typography.displayMedium.copy(fontWeight = FontWeight.Bold)
            )
        }

        Column {
            Text(
                text = "Temperature Adjustment",
                style = MaterialTheme.typography.bodyMedium.copy(fontSize = 12.sp)
            )
            Spacer(modifier = Modifier.height(4.dp))
            Slider(
                value = temp,
                onValueChange = onTempChange,
                valueRange = 16.0f..30.0f
            )
        }
    }
}
