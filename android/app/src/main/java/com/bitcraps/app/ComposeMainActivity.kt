package com.bitcraps.app

import android.Manifest
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.animation.*
import androidx.compose.animation.core.*
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.draw.scale
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.bitcraps.app.service.BitCrapsService
import com.bitcraps.app.ui.theme.BitCrapsTheme
import com.bitcraps.app.viewmodel.GameStateViewModel
import com.bitcraps.app.viewmodel.PermissionState
import com.bitcraps.app.viewmodel.GameState
import timber.log.Timber
import kotlin.math.cos
import kotlin.math.sin
import kotlin.random.Random

class ComposeMainActivity : ComponentActivity() {
    
    private val gameViewModel: GameStateViewModel by viewModels()
    
    private val bluetoothManager: BluetoothManager by lazy {
        getSystemService(Context.BLUETOOTH_SERVICE) as BluetoothManager
    }
    
    private val bluetoothAdapter: BluetoothAdapter? by lazy {
        bluetoothManager.adapter
    }
    
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        
        // Initialize logging
        if (BuildConfig.DEBUG) {
            Timber.plant(Timber.DebugTree())
        }
        
        setContent {
            BitCrapsTheme {
                BitCrapsApp(
                    gameViewModel = gameViewModel,
                    bluetoothAdapter = bluetoothAdapter,
                    onStartService = { startBitCrapsService() },
                    onStopService = { stopBitCrapsService() }
                )
            }
        }
    }
    
    private fun startBitCrapsService() {
        try {
            BitCrapsService.startService(this)
            gameViewModel.setServiceRunning(true)
        } catch (e: Exception) {
            Timber.e(e, "Error starting service")
            gameViewModel.showMessage("Failed to start service: ${e.message}")
        }
    }
    
    private fun stopBitCrapsService() {
        try {
            BitCrapsService.stopService(this)
            gameViewModel.setServiceRunning(false)
        } catch (e: Exception) {
            Timber.e(e, "Error stopping service")
            gameViewModel.showMessage("Failed to stop service: ${e.message}")
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BitCrapsApp(
    gameViewModel: GameStateViewModel,
    bluetoothAdapter: BluetoothAdapter?,
    onStartService: () -> Unit,
    onStopService: () -> Unit
) {
    val context = LocalContext.current
    val gameState by gameViewModel.gameState.collectAsStateWithLifecycle()
    val permissionState by gameViewModel.permissionState.collectAsStateWithLifecycle()
    val isServiceRunning by gameViewModel.isServiceRunning.collectAsStateWithLifecycle()
    
    val snackbarHostState = remember { SnackbarHostState() }
    
    // Permission launcher
    val permissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions()
    ) { permissions ->
        gameViewModel.handlePermissionResults(permissions)
    }
    
    // Bluetooth enable launcher
    val bluetoothEnableLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) { result ->
        if (result.resultCode == ComponentActivity.RESULT_OK) {
            gameViewModel.setBluetoothEnabled(true)
        }
    }
    
    // Check permissions and bluetooth on startup
    LaunchedEffect(Unit) {
        val requiredPermissions = getRequiredPermissions()
        val hasPermissions = requiredPermissions.all { permission ->
            ContextCompat.checkSelfPermission(context, permission) == PackageManager.PERMISSION_GRANTED
        }
        val hasBluetoothEnabled = bluetoothAdapter?.isEnabled == true
        
        gameViewModel.updatePermissionState(hasPermissions, hasBluetoothEnabled)
    }
    
    // Handle snackbar messages
    LaunchedEffect(gameState.message) {
        gameState.message?.let { message ->
            snackbarHostState.showSnackbar(message)
            gameViewModel.clearMessage()
        }
    }
    
    Scaffold(
        snackbarHost = { SnackbarHost(snackbarHostState) },
        topBar = {
            TopAppBar(
                title = { Text("BitCraps", fontWeight = FontWeight.Bold) },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.primaryContainer
                )
            )
        }
    ) { paddingValues ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues)
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            // Status Card
            StatusCard(
                permissionState = permissionState,
                isServiceRunning = isServiceRunning,
                gameState = gameState
            )
            
            // Permission and Bluetooth Controls
            PermissionControls(
                permissionState = permissionState,
                onRequestPermissions = {
                    val permissions = getRequiredPermissions().filter { permission ->
                        ContextCompat.checkSelfPermission(context, permission) != PackageManager.PERMISSION_GRANTED
                    }
                    if (permissions.isNotEmpty()) {
                        permissionLauncher.launch(permissions.toTypedArray())
                    }
                },
                onEnableBluetooth = {
                    if (bluetoothAdapter?.isEnabled != true) {
                        val enableBluetoothIntent = Intent(BluetoothAdapter.ACTION_REQUEST_ENABLE)
                        bluetoothEnableLauncher.launch(enableBluetoothIntent)
                    }
                },
                onOpenSettings = {
                    val intent = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                        data = Uri.fromParts("package", context.packageName, null)
                    }
                    context.startActivity(intent)
                }
            )
            
            // Service Controls
            ServiceControls(
                isServiceRunning = isServiceRunning,
                canStartService = permissionState.hasAllPermissions && permissionState.bluetoothEnabled,
                onStartService = onStartService,
                onStopService = onStopService
            )
            
            // Game Interface
            if (isServiceRunning) {
                GameInterface(
                    gameState = gameState,
                    onRollDice = { gameViewModel.rollDice() },
                    onPlaceBet = { amount -> gameViewModel.placeBet(amount) },
                    onCreateGame = { gameViewModel.createGame() },
                    onJoinGame = { gameViewModel.joinGame() }
                )
            }
        }
    }
}

