//! Cryptographic receipts for proof of work

#[derive(Debug, Clone)]
pub struct MiningReceipt;

#[derive(Debug, Clone)]
pub enum ReceiptType {
    Relay,
    Storage,
    Availability,
}
