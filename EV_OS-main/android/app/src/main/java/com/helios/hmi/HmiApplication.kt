package com.helios.hmi

import android.app.Application

class HmiApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        // Initialize HMI services, telemetry repository, or VHAL bindings here
    }
}
