package com.helios.hmi.cluster

import androidx.lifecycle.ViewModel
import com.helios.hmi.vhal.SimulatedVhalRepository
import com.helios.hmi.vhal.TelemetryState
import kotlinx.coroutines.flow.StateFlow

class ClusterViewModel : ViewModel() {
    val telemetry: StateFlow<TelemetryState> = SimulatedVhalRepository.telemetry
}
