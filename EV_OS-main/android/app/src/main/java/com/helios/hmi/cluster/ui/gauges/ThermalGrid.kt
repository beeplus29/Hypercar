package com.helios.hmi.cluster.ui.gauges

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.helios.hmi.ui.theme.CarbonCard
import com.helios.hmi.ui.theme.TextMuted
import com.helios.hmi.ui.theme.TextPrimary
import com.helios.hmi.ui.theme.TrackRed

@Composable
fun ThermalGrid(
    frontMotorTemp: Float,
    rearMotorTemp: Float,
    inverterTemp: Float,
    batteryTemp: Float,
    fuelCellTemp: Float,
    modifier: Modifier = Modifier
) {
    Column(
        modifier = modifier
            .clip(RoundedCornerShape(12.dp))
            .background(CarbonCard)
            .padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text(
            text = "THERMAL TELEMETRY",
            style = MaterialTheme.typography.labelLarge.copy(
                fontSize = 11.sp,
                fontWeight = FontWeight.Bold,
                letterSpacing = 1.5.sp,
                color = TextMuted
            )
        )

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            ThermalItem(label = "F. Motor", temp = frontMotorTemp)
            ThermalItem(label = "R. Motor", temp = rearMotorTemp)
            ThermalItem(label = "Inverter", temp = inverterTemp)
        }
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            ThermalItem(label = "Battery", temp = batteryTemp)
            ThermalItem(label = "Fuel Cell", temp = fuelCellTemp)
            ThermalItem(label = "Coolant", temp = 45.0f)
        }
    }
}

@Composable
fun ThermalItem(label: String, temp: Float) {
    Column(
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.bodyMedium.copy(
                fontSize = 10.sp,
                color = TextMuted
            )
        )
        val isHot = temp > 80f
        Text(
            text = "%.1f°C".format(temp),
            style = MaterialTheme.typography.bodyMedium.copy(
                fontSize = 13.sp,
                fontWeight = FontWeight.SemiBold,
                color = if (isHot) TrackRed else TextPrimary
            )
        )
    }
}