@Composable
fun StatusCard(
    permissionState: PermissionState,
    isServiceRunning: Boolean,
    gameState: GameState
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant
        )
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text(
                text = "System Status",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold
            )
            
            StatusRow(
                label = "Permissions",
                isGood = permissionState.hasAllPermissions,
                icon = if (permissionState.hasAllPermissions) Icons.Default.CheckCircle else Icons.Default.Warning
            )
            
            StatusRow(
                label = "Bluetooth",
                isGood = permissionState.bluetoothEnabled,
                icon = if (permissionState.bluetoothEnabled) Icons.Default.Bluetooth else Icons.Default.BluetoothDisabled
            )
            
            StatusRow(
                label = "Service",
                isGood = isServiceRunning,
                icon = if (isServiceRunning) Icons.Default.PlayArrow else Icons.Default.Stop
            )
            
            if (gameState.isInGame) {
                StatusRow(
                    label = "Game Session",
                    isGood = true,
                    icon = Icons.Default.Casino
                )
                
                Text(
                    text = "Players: ${gameState.playerCount} | Peers: ${gameState.connectedPeers}",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

@Composable
fun StatusRow(
    label: String,
    isGood: Boolean,
    icon: androidx.compose.ui.graphics.vector.ImageVector
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null,
            tint = if (isGood) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.error,
            modifier = Modifier.size(20.dp)
        )
        Text(
            text = label,
            style = MaterialTheme.typography.bodyMedium,
            color = if (isGood) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.error
        )
    }
}

@Composable
fun PermissionControls(
    permissionState: PermissionState,
    onRequestPermissions: () -> Unit,
    onEnableBluetooth: () -> Unit,
    onOpenSettings: () -> Unit
) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text(
                text = "Setup Required",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold
            )
            
            if (!permissionState.hasAllPermissions) {
                Button(
                    onClick = onRequestPermissions,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Icon(Icons.Default.Security, contentDescription = null)
                    Spacer(Modifier.width(8.dp))
                    Text("Grant Permissions")
                }
            }
            
            if (!permissionState.bluetoothEnabled) {
                Button(
                    onClick = onEnableBluetooth,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Icon(Icons.Default.Bluetooth, contentDescription = null)
                    Spacer(Modifier.width(8.dp))
                    Text("Enable Bluetooth")
                }
            }
            
            OutlinedButton(
                onClick = onOpenSettings,
                modifier = Modifier.fillMaxWidth()
            ) {
                Icon(Icons.Default.Settings, contentDescription = null)
                Spacer(Modifier.width(8.dp))
                Text("Open Settings")
            }
        }
    }
}

