package com.bitcraps.sample.ui

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.background
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.bitcraps.sample.viewmodel.GameViewModel
import kotlinx.coroutines.launch

/**
 * Home Screen - Main dashboard for BitCraps Android app
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(
    gameViewModel: GameViewModel = viewModel(),
    onNavigateToGame: (String) -> Unit,
    onNavigateToPeers: () -> Unit,
    onNavigateToSettings: () -> Unit
) {
    val uiState by gameViewModel.uiState.collectAsState()
    val scope = rememberCoroutineScope()
    var showCreateGameDialog by remember { mutableStateOf(false) }
    var isRefreshing by remember { mutableStateOf(false) }
    
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("BitCraps", fontWeight = FontWeight.Bold) },
                actions = {
                    IconButton(onClick = {
                        scope.launch {
                            isRefreshing = true
                            gameViewModel.discoverGames()
                            isRefreshing = false
                        }
                    }) {
                        Icon(
                            Icons.Default.Refresh,
                            contentDescription = "Refresh",
                            tint = if (isRefreshing) MaterialTheme.colorScheme.primary 
                                   else MaterialTheme.colorScheme.onSurface
                        )
                    }
                    
                    IconButton(onClick = onNavigateToSettings) {
                        Icon(Icons.Default.Settings, contentDescription = "Settings")
                    }
                }
            )
        },
        floatingActionButton = {
            ExtendedFloatingActionButton(
                onClick = { showCreateGameDialog = true },
                icon = { Icon(Icons.Default.Add, contentDescription = null) },
                text = { Text("Create Game") }
            )
        }
    ) { paddingValues ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues),
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            // Balance Card
            item {
                BalanceCard(balance = uiState.balance)
            }
            
            // Network Status
            item {
                NetworkStatusCard(
                    status = uiState.networkStatus,
                    connectedPeers = uiState.connectedPeers
                )
            }
            
            // Quick Actions
            item {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(16.dp)
                ) {
                    QuickActionCard(
                        modifier = Modifier.weight(1f),
                        title = "Join Game",
                        icon = Icons.Default.PlayArrow,
                        color = MaterialTheme.colorScheme.primary,
                        onClick = { /* Handle join */ }
                    )
                    
                    QuickActionCard(
                        modifier = Modifier.weight(1f),
                        title = "View Peers",
                        icon = Icons.Default.People,
                        color = MaterialTheme.colorScheme.secondary,
                        onClick = onNavigateToPeers
                    )
                }
            }
            
            // Active Games Section
            if (uiState.activeGames.isNotEmpty()) {
                item {
                    Text(
                        "Active Games",
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.Bold
                    )
                }
                
                items(uiState.activeGames) { game ->
                    ActiveGameCard(
                        game = game,
                        onResume = { onNavigateToGame(game.id) }
                    )
                }
            }
            
            // Discovered Games Section
            item {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        "Nearby Games",
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.Bold
                    )
                    
                    Text(
                        "${uiState.discoveredGames.size} found",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
            
            if (uiState.discoveredGames.isEmpty()) {
                item {
                    EmptyGamesCard()
                }
            } else {
                items(uiState.discoveredGames) { game ->
                    DiscoveredGameCard(
                        game = game,
                        onJoin = {
                            scope.launch {
                                gameViewModel.joinGame(game.id)
                                onNavigateToGame(game.id)
                            }
                        }
                    )
                }
            }
        }
    }
    
    // Create Game Dialog
    if (showCreateGameDialog) {
        CreateGameDialog(
            onDismiss = { showCreateGameDialog = false },
            onCreate = { minPlayers, ante ->
                scope.launch {
                    val gameId = gameViewModel.createGame(minPlayers, ante)
                    gameId?.let { onNavigateToGame(it) }
                    showCreateGameDialog = false
                }
            }
        )
    }
}

@Composable
fun BalanceCard(balance: Long) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.primaryContainer
        )
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(20.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column {
                Text(
                    "Balance",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onPrimaryContainer.copy(alpha = 0.7f)
                )
                Row(
                    verticalAlignment = Alignment.Bottom,
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    Text(
                        "$balance",
                        style = MaterialTheme.typography.headlineLarge,
                        fontWeight = FontWeight.Bold,
                        color = MaterialTheme.colorScheme.onPrimaryContainer
                    )
                    Text(
                        "CRAP",
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onPrimaryContainer.copy(alpha = 0.7f),
                        modifier = Modifier.padding(bottom = 4.dp)
                    )
                }
            }
            
            Icon(
                Icons.Default.AccountBalanceWallet,
                contentDescription = null,
                modifier = Modifier.size(48.dp),
                tint = MaterialTheme.colorScheme.onPrimaryContainer.copy(alpha = 0.5f)
            )
        }
    }
}

