use bitcraps::token::TokenLedger;

#[tokio::test]
async fn test_token_ledger_creation() {
    let ledger = TokenLedger::new();
    let peer_id = [1u8; 32];
    let balance = ledger.get_balance(&peer_id).await;
    assert_eq!(balance, 0);
}

#[tokio::test]
async fn test_account_creation() {
    let ledger = TokenLedger::new();
    let alice = [1u8; 32];

    // Create account
    ledger.create_account(alice).await.unwrap();

    // Check initial balance is 0
    assert_eq!(ledger.get_balance(&alice).await, 0);
}
