package com.helios.hmi.vhal

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlin.math.sin

object SimulatedVhalRepository {
    private val _telemetry = MutableStateFlow(TelemetryState())
    val telemetry: StateFlow<TelemetryState> = _telemetry.asStateFlow()

    private val scope = CoroutineScope(Dispatchers.Default)
    private var simulationJob: Job? = null
    private var tickCount = 0

    init {
        startSimulation()
    }

    private fun startSimulation() {
        simulationJob?.cancel()
        simulationJob = scope.launch {
            while (true) {
                delay(100) // 10 FPS simulation tick for smooth UI updates
                tickCount++
                
                _telemetry.update { current ->
                    // Simulate dynamic fluctuations based on current drive mode
                    val targetSpeed = when (current.driveMode) {
                        DriveMode.RANGE -> 110f
                        DriveMode.SPORT -> 180f
                        DriveMode.TRACK -> 260f
                        DriveMode.CUSTOM -> 200f
                    }
                    
                    // Simple oscillating speed & power simulation
                    val speedWave = (sin(tickCount * 0.05) * 15f).toFloat()
                    val simulatedSpeed = (targetSpeed * 0.7f + speedWave).coerceIn(0f, 350f)
                    
                    val powerWave = (sin(tickCount * 0.1) * 350f).toFloat()
                    val simulatedPower = if (simulatedSpeed > 0) {
                        if (powerWave < 0) powerWave * (when (current.regenLevel) {
                            RegenLevel.OFF -> 0.1f
                            RegenLevel.LOW -> 0.5f
                            RegenLevel.MEDIUM -> 1.0f
                            RegenLevel.MAX -> 1.8f
                        }) else powerWave * 2.5f
                    } else 0f

                    val gX = (sin(tickCount * 0.07) * 0.8f).toFloat()
                    val gY = (cos(tickCount * 0.09) * 1.2f).toFloat()

                    current.copy(
                        speedKph = simulatedSpeed,
                        powerKw = simulatedPower,
                        gForceX = gX,
                        gForceY = gY,
                        frontMotorTemp = 68f + (simulatedSpeed * 0.02f),
                        rearMotorTemp = 71f + (simulatedSpeed * 0.03f),
                        inverterTemp = 64f + (simulatedSpeed * 0.015f),
                        batteryPackTemp = 34f + (simulatedSpeed * 0.008f)
                    )
                }
            }
        }
    }

    fun setDriveMode(mode: DriveMode) {
        _telemetry.update { it.copy(driveMode = mode) }
    }

    fun setAeroMode(aero: AeroMode) {
        _telemetry.update { it.copy(aeroMode = aero) }
    }

    fun setSuspensionStiffness(stiffness: SuspensionStiffness) {
        _telemetry.update { it.copy(suspensionStiffness = stiffness) }
    }

    fun setRegenLevel(level: RegenLevel) {
        _telemetry.update { it.copy(regenLevel = level) }
    }

    fun setHvacPower(on: Boolean) {
        _telemetry.update { it.copy(hvacPowerOn = on) }
    }

    fun setDriverTemp(temp: Float) {
        _telemetry.update { it.copy(driverTemp = temp) }
    }

    fun setPassengerTemp(temp: Float) {
        _telemetry.update { it.copy(passengerTemp = temp) }
    }

    fun setFanSpeed(speed: Int) {
        _telemetry.update { it.copy(fanSpeed = speed.coerceIn(1, 5)) }
    }

    fun togglePlayPause() {
        _telemetry.update { it.copy(isPlaying = !it.isPlaying) }
    }
}

private fun cos(d: Double): Double = kotlin.math.cos(d)