@Composable
fun ServiceControls(
    isServiceRunning: Boolean,
    canStartService: Boolean,
    onStartService: () -> Unit,
    onStopService: () -> Unit
) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text(
                text = "Service Control",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold
            )
            
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Button(
                    onClick = onStartService,
                    enabled = !isServiceRunning && canStartService,
                    modifier = Modifier.weight(1f)
                ) {
                    Icon(Icons.Default.PlayArrow, contentDescription = null)
                    Spacer(Modifier.width(8.dp))
                    Text("Start Service")
                }
                
                Button(
                    onClick = onStopService,
                    enabled = isServiceRunning,
                    colors = ButtonDefaults.buttonColors(
                        containerColor = MaterialTheme.colorScheme.error
                    ),
                    modifier = Modifier.weight(1f)
                ) {
                    Icon(Icons.Default.Stop, contentDescription = null)
                    Spacer(Modifier.width(8.dp))
                    Text("Stop Service")
                }
            }
        }
    }
}

@Composable
fun GameInterface(
    gameState: GameState,
    onRollDice: () -> Unit,
    onPlaceBet: (Int) -> Unit,
    onCreateGame: () -> Unit,
    onJoinGame: () -> Unit
) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Text(
                text = "Game Interface",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold
            )
            
            if (!gameState.isInGame) {
                // Game lobby
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    Button(
                        onClick = onCreateGame,
                        modifier = Modifier.weight(1f)
                    ) {
                        Icon(Icons.Default.Add, contentDescription = null)
                        Spacer(Modifier.width(8.dp))
                        Text("Create Game")
                    }
                    
                    Button(
                        onClick = onJoinGame,
                        modifier = Modifier.weight(1f)
                    ) {
                        Icon(Icons.Default.Group, contentDescription = null)
                        Spacer(Modifier.width(8.dp))
                        Text("Join Game")
                    }
                }
            } else {
                // Active game interface
                DiceDisplay(
                    dice1 = gameState.dice1,
                    dice2 = gameState.dice2,
                    isRolling = gameState.isRolling
                )
                
                BettingInterface(
                    currentBet = gameState.currentBet,
                    balance = gameState.balance,
                    onPlaceBet = onPlaceBet
                )
                
                Button(
                    onClick = onRollDice,
                    enabled = !gameState.isRolling && gameState.canRoll,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Icon(Icons.Default.Casino, contentDescription = null)
                    Spacer(Modifier.width(8.dp))
                    Text(if (gameState.isRolling) "Rolling..." else "Roll Dice")
                }
            }
        }
    }
}

@Composable
fun DiceDisplay(
    dice1: Int,
    dice2: Int,
    isRolling: Boolean
) {
    val hapticFeedback = LocalHapticFeedback.current
    
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceEvenly
    ) {
        AnimatedDie(
            value = dice1,
            isRolling = isRolling,
            modifier = Modifier.size(80.dp)
        )
        
        AnimatedDie(
            value = dice2,
            isRolling = isRolling,
            modifier = Modifier.size(80.dp)
        )
    }
    
    LaunchedEffect(dice1, dice2) {
        if (dice1 > 0 && dice2 > 0 && !isRolling) {
            hapticFeedback.performHapticFeedback(HapticFeedbackType.LongPress)
        }
    }
    
    if (dice1 > 0 && dice2 > 0) {
        Text(
            text = "Total: ${dice1 + dice2}",
            style = MaterialTheme.typography.headlineMedium,
            fontWeight = FontWeight.Bold,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth()
        )
    }
}

