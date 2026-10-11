package com.helios.infotainment

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import com.helios.core.vhal.ui.theme.HeliosHMITheme
import com.helios.infotainment.ui.InfotainmentScreen

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
