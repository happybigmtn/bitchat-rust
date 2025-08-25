package com.bitcraps.app.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import timber.log.Timber
import kotlin.random.Random

data class GameState(
    val isInGame: Boolean = false,
    val isRolling: Boolean = false,
    val dice1: Int = 0,
    val dice2: Int = 0,
    val currentBet: Int = 0,
    val balance: Int = 1000,
    val playerCount: Int = 1,
    val connectedPeers: Int = 0,
    val canRoll: Boolean = false,
    val gamePhase: GamePhase = GamePhase.COME_OUT,
    val point: Int? = null,
    val message: String? = null
)

data class PermissionState(
    val hasAllPermissions: Boolean = false,
    val bluetoothEnabled: Boolean = false
)

enum class GamePhase {
    COME_OUT,
    POINT,
    GAME_OVER
}

class GameStateViewModel : ViewModel() {
    
    private val _gameState = MutableStateFlow(GameState())
    val gameState: StateFlow<GameState> = _gameState.asStateFlow()
    
    private val _permissionState = MutableStateFlow(PermissionState())
    val permissionState: StateFlow<PermissionState> = _permissionState.asStateFlow()
    
    private val _isServiceRunning = MutableStateFlow(false)
    val isServiceRunning: StateFlow<Boolean> = _isServiceRunning.asStateFlow()
    
    fun updatePermissionState(hasPermissions: Boolean, bluetoothEnabled: Boolean) {
        _permissionState.value = PermissionState(
            hasAllPermissions = hasPermissions,
            bluetoothEnabled = bluetoothEnabled
        )
        Timber.d("Permission state updated: permissions=$hasPermissions, bluetooth=$bluetoothEnabled")
    }
    
    fun handlePermissionResults(permissions: Map<String, Boolean>) {
        val allGranted = permissions.values.all { it }
        _permissionState.value = _permissionState.value.copy(
            hasAllPermissions = allGranted
        )
        
        if (allGranted) {
            showMessage("All permissions granted")
        } else {
            val deniedPermissions = permissions.filterValues { !it }.keys
            showMessage("Some permissions were denied: ${deniedPermissions.joinToString(", ")}")
        }
    }
    
    fun setBluetoothEnabled(enabled: Boolean) {
        _permissionState.value = _permissionState.value.copy(bluetoothEnabled = enabled)
        if (enabled) {
            showMessage("Bluetooth enabled")
        }
    }
    
    fun setServiceRunning(running: Boolean) {
        _isServiceRunning.value = running
        if (running) {
            showMessage("BitCraps service started")
        } else {
            showMessage("BitCraps service stopped")
            // Reset game state when service stops
            _gameState.value = GameState()
        }
    }
    
    fun createGame() {
        if (!_isServiceRunning.value) {
            showMessage("Service must be running to create a game")
            return
        }
        
        viewModelScope.launch {
            try {
                showMessage("Creating game session...")
                delay(1000) // Simulate network delay
                
                _gameState.value = _gameState.value.copy(
                    isInGame = true,
                    canRoll = true,
                    playerCount = 1,
                    connectedPeers = 0,
                    gamePhase = GamePhase.COME_OUT
                )
                
                showMessage("Game created! You are the host.")
                Timber.d("Game created successfully")
                
            } catch (e: Exception) {
                Timber.e(e, "Failed to create game")
                showMessage("Failed to create game: ${e.message}")
            }
        }
    }
    
    fun joinGame() {
        if (!_isServiceRunning.value) {
            showMessage("Service must be running to join a game")
            return
        }
        
        viewModelScope.launch {
            try {
                showMessage("Looking for available games...")
                delay(2000) // Simulate discovery and connection
                
                _gameState.value = _gameState.value.copy(
                    isInGame = true,
                    canRoll = false, // Wait for turn
                    playerCount = 2,
                    connectedPeers = 1,
                    gamePhase = GamePhase.COME_OUT
                )
                
                showMessage("Joined game! Waiting for your turn.")
                Timber.d("Joined game successfully")
                
            } catch (e: Exception) {
                Timber.e(e, "Failed to join game")
                showMessage("Failed to join game: ${e.message}")
            }
        }
    }
    
