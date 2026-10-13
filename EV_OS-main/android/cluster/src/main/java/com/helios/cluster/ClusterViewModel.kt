package com.helios.cluster

import androidx.lifecycle.ViewModel
import com.helios.core.vhal.SimulatedVhalRepository
import com.helios.core.vhal.TelemetryState
import kotlinx.coroutines.flow.StateFlow

class ClusterViewModel : ViewModel() {
    val telemetry: StateFlow<TelemetryState> = SimulatedVhalRepository.telemetry
}
