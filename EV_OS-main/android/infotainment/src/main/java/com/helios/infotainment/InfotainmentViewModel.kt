package com.helios.infotainment

import androidx.lifecycle.ViewModel
import com.helios.core.vhal.AeroMode
import com.helios.core.vhal.DriveMode
import com.helios.core.vhal.RegenLevel
import com.helios.core.vhal.SimulatedVhalRepository
import com.helios.core.vhal.SuspensionStiffness
import com.helios.core.vhal.TelemetryState
import kotlinx.coroutines.flow.StateFlow

class InfotainmentViewModel : ViewModel() {
    val telemetry: StateFlow<TelemetryState> = SimulatedVhalRepository.telemetry

    fun setDriveMode(mode: DriveMode) = SimulatedVhalRepository.setDriveMode(mode)
    fun setAeroMode(aero: AeroMode) = SimulatedVhalRepository.setAeroMode(aero)
    fun setSuspensionStiffness(stiffness: SuspensionStiffness) = SimulatedVhalRepository.setSuspensionStiffness(stiffness)
    fun setRegenLevel(level: RegenLevel) = SimulatedVhalRepository.setRegenLevel(level)
    fun setHvacPower(on: Boolean) = SimulatedVhalRepository.setHvacPower(on)
    fun setDriverTemp(temp: Float) = SimulatedVhalRepository.setDriverTemp(temp)
    fun setPassengerTemp(temp: Float) = SimulatedVhalRepository.setPassengerTemp(temp)
    fun setFanSpeed(speed: Int) = SimulatedVhalRepository.setFanSpeed(speed)
    fun togglePlayPause() = SimulatedVhalRepository.togglePlayPause()
}