    fun rollDice() {
        val currentState = _gameState.value
        if (!currentState.isInGame || currentState.isRolling || !currentState.canRoll) {
            return
        }
        
        viewModelScope.launch {
            try {
                _gameState.value = currentState.copy(isRolling = true)
                
                // Animate dice roll for 2 seconds
                delay(2000)
                
                val dice1 = Random.nextInt(1, 7)
                val dice2 = Random.nextInt(1, 7)
                val total = dice1 + dice2
                
                val newState = when (currentState.gamePhase) {
                    GamePhase.COME_OUT -> {
                        when (total) {
                            7, 11 -> {
                                // Natural win
                                val winAmount = currentState.currentBet * 2
                                showMessage("Natural! You win $winAmount chips!")
                                currentState.copy(
                                    isRolling = false,
                                    dice1 = dice1,
                                    dice2 = dice2,
                                    balance = currentState.balance + winAmount,
                                    currentBet = 0,
                                    gamePhase = GamePhase.COME_OUT
                                )
                            }
                            2, 3, 12 -> {
                                // Craps
                                showMessage("Craps! You lose ${currentState.currentBet} chips.")
                                currentState.copy(
                                    isRolling = false,
                                    dice1 = dice1,
                                    dice2 = dice2,
                                    currentBet = 0,
                                    gamePhase = GamePhase.COME_OUT
                                )
                            }
                            else -> {
                                // Point established
                                showMessage("Point is $total. Roll again!")
                                currentState.copy(
                                    isRolling = false,
                                    dice1 = dice1,
                                    dice2 = dice2,
                                    point = total,
                                    gamePhase = GamePhase.POINT
                                )
                            }
                        }
                    }
                    GamePhase.POINT -> {
                        when {
                            total == currentState.point -> {
                                // Point made
                                val winAmount = currentState.currentBet * 2
                                showMessage("Point made! You win $winAmount chips!")
                                currentState.copy(
                                    isRolling = false,
                                    dice1 = dice1,
                                    dice2 = dice2,
                                    balance = currentState.balance + winAmount,
                                    currentBet = 0,
                                    point = null,
                                    gamePhase = GamePhase.COME_OUT
                                )
                            }
                            total == 7 -> {
                                // Seven out
                                showMessage("Seven out! You lose ${currentState.currentBet} chips.")
                                currentState.copy(
                                    isRolling = false,
                                    dice1 = dice1,
                                    dice2 = dice2,
                                    currentBet = 0,
                                    point = null,
                                    gamePhase = GamePhase.COME_OUT
                                )
                            }
                            else -> {
                                // Keep rolling
                                showMessage("Roll again! Point is ${currentState.point}")
                                currentState.copy(
                                    isRolling = false,
                                    dice1 = dice1,
                                    dice2 = dice2
                                )
                            }
                        }
                    }
                    GamePhase.GAME_OVER -> currentState.copy(isRolling = false)
                }
                
                _gameState.value = newState
                Timber.d("Dice rolled: $dice1, $dice2 (total: $total)")
                
            } catch (e: Exception) {
                Timber.e(e, "Error rolling dice")
                _gameState.value = currentState.copy(isRolling = false)
                showMessage("Error rolling dice: ${e.message}")
            }
        }
    }
    
    fun placeBet(amount: Int) {
        val currentState = _gameState.value
        if (currentState.balance < amount) {
            showMessage("Insufficient balance")
            return
        }
        
        _gameState.value = currentState.copy(
            currentBet = amount,
            balance = currentState.balance - amount
        )
        
        showMessage("Placed bet: $amount chips")
        Timber.d("Bet placed: $amount chips")
    }
    
    fun showMessage(message: String) {
        _gameState.value = _gameState.value.copy(message = message)
    }
    
    fun clearMessage() {
        _gameState.value = _gameState.value.copy(message = null)
    }
    
    // Simulate peer connections for demo
    fun simulatePeerConnection() {
        viewModelScope.launch {
            delay(5000)
            val currentState = _gameState.value
            if (currentState.isInGame) {
                _gameState.value = currentState.copy(
                    connectedPeers = currentState.connectedPeers + 1,
                    playerCount = currentState.playerCount + 1
                )
                showMessage("New player joined the game!")
            }
        }
    }
    
    // Battery optimization detection
    fun checkBatteryOptimization() {
        // TODO: Implement battery optimization detection
        showMessage("Checking battery optimization settings...")
    }
    
    // Performance monitoring
    fun reportPerformanceMetrics() {
        viewModelScope.launch {
            // TODO: Implement performance monitoring
            Timber.d("Performance metrics: Game running smoothly")
        }
    }
}