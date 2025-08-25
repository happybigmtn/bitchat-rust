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
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import androidx.lifecycle.lifecycleScope
import com.bitcraps.app.databinding.ActivityMainBinding
import com.bitcraps.app.service.BitCrapsService
import com.google.android.material.snackbar.Snackbar
import kotlinx.coroutines.*
import timber.log.Timber

class MainActivity : AppCompatActivity() {
    
    private lateinit var binding: ActivityMainBinding
    
    private val bluetoothManager: BluetoothManager by lazy {
        getSystemService(Context.BLUETOOTH_SERVICE) as BluetoothManager
    }
    
    private val bluetoothAdapter: BluetoothAdapter? by lazy {
        bluetoothManager.adapter
    }
    
    // Permission request launcher
    private val permissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions()
    ) { permissions ->
        handlePermissionResults(permissions)
    }
    
    // Bluetooth enable request launcher
    private val bluetoothEnableLauncher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) { result ->
        if (result.resultCode == RESULT_OK) {
            Timber.d("Bluetooth enabled")
            checkAllRequirements()
        } else {
            showBluetoothRequiredDialog()
        }
    }
    
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        
        // Initialize Timber logging
        if (BuildConfig.DEBUG) {
            Timber.plant(Timber.DebugTree())
        }
        
        binding = ActivityMainBinding.inflate(layoutInflater)
        setContentView(binding.root)
        
        setupUI()
        checkAllRequirements()
    }
    
    override fun onResume() {
        super.onResume()
        updateServiceStatus()
        updateUI()
    }
    
    private fun setupUI() {
        binding.apply {
            // Service control buttons
            buttonStartService.setOnClickListener {
                if (checkAllRequirements()) {
                    startBitCrapsService()
                }
            }
            
            buttonStopService.setOnClickListener {
                stopBitCrapsService()
            }
            
            // Game control buttons
            buttonCreateGame.setOnClickListener {
                createNewGame()
            }
            
            buttonJoinGame.setOnClickListener {
                joinGame()
            }
            
            // Settings and permissions
            buttonPermissions.setOnClickListener {
                requestAllPermissions()
            }
            
            buttonSettings.setOnClickListener {
                openAppSettings()
            }
        }
    }
    
    private fun checkAllRequirements(): Boolean {
        val hasPermissions = checkPermissions()
        val hasBluetoothEnabled = checkBluetoothEnabled()
        
        return hasPermissions && hasBluetoothEnabled
    }
    
    private fun checkPermissions(): Boolean {
        val requiredPermissions = getRequiredPermissions()
        val missingPermissions = requiredPermissions.filter { permission ->
            ContextCompat.checkSelfPermission(this, permission) != PackageManager.PERMISSION_GRANTED
        }
        
        if (missingPermissions.isNotEmpty()) {
            Timber.d("Missing permissions: %s", missingPermissions)
            return false
        }
        
        return true
    }
    
    private fun checkBluetoothEnabled(): Boolean {
        if (bluetoothAdapter == null) {
            showMessage("Bluetooth not supported on this device")
            return false
        }
        
        if (bluetoothAdapter?.isEnabled != true) {
            promptEnableBluetooth()
            return false
        }
        
        return true
    }
    
    private fun getRequiredPermissions(): List<String> {
        return mutableListOf<String>().apply {
            // Basic BLE permissions
            add(Manifest.permission.BLUETOOTH)
            add(Manifest.permission.BLUETOOTH_ADMIN)
            
            // Android 12+ runtime permissions
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                add(Manifest.permission.BLUETOOTH_SCAN)
                add(Manifest.permission.BLUETOOTH_ADVERTISE)
                add(Manifest.permission.BLUETOOTH_CONNECT)
            }
            
            // Location permissions for older Android versions
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) {
                add(Manifest.permission.ACCESS_FINE_LOCATION)
                add(Manifest.permission.ACCESS_COARSE_LOCATION)
            }
            
            // Notification permission for Android 13+
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                add(Manifest.permission.POST_NOTIFICATIONS)
            }
        }
    }
    
    private fun requestAllPermissions() {
        val requiredPermissions = getRequiredPermissions()
        val missingPermissions = requiredPermissions.filter { permission ->
            ContextCompat.checkSelfPermission(this, permission) != PackageManager.PERMISSION_GRANTED
        }
        
        if (missingPermissions.isNotEmpty()) {
            Timber.d("Requesting permissions: %s", missingPermissions)
            permissionLauncher.launch(missingPermissions.toTypedArray())
        } else {
            showMessage("All permissions already granted")
            updateUI()
        }
    }
    
    private fun handlePermissionResults(permissions: Map<String, Boolean>) {
        val deniedPermissions = permissions.filterValues { !it }.keys
        
        if (deniedPermissions.isEmpty()) {
            showMessage("All permissions granted")
            checkAllRequirements()
        } else {
            Timber.w("Permissions denied: %s", deniedPermissions)
            
            val criticalPermissions = deniedPermissions.filter { permission ->
                when (permission) {
                    Manifest.permission.BLUETOOTH,
                    Manifest.permission.BLUETOOTH_ADMIN,
                    Manifest.permission.BLUETOOTH_SCAN,
                    Manifest.permission.BLUETOOTH_ADVERTISE -> true
                    else -> false
                }
            }
            
            if (criticalPermissions.isNotEmpty()) {
                showPermissionRequiredDialog(criticalPermissions)
            } else {
                showMessage("Some optional permissions were denied")
            }
        }
        
        updateUI()
    }
    
    private fun promptEnableBluetooth() {
        if (bluetoothAdapter?.isEnabled != true) {
            val enableBluetoothIntent = Intent(BluetoothAdapter.ACTION_REQUEST_ENABLE)
            bluetoothEnableLauncher.launch(enableBluetoothIntent)
        }
    }
    
    private fun showBluetoothRequiredDialog() {
        AlertDialog.Builder(this)
            .setTitle("Bluetooth Required")
            .setMessage("BitCraps requires Bluetooth to connect with other players. Please enable Bluetooth to continue.")
            .setPositiveButton("Enable") { _, _ ->
                promptEnableBluetooth()
            }
            .setNegativeButton("Cancel") { _, _ ->
                showMessage("Bluetooth is required for BitCraps to function")
            }
            .setCancelable(false)
            .show()
    }
    
    private fun showPermissionRequiredDialog(permissions: Set<String>) {
        val permissionNames = permissions.joinToString(", ") { permission ->
            when (permission) {
                Manifest.permission.BLUETOOTH -> "Bluetooth"
                Manifest.permission.BLUETOOTH_ADMIN -> "Bluetooth Admin"
                Manifest.permission.BLUETOOTH_SCAN -> "Bluetooth Scan"
                Manifest.permission.BLUETOOTH_ADVERTISE -> "Bluetooth Advertise"
                Manifest.permission.ACCESS_FINE_LOCATION -> "Location"
                else -> permission.substringAfterLast(".")
            }
        }
        
        AlertDialog.Builder(this)
            .setTitle("Permissions Required")
            .setMessage("BitCraps requires the following permissions to function properly: $permissionNames")
            .setPositiveButton("Grant") { _, _ ->
                requestAllPermissions()
            }
            .setNeutralButton("Settings") { _, _ ->
                openAppSettings()
            }
            .setNegativeButton("Cancel", null)
            .show()
    }
    
    private fun openAppSettings() {
        val intent = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
            data = Uri.fromParts("package", packageName, null)
        }
        startActivity(intent)
    }
    
    private fun startBitCrapsService() {
        try {
            BitCrapsService.startService(this)
            showMessage("Starting BitCraps service...")
            
            // Update UI after a short delay to allow service to start
            lifecycleScope.launch {
                delay(1000)
                updateServiceStatus()
            }
        } catch (e: Exception) {
            Timber.e(e, "Error starting service")
            showMessage("Failed to start service: ${e.message}")
        }
    }
    
    private fun stopBitCrapsService() {
        try {
            BitCrapsService.stopService(this)
            showMessage("Stopping BitCraps service...")
            
            // Update UI after a short delay to allow service to stop
            lifecycleScope.launch {
                delay(1000)
                updateServiceStatus()
            }
        } catch (e: Exception) {
            Timber.e(e, "Error stopping service")
            showMessage("Failed to stop service: ${e.message}")
        }
    }
    
    private fun createNewGame() {
        // TODO: Implement game creation dialog
        showMessage("Game creation not yet implemented")
    }
    
    private fun joinGame() {
        // TODO: Implement game join dialog
        showMessage("Game joining not yet implemented")
    }
    
    private fun updateServiceStatus() {
        // TODO: Check actual service status
        // For now, just update UI based on requirements
        updateUI()
    }
    
    private fun updateUI() {
        val hasPermissions = checkPermissions()
        val hasBluetoothEnabled = checkBluetoothEnabled()
        val allRequirementsMet = hasPermissions && hasBluetoothEnabled
        
        binding.apply {
            // Update status text
            textServiceStatus.text = when {
                !hasBluetoothEnabled -> "Bluetooth disabled"
                !hasPermissions -> "Permissions required"
                allRequirementsMet -> "Ready to start"
                else -> "Requirements not met"
            }
            
            // Update button states
            buttonStartService.isEnabled = allRequirementsMet
            buttonStopService.isEnabled = false // TODO: Check actual service state
            buttonCreateGame.isEnabled = allRequirementsMet
            buttonJoinGame.isEnabled = allRequirementsMet
            
            // Update permission button
            buttonPermissions.text = if (hasPermissions) {
                "Permissions Granted"
            } else {
                "Grant Permissions"
            }
            
            // Update status indicators
            imageBluetoothStatus.setImageResource(
                if (hasBluetoothEnabled) R.drawable.ic_bluetooth_connected
                else R.drawable.ic_bluetooth_disabled
            )
            
            imagePermissionStatus.setImageResource(
                if (hasPermissions) R.drawable.ic_check_circle
                else R.drawable.ic_warning
            )
        }
    }
    
    private fun showMessage(message: String) {
        Snackbar.make(binding.root, message, Snackbar.LENGTH_SHORT).show()
        Timber.d("UI Message: %s", message)
    }
}