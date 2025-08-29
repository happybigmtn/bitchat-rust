package com.bitcraps.sample

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.ui.Modifier
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.lifecycleScope
import com.bitcraps.sample.ui.theme.BitCrapsSampleTheme
import com.bitcraps.sample.ui.BitCrapsApp
import com.bitcraps.sample.viewmodel.GameViewModel
import com.bitcraps.sdk.BitCrapsSDK
import kotlinx.coroutines.launch
import timber.log.Timber

class MainActivity : ComponentActivity() {
    
    private val gameViewModel: GameViewModel by viewModels {
        object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : androidx.lifecycle.ViewModel> create(modelClass: Class<T>): T {
                return GameViewModel(applicationContext) as T
            }
        }
    }
    
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        
        // Initialize Timber logging
        if (BuildConfig.DEBUG) {
            Timber.plant(Timber.DebugTree())
        }
        
        // Initialize BitCraps SDK
        lifecycleScope.launch {
            try {
                BitCrapsSDK.initialize(applicationContext)
                gameViewModel.onSDKInitialized()
            } catch (e: Exception) {
                Timber.e(e, "Failed to initialize BitCraps SDK")
                gameViewModel.onSDKInitializationFailed(e)
            }
        }
        
        setContent {
            BitCrapsSampleTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background
                ) {
                    BitCrapsApp(gameViewModel = gameViewModel)
                }
            }
        }
    }
    
    override fun onResume() {
        super.onResume()
        gameViewModel.onAppResumed()
    }
    
    override fun onPause() {
        super.onPause()
        gameViewModel.onAppPaused()
    }
    
    override fun onDestroy() {
        super.onDestroy()
        lifecycleScope.launch {
            gameViewModel.onAppDestroyed()
        }
    }
}