//! Chat module for BitCraps UI
//!
//! This module implements the user interface components for BitCraps
//! including CLI, TUI, and specialized casino widgets.

use crate::PeerId;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Widget};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs::File;
use uuid::Uuid;

// Add missing types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChunk {
    pub index: u32,
    pub data: Vec<u8>,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferError {
    pub message: String,
}

impl From<std::io::Error> for TransferError {
    fn from(err: std::io::Error) -> Self {
        TransferError {
            message: err.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOffer {
    pub transfer_id: String,
    pub filename: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAccept {
    pub transfer_id: String,
}

#[derive(Debug, Clone)]
pub struct FileTransfer {
    pub id: String,
    pub filename: String,
    pub size: u64,
    pub hash: String,
    pub chunks: Vec<FileChunk>,
    pub progress: f32,
    pub status: TransferStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransferStatus {
    Pending,
    Transferring,
    Completed,
    Failed(String),
    Cancelled,
}

#[allow(dead_code)]
pub struct FileTransferManager {
    active_transfers: HashMap<String, FileTransfer>,
    download_dir: PathBuf,
    chunk_size: usize,
    network: NetworkManager,
}

pub struct NetworkManager;
impl NetworkManager {
    pub async fn send_message(
        &self,
        _peer: PeerId,
        _message: Message,
    ) -> Result<(), TransferError> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    FileOffer(FileOffer),
}

async fn calculate_file_hash(_filepath: &PathBuf) -> Result<String, TransferError> {
    Ok("dummy_hash".to_string())
}

impl FileTransferManager {
    pub async fn send_file(
        &mut self,
        peer: PeerId,
        filepath: PathBuf,
    ) -> Result<String, TransferError> {
        let file = File::open(&filepath).await?;
        let metadata = file.metadata().await?;
        let filename = filepath
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown_file".to_string());

        let transfer = FileTransfer {
            id: Uuid::new_v4().to_string(),
            filename,
            size: metadata.len(),
            hash: calculate_file_hash(&filepath).await?,
            chunks: Vec::new(),
            progress: 0.0,
            status: TransferStatus::Pending,
        };

        // Send file offer to peer
        let offer = FileOffer {
            transfer_id: transfer.id.clone(),
            filename: transfer.filename.clone(),
            size: transfer.size,
        };

        self.network
            .send_message(peer, Message::FileOffer(offer))
            .await?;
        self.active_transfers
            .insert(transfer.id.clone(), transfer.clone());

        Ok(transfer.id)
    }

    pub async fn accept_file(&mut self, transfer_id: &str) -> Result<(), TransferError> {
        if let Some(transfer) = self.active_transfers.get_mut(transfer_id) {
            transfer.status = TransferStatus::Transferring;

            // Send acceptance and start receiving chunks
            let _accept = FileAccept {
                transfer_id: transfer_id.to_string(),
            };
            // Send to appropriate peer...
        }

        Ok(())
    }

    pub fn get_transfer_progress(&self, transfer_id: &str) -> Option<f32> {
        self.active_transfers.get(transfer_id).map(|t| t.progress)
    }
}

pub struct FileTransferWidget {
    transfers: Vec<FileTransfer>,
}

impl FileTransferWidget {
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let items: Vec<ListItem> = self
            .transfers
            .iter()
            .map(|transfer| {
                let progress_bar = format!(
                    "[{}{}] {:.1}%",
                    "=".repeat((transfer.progress * 20.0) as usize),
                    " ".repeat(20 - (transfer.progress * 20.0) as usize),
                    transfer.progress * 100.0
                );

                let line = Line::from(vec![
                    Span::raw(&transfer.filename),
                    Span::raw(" "),
                    Span::styled(progress_bar, Style::default().fg(Color::Green)),
                    Span::raw(" "),
                    Span::styled(
                        format!("{:?}", transfer.status),
                        Style::default().fg(Color::Yellow),
                    ),
                ]);

                ListItem::new(line)
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title("File Transfers"),
        );

        Widget::render(list, area, buf);
    }
}
