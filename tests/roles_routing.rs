use bitcraps::{AppConfig, NodeRole};

#[test]
fn test_appconfig_default_role_is_client() {
    let cfg = AppConfig::default();
    assert_eq!(cfg.role, NodeRole::Client);
}

#[test]
fn test_node_role_variants() {
    // Ensure enum is available and comparable
    let _v = NodeRole::Validator;
    let _g = NodeRole::Gateway;
    let _c = NodeRole::Client;

    assert_ne!(NodeRole::Validator, NodeRole::Gateway);
    assert_ne!(NodeRole::Gateway, NodeRole::Client);
}

