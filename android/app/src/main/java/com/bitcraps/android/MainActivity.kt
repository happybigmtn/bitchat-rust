package com.bitcraps.android

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.lifecycle.lifecycleScope
import com.bitcraps.android.ui.theme.BitCrapsTheme
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    
    private lateinit var bitcrapsNode: BitCrapsNode
    private var isBluetoothEnabled by mutableStateOf(false)
    private var isDiscovering by mutableStateOf(false)
    private var connectedPeers by mutableStateOf(listOf<PeerInfo>())
    
    private val bluetoothPermissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions()
    ) { permissions ->
        isBluetoothEnabled = permissions.all { it.value }
        if (isBluetoothEnabled) {
            startDiscovery()
        }
    }
    
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        
        // Initialize BitCraps node
        initializeBitCrapsNode()
        
        // Check and request Bluetooth permissions
        checkBluetoothPermissions()
        
        setContent {
            BitCrapsTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background
                ) {
                    MainScreen()
                }
            }
        }
    }
    
    @Composable
    fun MainScreen() {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Top
        ) {
            Text(
                text = "BitCraps",
                style = MaterialTheme.typography.headlineLarge,
                modifier = Modifier.padding(bottom = 32.dp)
            )
            
            // Status card
            Card(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(bottom = 16.dp)
            ) {
                Column(
                    modifier = Modifier.padding(16.dp)
                ) {
                    Text(
                        text = "Status",
                        style = MaterialTheme.typography.titleMedium,
                        modifier = Modifier.padding(bottom = 8.dp)
                    )
                    Text("Bluetooth: ${if (isBluetoothEnabled) "Enabled" else "Disabled"}")
                    Text("Discovery: ${if (isDiscovering) "Active" else "Inactive"}")
                    Text("Connected Peers: ${connectedPeers.size}")
                }
            }
            
            // Discovery controls
            Row(
                modifier = Modifier.padding(bottom = 16.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Button(
                    onClick = { startDiscovery() },
                    enabled = isBluetoothEnabled && !isDiscovering
                ) {
                    Text("Start Discovery")
                }
                
                Button(
                    onClick = { stopDiscovery() },
                    enabled = isDiscovering
                ) {
                    Text("Stop Discovery")
                }
            }
            
            // Game controls
            Row(
                modifier = Modifier.padding(bottom = 16.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Button(
                    onClick = { createGame() },
                    enabled = connectedPeers.isNotEmpty()
                ) {
                    Text("Create Game")
                }
                
                Button(
                    onClick = { joinGame() },
                    enabled = connectedPeers.isNotEmpty()
                ) {
                    Text("Join Game")
                }
            }
            
            // Peers list
            if (connectedPeers.isNotEmpty()) {
                Card(
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Column(
                        modifier = Modifier.padding(16.dp)
                    ) {
                        Text(
                            text = "Connected Peers",
                            style = MaterialTheme.typography.titleMedium,
                            modifier = Modifier.padding(bottom = 8.dp)
                        )
                        connectedPeers.forEach { peer ->
                            Text("• ${peer.name} (${peer.peerId.take(8)}...)")
                        }
                    }
                }
            }
        }
    }
    
    private fun initializeBitCrapsNode() {
        try {
            // Load native library
            System.loadLibrary("bitcraps")
            
            // Create node configuration
            val config = BitCrapsConfig(
                bluetoothName = "BitCraps-${Build.MODEL}",
                enableBatteryOptimization = true,
                maxPeers = 10,
                discoveryTimeoutSeconds = 30
            )
            
            // Create BitCraps node
            bitcrapsNode = BitCrapsFFI.createNode(config)
            
        } catch (e: Exception) {
            e.printStackTrace()
        }
    }
    
    private fun checkBluetoothPermissions() {
        val permissions = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            arrayOf(
                Manifest.permission.BLUETOOTH_SCAN,
                Manifest.permission.BLUETOOTH_ADVERTISE,
                Manifest.permission.BLUETOOTH_CONNECT,
                Manifest.permission.ACCESS_FINE_LOCATION
            )
        } else {
            arrayOf(
                Manifest.permission.BLUETOOTH,
                Manifest.permission.BLUETOOTH_ADMIN,
                Manifest.permission.ACCESS_FINE_LOCATION
            )
        }
        
        val hasPermissions = permissions.all {
            ContextCompat.checkSelfPermission(this, it) == PackageManager.PERMISSION_GRANTED
        }
        
        if (hasPermissions) {
            isBluetoothEnabled = true
        } else {
            bluetoothPermissionLauncher.launch(permissions)
        }
    }
    
    private fun startDiscovery() {
        lifecycleScope.launch {
            try {
                bitcrapsNode.startDiscovery()
                isDiscovering = true
                
                // Start polling for events
                pollForEvents()
            } catch (e: Exception) {
                e.printStackTrace()
            }
        }
    }
    
    private fun stopDiscovery() {
        lifecycleScope.launch {
            try {
                bitcrapsNode.stopDiscovery()
                isDiscovering = false
            } catch (e: Exception) {
                e.printStackTrace()
            }
        }
    }
    
    private fun createGame() {
        lifecycleScope.launch {
            try {
                val config = GameConfig(
                    minBet = 10,
                    maxBet = 1000,
                    playerLimit = 4,
                    timeoutSeconds = 30,
                    allowSpectators = true
                )
                
                val gameHandle = bitcrapsNode.createGame(config)
                // Navigate to game screen
            } catch (e: Exception) {
                e.printStackTrace()
            }
        }
    }
    
    private fun joinGame() {
        // Show game selection dialog
    }
    
    private fun pollForEvents() {
        lifecycleScope.launch {
            while (isDiscovering) {
                try {
                    val events = bitcrapsNode.drainEvents()
                    events.forEach { event ->
                        handleEvent(event)
                    }
                    
                    // Update peer list
                    connectedPeers = bitcrapsNode.getConnectedPeers()
                    
                    // Wait before next poll
                    kotlinx.coroutines.delay(1000)
                } catch (e: Exception) {
                    e.printStackTrace()
                }
            }
        }
    }
    
    private fun handleEvent(event: GameEvent) {
        when (event) {
            is GameEvent.PeerDiscovered -> {
                // Handle peer discovered
            }
            is GameEvent.PeerConnected -> {
                // Handle peer connected
            }
            is GameEvent.GameCreated -> {
                // Handle game created
            }
            else -> {
                // Handle other events
            }
        }
    }
}