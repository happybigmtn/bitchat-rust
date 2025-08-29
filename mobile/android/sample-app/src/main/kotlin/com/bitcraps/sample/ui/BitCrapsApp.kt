package com.bitcraps.sample.ui

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.bitcraps.sample.ui.screens.*
import com.bitcraps.sample.viewmodel.GameViewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BitCrapsApp(
    gameViewModel: GameViewModel,
    navController: NavHostController = rememberNavController()
) {
    val uiState by gameViewModel.uiState.collectAsState()
    
    Scaffold(
        topBar = {
            BitCrapsTopBar(
                title = getCurrentScreenTitle(navController),
                uiState = uiState,
                onSettingsClick = { navController.navigate("settings") }
            )
        },
        bottomBar = {
            if (shouldShowBottomBar(navController)) {
                BitCrapsBottomBar(
                    navController = navController,
                    uiState = uiState
                )
            }
        }
    ) { paddingValues ->
        NavHost(
            navController = navController,
            startDestination = if (uiState.isSDKInitialized) "home" else "loading",
            modifier = Modifier.padding(paddingValues)
        ) {
            // Loading screen
            composable("loading") {
                LoadingScreen(
                    uiState = uiState,
                    onRetry = { gameViewModel.retrySDKInitialization() }
                )
            }
            
            // Main screens
            composable("home") {
                HomeScreen(
                    uiState = uiState,
                    onStartDiscovery = { gameViewModel.startDiscovery() },
                    onStopDiscovery = { gameViewModel.stopDiscovery() },
                    onCreateGame = { gameViewModel.createGame() },
                    onJoinGame = { gameId -> gameViewModel.joinGame(gameId) },
                    onConnectToPeer = { peerId -> gameViewModel.connectToPeer(peerId) }
                )
            }
            
            composable("peers") {
                PeersScreen(
                    uiState = uiState,
                    onConnectToPeer = { peerId -> gameViewModel.connectToPeer(peerId) },
                    onDisconnectFromPeer = { peerId -> gameViewModel.disconnectFromPeer(peerId) },
                    onSendMessage = { peerId, message -> gameViewModel.sendMessage(peerId, message) }
                )
            }
            
            composable("game") {
                GameScreen(
                    uiState = uiState,
                    onPlaceBet = { amount, betType -> gameViewModel.placeBet(amount, betType) },
                    onRollDice = { gameViewModel.rollDice() },
                    onLeaveGame = { gameViewModel.leaveGame() }
                )
            }
            
            composable("stats") {
                StatsScreen(
                    uiState = uiState,
                    onRefreshStats = { gameViewModel.refreshStats() },
                    onRunDiagnostics = { gameViewModel.runDiagnostics() }
                )
            }
            
            composable("settings") {
                SettingsScreen(
                    uiState = uiState,
                    onPowerModeChanged = { powerMode -> gameViewModel.setPowerMode(powerMode) },
                    onBluetoothConfigChanged = { config -> gameViewModel.setBluetoothConfig(config) },
                    onBiometricToggled = { enabled -> gameViewModel.setBiometricEnabled(enabled) },
                    onResetNode = { gameViewModel.resetNode() },
                    onExportHistory = { gameViewModel.exportHistory() },
                    onImportHistory = { data -> gameViewModel.importHistory(data) }
                )
            }
        }
    }
    
    // Handle navigation based on state changes
    LaunchedEffect(uiState.isSDKInitialized, uiState.currentGameId) {
        when {
            !uiState.isSDKInitialized && navController.currentDestination?.route != "loading" -> {
                navController.navigate("loading") {
                    popUpTo(0) { inclusive = true }
                }
            }
            uiState.isSDKInitialized && navController.currentDestination?.route == "loading" -> {
                navController.navigate("home") {
                    popUpTo("loading") { inclusive = true }
                }
            }
            uiState.currentGameId != null && navController.currentDestination?.route != "game" -> {
                navController.navigate("game")
            }
            uiState.currentGameId == null && navController.currentDestination?.route == "game" -> {
                navController.popBackStack()
            }
        }
    }
}

@Composable
private fun getCurrentScreenTitle(navController: NavHostController): String {
    return when (navController.currentDestination?.route) {
        "loading" -> "BitCraps"
        "home" -> "BitCraps"
        "peers" -> "Peers"
        "game" -> "Game"
        "stats" -> "Statistics"
        "settings" -> "Settings"
        else -> "BitCraps"
    }
}

@Composable
private fun shouldShowBottomBar(navController: NavHostController): Boolean {
    return when (navController.currentDestination?.route) {
        "loading" -> false
        "settings" -> false
        else -> true
    }
}