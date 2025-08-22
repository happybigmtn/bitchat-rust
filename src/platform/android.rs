#[cfg(target_os = "android")]
pub mod android {
    use jni::JNIEnv;
    use jni::objects::{JClass, JString, JObject};
    use jni::sys::{jlong, jboolean};
    use crate::{BitCrapsApp, AppConfig, CrapTokens};
    
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
        // Convert Java strings to Rust
        let data_dir: String = env.get_string(data_dir)
            .expect("Invalid data_dir")
            .into();
        
        let nickname: String = env.get_string(nickname)
            .expect("Invalid nickname")
            .into();
        
        // Start BitCraps node
        let config = AppConfig {
            data_dir,
            nickname: Some(nickname),
            pow_difficulty: difficulty as u32,
            ..AppConfig::default()
        };
        
        // Return handle to app instance
        let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
        let app = rt.block_on(async {
            BitCrapsApp::new(config).await
        }).expect("Failed to create app");
        
        Box::into_raw(Box::new((rt, app))) as jlong
    }
    
    #[no_mangle]
    pub extern "C" fn Java_com_bitcraps_BitCrapsService_createGame(
        env: JNIEnv,
        _class: JClass,
        app_ptr: jlong,
        buy_in: jlong,
    ) -> JString {
        let (rt, app) = unsafe { &mut *(app_ptr as *mut (tokio::runtime::Runtime, BitCrapsApp)) };
        
        // Create game
        let game_id = rt.block_on(async {
            app.game_runtime.create_game(
                app.identity.peer_id,
                8,
                CrapTokens::new_unchecked(buy_in as u64),
            ).await
        }).expect("Failed to create game");
        
        // Return game ID as string
        env.new_string(format!("{:?}", game_id))
            .expect("Failed to create string")
    }
    
    #[no_mangle]
    pub extern "C" fn Java_com_bitcraps_BitCrapsService_joinGame(
        env: JNIEnv,
        _class: JClass,
        app_ptr: jlong,
        game_id: JString,
    ) -> jboolean {
        let (rt, app) = unsafe { &mut *(app_ptr as *mut (tokio::runtime::Runtime, BitCrapsApp)) };
        
        let game_id_str: String = env.get_string(game_id)
            .expect("Invalid game_id")
            .into();
        
        // Parse game ID from hex string
        let game_id_bytes = match hex::decode(game_id_str) {
            Ok(bytes) if bytes.len() == 16 => {
                let mut array = [0u8; 16];
                array.copy_from_slice(&bytes);
                array
            }
            _ => return false as jboolean,
        };
        
        // Join game
        let result = rt.block_on(async {
            app.game_runtime.join_game(game_id_bytes, app.identity.peer_id).await
        });
        
        result.is_ok() as jboolean
    }
    
    #[no_mangle]
    pub extern "C" fn Java_com_bitcraps_BitCrapsService_getBalance(
        env: JNIEnv,
        _class: JClass,
        app_ptr: jlong,
    ) -> jlong {
        let (rt, app) = unsafe { &mut *(app_ptr as *mut (tokio::runtime::Runtime, BitCrapsApp)) };
        
        let balance = rt.block_on(async {
            app.ledger.get_balance(&app.identity.peer_id).await
        });
        
        balance as jlong
    }
    
    #[no_mangle]
    pub extern "C" fn Java_com_bitcraps_BitCrapsService_stopNode(
        env: JNIEnv,
        _class: JClass,
        app_ptr: jlong,
    ) {
        let boxed = unsafe { Box::from_raw(app_ptr as *mut (tokio::runtime::Runtime, BitCrapsApp)) };
        // Automatically dropped
    }
}