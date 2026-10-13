package com.helios.hmi.infotainment

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import com.helios.hmi.infotainment.ui.InfotainmentScreen
import com.helios.hmi.ui.theme.HeliosHMITheme

class InfotainmentActivity : ComponentActivity() {
    private val viewModel: InfotainmentViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            HeliosHMITheme {
                InfotainmentScreen(viewModel = viewModel)
            }
        }
    }
}
