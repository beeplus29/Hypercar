package com.helios.hmi.infotainment

import androidx.lifecycle.ViewModel
import com.helios.hmi.vhal.AeroMode
import com.helios.hmi.vhal.DriveMode
import com.helios.hmi.vhal.RegenLevel
import com.helios.hmi.vhal.SimulatedVhalRepository
import com.helios.hmi.vhal.SuspensionStiffness
import com.helios.hmi.vhal.TelemetryState
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
