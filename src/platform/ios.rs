#[cfg(target_os = "ios")]
pub mod ios {
    use std::ffi::{CStr, CString};
    use std::os::raw::c_char;
    use crate::{BitCrapsApp, AppConfig, CrapTokens};
    
    /// Objective-C bridge for iOS
    /// 
    /// Feynman: iOS speaks Objective-C with a funny accent (Swift).
    /// We need to learn their language to run our casino on iPhones.
    #[no_mangle]
    pub extern "C" fn bitcraps_start_node(
        data_dir: *const c_char,
        nickname: *const c_char,
        difficulty: i32,
    ) -> *mut (tokio::runtime::Runtime, BitCrapsApp) {
        // Convert C strings to Rust
        let data_dir = unsafe {
            CStr::from_ptr(data_dir)
                .to_string_lossy()
                .into_owned()
        };
        
        let nickname = unsafe {
            CStr::from_ptr(nickname)
                .to_string_lossy()
                .into_owned()
        };
        
        let config = AppConfig {
            data_dir,
            nickname: Some(nickname),
            pow_difficulty: difficulty as u32,
            ..AppConfig::default()
        };
        
        // Create runtime and app
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(_) => return std::ptr::null_mut(),
        };
        
        let app = match rt.block_on(async {
            BitCrapsApp::new(config).await
        }) {
            Ok(app) => app,
            Err(_) => return std::ptr::null_mut(),
        };
        
        Box::into_raw(Box::new((rt, app)))
    }
    
    #[no_mangle]
    pub extern "C" fn bitcraps_create_game(
        app_ptr: *mut (tokio::runtime::Runtime, BitCrapsApp),
        buy_in: u64,
    ) -> *mut c_char {
        if app_ptr.is_null() {
            return std::ptr::null_mut();
        }
        
        let (rt, app) = unsafe { &mut *app_ptr };
        
        let game_id = match rt.block_on(async {
            app.game_runtime.create_game(
                app.identity.peer_id,
                8,
                CrapTokens::new_unchecked(buy_in * 1_000_000),
            ).await
        }) {
            Ok(id) => id,
            Err(_) => return std::ptr::null_mut(),
        };
        
        let game_id_str = format!("{:?}", game_id);
        match CString::new(game_id_str) {
            Ok(c_string) => c_string.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    }
    
    #[no_mangle]
    pub extern "C" fn bitcraps_join_game(
        app_ptr: *mut (tokio::runtime::Runtime, BitCrapsApp),
        game_id: *const c_char,
    ) -> bool {
        if app_ptr.is_null() || game_id.is_null() {
            return false;
        }
        
        let (rt, app) = unsafe { &mut *app_ptr };
        
        let game_id_str = unsafe {
            CStr::from_ptr(game_id)
                .to_string_lossy()
                .into_owned()
        };
        
        // Parse game ID from hex string
        let game_id_bytes = match hex::decode(game_id_str) {
            Ok(bytes) if bytes.len() == 16 => {
                let mut array = [0u8; 16];
                array.copy_from_slice(&bytes);
                array
            }
            _ => return false,
        };
        
        // Join game
        rt.block_on(async {
            app.game_runtime.join_game(game_id_bytes, app.identity.peer_id).await
        }).is_ok()
    }
    
    #[no_mangle]
    pub extern "C" fn bitcraps_get_balance(
        app_ptr: *mut (tokio::runtime::Runtime, BitCrapsApp),
    ) -> u64 {
        if app_ptr.is_null() {
            return 0;
        }
        
        let (rt, app) = unsafe { &mut *app_ptr };
        
        rt.block_on(async {
            app.ledger.get_balance(&app.identity.peer_id).await
        })
    }
    
    #[no_mangle]
    pub extern "C" fn bitcraps_get_peer_id(
        app_ptr: *mut (tokio::runtime::Runtime, BitCrapsApp),
    ) -> *mut c_char {
        if app_ptr.is_null() {
            return std::ptr::null_mut();
        }
        
        let (_, app) = unsafe { &*app_ptr };
        
        let peer_id_str = format!("{:?}", app.identity.peer_id);
        match CString::new(peer_id_str) {
            Ok(c_string) => c_string.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    }
    
    #[no_mangle]
    pub extern "C" fn bitcraps_start_main_loop(
        app_ptr: *mut (tokio::runtime::Runtime, BitCrapsApp),
    ) -> bool {
        if app_ptr.is_null() {
            return false;
        }
        
        let (rt, app) = unsafe { &mut *app_ptr };
        
        rt.block_on(async {
            app.start().await
        }).is_ok()
    }
    
    #[no_mangle]
    pub extern "C" fn bitcraps_stop_node(
        app_ptr: *mut (tokio::runtime::Runtime, BitCrapsApp),
    ) {
        if !app_ptr.is_null() {
            let _ = unsafe { Box::from_raw(app_ptr) };
        }
    }
    
    #[no_mangle]
    pub extern "C" fn bitcraps_free_string(s: *mut c_char) {
        if !s.is_null() {
            unsafe {
                let _ = CString::from_raw(s);
            }
        }
    }
}