package com.helios.hmi.vhal

enum class DriveMode {
    RANGE,
    SPORT,
    TRACK,
    CUSTOM
}

enum class AeroMode {
    LOW_DRAG,
    HIGH_DOWNFORCE
}

enum class SuspensionStiffness {
    COMFORT,
    DYNAMIC,
    RACE
}

enum class RegenLevel {
    OFF,
    LOW,
    MEDIUM,
    MAX
}

data class TelemetryState(
    val speedKph: Float = 0f,
    val powerKw: Float = 0f, // Positive = acceleration output, Negative = regen recovery
    val stateOfCharge: Float = 88.5f, // Percentage 0 - 100
    val estimatedRangeKm: Int = 420,
    val driveMode: DriveMode = DriveMode.SPORT,
    val aeroMode: AeroMode = AeroMode.HIGH_DOWNFORCE,
    val suspensionStiffness: SuspensionStiffness = SuspensionStiffness.DYNAMIC,
    val regenLevel: RegenLevel = RegenLevel.MEDIUM,
    
    // Thermal monitoring (Celsius)
    val frontMotorTemp: Float = 68.5f,
    val rearMotorTemp: Float = 72.0f,
    val inverterTemp: Float = 65.0f,
    val batteryPackTemp: Float = 34.2f,
    val fuelCellTemp: Float = 78.0f,
    
    // G-Force
    val gForceX: Float = 0.0f, // Lateral
    val gForceY: Float = 0.0f, // Longitudinal (Acceleration/Braking)
    
    // HVAC State
    val driverTemp: Float = 21.5f,
    val passengerTemp: Float = 22.0f,
    val hvacPowerOn: Boolean = true,
    val fanSpeed: Int = 3,
    val acActive: Boolean = true,
    
    // Media State
    val mediaTitle: String = "Hypercar Pulse (Live Mix)",
    val mediaArtist: String = "HELIOS Audio Core",
    val mediaAlbum: String = "EV_OS Vol. 1",
    val isPlaying: Boolean = true,
    val playbackProgress: Float = 0.35f,
    
    // Navigation State
    val navDestination: String = "Nürburgring Nordschleife GP",
    val navEta: String = "14:25",
    val navDistanceRemaining: String = "18.4 km",
    val nextManeuver: String = "In 300m, turn sharp right onto Karussell"
)