@Composable
fun AnimatedDie(
    value: Int,
    isRolling: Boolean,
    modifier: Modifier = Modifier
) {
    val rotation by animateFloatAsState(
        targetValue = if (isRolling) 360f * 3 else 0f,
        animationSpec = if (isRolling) {
            infiniteRepeatable(
                animation = tween(1000, easing = LinearEasing),
                repeatMode = RepeatMode.Restart
            )
        } else {
            tween(300)
        },
        label = "DiceRotation"
    )
    
    val scale by animateFloatAsState(
        targetValue = if (isRolling) 1.2f else 1f,
        animationSpec = if (isRolling) {
            infiniteRepeatable(
                animation = tween(500, easing = LinearEasing),
                repeatMode = RepeatMode.Reverse
            )
        } else {
            spring()
        },
        label = "DiceScale"
    )
    
    Box(
        modifier = modifier
            .scale(scale)
            .rotate(rotation)
            .background(
                brush = Brush.radialGradient(
                    colors = listOf(
                        MaterialTheme.colorScheme.primary,
                        MaterialTheme.colorScheme.primaryContainer
                    )
                ),
                shape = RoundedCornerShape(12.dp)
            )
            .clip(RoundedCornerShape(12.dp)),
        contentAlignment = Alignment.Center
    ) {
        if (isRolling) {
            Text(
                text = "?",
                fontSize = 24.sp,
                fontWeight = FontWeight.Bold,
                color = MaterialTheme.colorScheme.onPrimary
            )
        } else if (value > 0) {
            Canvas(modifier = Modifier.fillMaxSize()) {
                drawDieFace(value, size.width, this)
            }
        }
    }
}

fun drawDieFace(value: Int, size: Float, drawScope: DrawScope) {
    val dotRadius = size * 0.08f
    val margin = size * 0.25f
    
    with(drawScope) {
        val positions = when (value) {
            1 -> listOf(Offset(size / 2, size / 2))
            2 -> listOf(
                Offset(margin, margin),
                Offset(size - margin, size - margin)
            )
            3 -> listOf(
                Offset(margin, margin),
                Offset(size / 2, size / 2),
                Offset(size - margin, size - margin)
            )
            4 -> listOf(
                Offset(margin, margin),
                Offset(size - margin, margin),
                Offset(margin, size - margin),
                Offset(size - margin, size - margin)
            )
            5 -> listOf(
                Offset(margin, margin),
                Offset(size - margin, margin),
                Offset(size / 2, size / 2),
                Offset(margin, size - margin),
                Offset(size - margin, size - margin)
            )
            6 -> listOf(
                Offset(margin, margin),
                Offset(size - margin, margin),
                Offset(margin, size / 2),
                Offset(size - margin, size / 2),
                Offset(margin, size - margin),
                Offset(size - margin, size - margin)
            )
            else -> emptyList()
        }
        
        positions.forEach { position ->
            drawCircle(
                color = Color.White,
                radius = dotRadius,
                center = position
            )
        }
    }
}

@Composable
fun BettingInterface(
    currentBet: Int,
    balance: Int,
    onPlaceBet: (Int) -> Unit
) {
    Column(
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text(
            text = "Balance: $balance chips",
            style = MaterialTheme.typography.bodyLarge,
            fontWeight = FontWeight.Bold
        )
        
        if (currentBet > 0) {
            Text(
                text = "Current Bet: $currentBet chips",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.primary
            )
        }
        
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            val betAmounts = listOf(10, 25, 50, 100)
            betAmounts.forEach { amount ->
                Button(
                    onClick = { onPlaceBet(amount) },
                    enabled = balance >= amount,
                    modifier = Modifier.weight(1f)
                ) {
                    Text("$amount")
                }
            }
        }
    }
}

fun getRequiredPermissions(): List<String> {
    return mutableListOf<String>().apply {
        add(Manifest.permission.BLUETOOTH)
        add(Manifest.permission.BLUETOOTH_ADMIN)
        
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            add(Manifest.permission.BLUETOOTH_SCAN)
            add(Manifest.permission.BLUETOOTH_ADVERTISE)
            add(Manifest.permission.BLUETOOTH_CONNECT)
        }
        
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) {
            add(Manifest.permission.ACCESS_FINE_LOCATION)
            add(Manifest.permission.ACCESS_COARSE_LOCATION)
        }
        
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            add(Manifest.permission.POST_NOTIFICATIONS)
        }
    }
}

@Preview(showBackground = true)
@Composable
fun BitCrapsAppPreview() {
    BitCrapsTheme {
        // Preview with mock data
    }
}