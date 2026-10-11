package com.helios.infotainment.ui.controls

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
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ElevatedButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.helios.core.vhal.AeroMode
import com.helios.core.vhal.DriveMode
import com.helios.core.vhal.RegenLevel
import com.helios.core.vhal.SuspensionStiffness
import com.helios.core.vhal.TelemetryState
import com.helios.core.vhal.ui.theme.CarbonCard
import com.helios.core.vhal.ui.theme.ElectricCyan

@Composable
fun VehicleControlsScreen(
    telemetry: TelemetryState,
    onSelectDriveMode: (DriveMode) -> Unit,
    onSelectAero: (AeroMode) -> Unit,
    onSelectSuspension: (SuspensionStiffness) -> Unit,
    onSelectRegen: (RegenLevel) -> Unit,
    modifier: Modifier = Modifier
) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Text(
            text = "VEHICLE DYNAMICS & CONTROLS",
            style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.Bold)
        )

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            ControlSection(
                title = "DRIVE MODES",
                modifier = Modifier.weight(1f)
            ) {
                DriveMode.entries.forEach { mode ->
                    ControlOptionButton(
                        label = mode.name,
                        isSelected = telemetry.driveMode == mode,
                        onClick = { onSelectDriveMode(mode) }
                    )
                }
            }

            ControlSection(
                title = "ACTIVE AERO (AGEDS)",
                modifier = Modifier.weight(1f)
            ) {
                AeroMode.entries.forEach { aero ->
                    ControlOptionButton(
                        label = aero.name.replace("_", " "),
                        isSelected = telemetry.aeroMode == aero,
                        onClick = { onSelectAero(aero) }
                    )
                }
            }
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            ControlSection(
                title = "SUSPENSION STIFFNESS",
                modifier = Modifier.weight(1f)
            ) {
                SuspensionStiffness.entries.forEach { stiffness ->
                    ControlOptionButton(
                        label = stiffness.name,
                        isSelected = telemetry.suspensionStiffness == stiffness,
                        onClick = { onSelectSuspension(stiffness) }
                    )
                }
            }

            ControlSection(
                title = "REGEN BRAKING INTENSITY",
                modifier = Modifier.weight(1f)
            ) {
                RegenLevel.entries.forEach { level ->
                    ControlOptionButton(
                        label = level.name,
                        isSelected = telemetry.regenLevel == level,
                        onClick = { onSelectRegen(level) }
                    )
                }
            }
        }
    }
}

@Composable
fun ControlSection(
    title: String,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit
) {
    Column(
        modifier = modifier
            .background(CarbonCard, RoundedCornerShape(16.dp))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text(
            text = title,
            style = MaterialTheme.typography.labelLarge.copy(
                fontSize = 12.sp,
                fontWeight = FontWeight.Bold,
                color = ElectricCyan
            )
        )
        Spacer(modifier = Modifier.height(4.dp))
        content()
    }
}

@Composable
fun ControlOptionButton(
    label: String,
    isSelected: Boolean,
    onClick: () -> Unit
) {
    ElevatedButton(
        onClick = onClick,
        modifier = Modifier.fillMaxWidth(),
        colors = ButtonDefaults.elevatedButtonColors(
            containerColor = if (isSelected) ElectricCyan else CarbonCard,
            contentColor = if (isSelected) MaterialTheme.colorScheme.background else MaterialTheme.colorScheme.onSurface
        ),
        shape = RoundedCornerShape(8.dp)
    ) {
        Text(text = label, fontWeight = FontWeight.SemiBold)
    }
}
