package com.helios.hmi.cluster

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import com.helios.hmi.cluster.ui.ClusterScreen
import com.helios.hmi.ui.theme.HeliosHMITheme

class ClusterActivity : ComponentActivity() {
    private val viewModel: ClusterViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            ClusterScreen(viewModel = viewModel)
        }
    }
}