@Composable
fun NetworkStatusCard(
    status: NetworkStatus,
    connectedPeers: Int
) {
    val statusColor = when (status) {
        NetworkStatus.CONNECTED -> Color.Green
        NetworkStatus.CONNECTING -> Color.Yellow
        NetworkStatus.DISCONNECTED -> Color.Red
    }
    
    val statusText = when (status) {
        NetworkStatus.CONNECTED -> "Connected"
        NetworkStatus.CONNECTING -> "Connecting..."
        NetworkStatus.DISCONNECTED -> "Disconnected"
    }
    
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant
        )
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Row(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Box(
                    modifier = Modifier
                        .size(8.dp)
                        .clip(CircleShape)
                        .background(statusColor)
                )
                Text(
                    "Network: $statusText",
                    style = MaterialTheme.typography.bodyMedium
                )
            }
            
            Text(
                "$connectedPeers peers",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun QuickActionCard(
    modifier: Modifier = Modifier,
    title: String,
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    color: Color,
    onClick: () -> Unit
) {
    Card(
        modifier = modifier,
        onClick = onClick
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Icon(
                icon,
                contentDescription = null,
                modifier = Modifier.size(32.dp),
                tint = color
            )
            Text(
                title,
                style = MaterialTheme.typography.bodyMedium
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ActiveGameCard(
    game: GameInfo,
    onResume: () -> Unit
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        onClick = onResume
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column {
                Text(
                    "Game #${game.id.take(8)}",
                    style = MaterialTheme.typography.bodyLarge,
                    fontWeight = FontWeight.Medium
                )
                Text(
                    "${game.participants} players • ${game.phase}",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            
            Button(
                onClick = onResume,
                modifier = Modifier.height(36.dp)
            ) {
                Text("Resume", fontSize = 12.sp)
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DiscoveredGameCard(
    game: DiscoveredGame,
    onJoin: () -> Unit
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .animateContentSize()
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Row(
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Box(
                    modifier = Modifier
                        .size(8.dp)
                        .clip(CircleShape)
                        .background(
                            if (game.isJoinable) Color.Green else Color.Red
                        )
                )
                
                Column {
                    Text(
                        game.hostName,
                        style = MaterialTheme.typography.bodyLarge,
                        fontWeight = FontWeight.Medium
                    )
                    Text(
                        "${game.currentPlayers}/${game.maxPlayers} players • Ante: ${game.ante}",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
            
            if (game.isJoinable) {
                OutlinedButton(
                    onClick = onJoin,
                    modifier = Modifier.height(36.dp)
                ) {
                    Text("Join", fontSize = 12.sp)
                }
            }
        }
    }
}

@Composable
fun EmptyGamesCard() {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant
        )
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(32.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Icon(
                Icons.Default.Casino,
                contentDescription = null,
                modifier = Modifier.size(48.dp),
                tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f)
            )
            Text(
                "No games found",
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Text(
                "Create a new game or wait for others",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f)
            )
        }
    }
}

@Composable
fun CreateGameDialog(
    onDismiss: () -> Unit,
    onCreate: (minPlayers: Int, ante: Long) -> Unit
) {
    var minPlayers by remember { mutableStateOf("2") }
    var ante by remember { mutableStateOf("100") }
    
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Create New Game") },
        text = {
            Column(
                verticalArrangement = Arrangement.spacedBy(16.dp)
            ) {
                OutlinedTextField(
                    value = minPlayers,
                    onValueChange = { minPlayers = it },
                    label = { Text("Minimum Players") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
                
                OutlinedTextField(
                    value = ante,
                    onValueChange = { ante = it },
                    label = { Text("Ante (CRAP)") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    val players = minPlayers.toIntOrNull() ?: 2
                    val anteAmount = ante.toLongOrNull() ?: 100
                    onCreate(players, anteAmount)
                }
            ) {
                Text("Create")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel")
            }
        }
    )
}

// Data classes for UI state
data class GameInfo(
    val id: String,
    val participants: Int,
    val phase: String
)

data class DiscoveredGame(
    val id: String,
    val hostName: String,
    val currentPlayers: Int,
    val maxPlayers: Int,
    val ante: Long,
    val isJoinable: Boolean
)

enum class NetworkStatus {
    CONNECTED, CONNECTING, DISCONNECTED
}