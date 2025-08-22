//! Widgets module for BitCraps UI
//! 
//! This module implements the user interface components for BitCraps
//! including CLI, TUI, and specialized casino widgets.

use serde::{Serialize, Deserialize};
use std::time::SystemTime;
use crate::PeerId;
use ratatui::text::{Span, Line};
use ratatui::style::{Style, Color, Modifier};
use ratatui::widgets::{List, ListItem, Block, Borders, StatefulWidget, ListState};
use ratatui::layout::Rect;
use ratatui::buffer::Buffer;

// Add missing types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageFilter {
    All,
    From(String),
    Channel(String),
    System,
}

#[allow(dead_code)]
pub struct AutoComplete {
    commands: Vec<String>,
    peers: Vec<String>,
    channels: Vec<String>,
}

impl AutoComplete {
    pub fn complete(&self, input: &str) -> Vec<String> {
        if input.starts_with('/') {
            self.complete_command(input)
        } else {
            self.complete_command(input)
        }
    }
    
    fn complete_command(&self, input: &str) -> Vec<String> {
        self.commands
            .iter()
            .filter(|cmd| cmd.starts_with(input))
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub sender: PeerId,
    pub content: MessageContent,
    pub timestamp: SystemTime,
    pub channel: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageContent {
    Text(String),
    File(String), // Simplified file transfer as filename
    System(SystemMessage),
    Encrypted(String), // Simplified encrypted content as string
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemMessage {
    PeerJoined(String),
    PeerLeft(String),
    ChannelCreated(String),
    Error(String),
}

pub struct MessageFormatter;

impl MessageFormatter {
    pub fn format_message(msg: &ChatMessage) -> Vec<Span<'_>> {
        let timestamp = format!("{:?}", msg.timestamp);
        let sender = format!("{:?}", msg.sender);
        
        match &msg.content {
            MessageContent::Text(text) => {
                vec![
                    Span::styled(timestamp, Style::default().fg(Color::Gray)),
                    Span::raw(" "),
                    Span::styled(sender, Style::default().fg(Color::Cyan)),
                    Span::raw(": "),
                    Span::raw(text.clone()),
                ]
            }
            MessageContent::File(filename) => {
                vec![
                    Span::styled(timestamp, Style::default().fg(Color::Gray)),
                    Span::raw(" "),
                    Span::styled(sender, Style::default().fg(Color::Cyan)),
                    Span::raw(": "),
                    Span::styled(format!("📁 {}", filename), Style::default().fg(Color::Green)),
                ]
            }
            MessageContent::System(sys_msg) => {
                vec![
                    Span::styled(timestamp, Style::default().fg(Color::Gray)),
                    Span::raw(" "),
                    Span::styled(format!("* {:?}", sys_msg), Style::default().fg(Color::Yellow)),
                ]
            }
            MessageContent::Encrypted(content) => {
                vec![
                    Span::styled(timestamp, Style::default().fg(Color::Gray)),
                    Span::raw(" "),
                    Span::styled(sender, Style::default().fg(Color::Cyan)),
                    Span::raw(": "),
                    Span::styled(format!("🔒 {}", content), Style::default().fg(Color::Magenta)),
                ]
            }
        }
    }
    
    pub fn format_for_export(messages: &[ChatMessage]) -> String {
        messages
            .iter()
            .map(|msg| {
                format!(
                    "[{}] {}: {:?}",
                    format!("{:?}", msg.timestamp),
                    format!("{:?}", msg.sender),
                    msg.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[allow(dead_code)]
pub struct ChatView {
    messages: Vec<ChatMessage>,
    scroll_offset: usize,
    filter: MessageFilter,
    state: ListState,
}

impl ChatView {
    pub fn get_visible_messages(&self) -> Vec<&ChatMessage> {
        let start = self.scroll_offset;
        let end = std::cmp::min(start + 50, self.messages.len());
        self.messages[start..end].iter().collect()
    }
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            scroll_offset: 0,
            filter: MessageFilter::All,
            state: ListState::default(),
        }
    }
    
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let start = self.scroll_offset;
        let end = std::cmp::min(start + 50, self.messages.len());
        
        let items: Vec<ListItem> = self.messages[start..end]
            .iter()
            .map(|msg| {
                let spans = MessageFormatter::format_message(msg);
                ListItem::new(Line::from(spans))
            })
            .collect();
            
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Chat"))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));
            
        StatefulWidget::render(list, area, buf, &mut self.state);
    }
    
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }
    
    pub fn scroll_down(&mut self) {
        self.scroll_offset = (self.scroll_offset + 1).min(self.messages.len());
    }
}

