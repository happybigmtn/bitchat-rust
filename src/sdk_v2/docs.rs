//! Documentation Generator
//!
//! Automated documentation generation system for the BitCraps SDK
//! with support for multiple formats and interactive examples.

use crate::sdk_v2::{
    error::{SDKError, SDKResult},
    rest::generate_openapi_spec,
    types::*,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

/// Documentation generator for SDK
#[derive(Debug)]
pub struct DocumentationGenerator {
    config: DocConfig,
    templates: HashMap<DocFormat, String>,
}

/// Documentation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocConfig {
    pub title: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub base_url: String,
    pub include_examples: bool,
    pub include_schemas: bool,
    pub theme: DocTheme,
}

/// Documentation themes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocTheme {
    Default,
    Dark,
    Minimal,
    Corporate,
}

/// Documentation formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocFormat {
    Html,
    Markdown,
    OpenApi,
    Postman,
    Insomnia,
}

/// Generated documentation structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Documentation {
    pub config: DocConfig,
    pub sections: Vec<DocSection>,
    pub examples: Vec<CodeExample>,
    pub schemas: Vec<SchemaDoc>,
}

/// Documentation section
#[derive(Debug, Serialize, Deserialize)]
pub struct DocSection {
    pub title: String,
    pub content: String,
    pub subsections: Vec<DocSection>,
    pub code_examples: Vec<CodeExample>,
}

/// Code example
#[derive(Debug, Serialize, Deserialize)]
pub struct CodeExample {
    pub title: String,
    pub description: String,
    pub language: String,
    pub code: String,
    pub output: Option<String>,
}

/// Schema documentation
#[derive(Debug, Serialize, Deserialize)]
pub struct SchemaDoc {
    pub name: String,
    pub description: String,
    pub properties: Vec<PropertyDoc>,
    pub example: serde_json::Value,
}

/// Property documentation
#[derive(Debug, Serialize, Deserialize)]
pub struct PropertyDoc {
    pub name: String,
    pub property_type: String,
    pub description: String,
    pub required: bool,
    pub example: Option<serde_json::Value>,
}

impl Default for DocConfig {
    fn default() -> Self {
        Self {
            title: "BitCraps SDK Documentation".to_string(),
            version: "2.0.0".to_string(),
            description: "Comprehensive SDK for the BitCraps gaming platform".to_string(),
            author: "BitCraps Team".to_string(),
            base_url: "https://api.bitcraps.com/v2".to_string(),
            include_examples: true,
            include_schemas: true,
            theme: DocTheme::Default,
        }
    }
}

impl DocumentationGenerator {
    /// Create a new documentation generator
    pub fn new(config: DocConfig) -> Self {
        let mut templates = HashMap::new();
        
        // HTML template
        templates.insert(DocFormat::Html, Self::html_template());
        
        // Markdown template
        templates.insert(DocFormat::Markdown, Self::markdown_template());
        
        Self { config, templates }
    }
    
    /// Generate complete documentation
    pub async fn generate(&self, output_dir: PathBuf, format: DocFormat) -> SDKResult<()> {
        // Create output directory
        fs::create_dir_all(&output_dir).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to create output directory: {}", e)))?;
        
        // Generate documentation content
        let documentation = self.build_documentation().await?;
        
        match format {
            DocFormat::Html => self.generate_html(&documentation, output_dir).await,
            DocFormat::Markdown => self.generate_markdown(&documentation, output_dir).await,
            DocFormat::OpenApi => self.generate_openapi(output_dir).await,
            DocFormat::Postman => self.generate_postman(output_dir).await,
            DocFormat::Insomnia => self.generate_insomnia(output_dir).await,
        }
    }
    
    /// Build documentation structure
    async fn build_documentation(&self) -> SDKResult<Documentation> {
        let mut sections = Vec::new();
        
        // Overview section
        sections.push(DocSection {
            title: "Overview".to_string(),
            content: format!(
                "{}\n\nThe BitCraps SDK provides a comprehensive set of APIs for building applications on the decentralized gaming platform.",
                self.config.description
            ),
            subsections: vec![
                DocSection {
                    title: "Features".to_string(),
                    content: "• High-level game management APIs\n• Real-time WebSocket communication\n• Consensus voting system\n• Peer-to-peer networking\n• Cross-platform support".to_string(),
                    subsections: vec![],
                    code_examples: vec![],
                },
                DocSection {
                    title: "Installation".to_string(),
                    content: "Add the SDK to your project dependencies:".to_string(),
                    subsections: vec![],
                    code_examples: vec![
                        CodeExample {
                            title: "Rust".to_string(),
                            description: "Add to Cargo.toml".to_string(),
                            language: "toml".to_string(),
                            code: r#"[dependencies]
bitcraps-sdk = "2.0""#.to_string(),
                            output: None,
                        },
                        CodeExample {
                            title: "Python".to_string(),
                            description: "Install via pip".to_string(),
                            language: "bash".to_string(),
                            code: "pip install bitcraps-sdk".to_string(),
                            output: None,
                        },
                    ],
                },
            ],
            code_examples: vec![],
        });
        
        // Getting Started section
        sections.push(DocSection {
            title: "Getting Started".to_string(),
            content: "Quick start guide to using the BitCraps SDK".to_string(),
            subsections: vec![],
            code_examples: vec![
                CodeExample {
                    title: "Basic Usage".to_string(),
                    description: "Initialize the SDK and create a game".to_string(),
                    language: "rust".to_string(),
                    code: r#"use bitcraps_sdk::BitCrapsSDK;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sdk = BitCrapsSDK::new(config).await?;
    
    // Create a new game
    let game = sdk.create_game("My Game")
        .with_max_players(8)
        .with_betting_limits(10, 1000)
        .build()
        .await?;
    
    println!("Created game: {}", game.id);
    Ok(())
}"#.to_string(),
                    output: Some("Created game: game_12345".to_string()),
                },
            ],
        });
        
        // API Reference section
        sections.push(DocSection {
            title: "API Reference".to_string(),
            content: "Comprehensive API documentation".to_string(),
            subsections: self.build_api_sections().await?,
            code_examples: vec![],
        });
        
        // Examples section
        let examples = if self.config.include_examples {
            self.build_examples().await?
        } else {
            vec![]
        };
        
        // Schemas section
        let schemas = if self.config.include_schemas {
            self.build_schemas().await?
        } else {
            vec![]
        };
        
        Ok(Documentation {
            config: self.config.clone(),
            sections,
            examples,
            schemas,
        })
    }
    
    /// Build API reference sections
    async fn build_api_sections(&self) -> SDKResult<Vec<DocSection>> {
        let mut sections = Vec::new();
        
        // Games API
        sections.push(DocSection {
            title: "Games API".to_string(),
            content: "Manage games, players, and game state".to_string(),
            subsections: vec![
                DocSection {
                    title: "Creating Games".to_string(),
                    content: "Create new game instances with customizable rules".to_string(),
                    subsections: vec![],
                    code_examples: vec![
                        CodeExample {
                            title: "Create Game".to_string(),
                            description: "Create a new craps game".to_string(),
                            language: "rust".to_string(),
                            code: r#"let game = sdk.create_game("High Stakes Craps")
    .game_type(GameType::Craps)
    .with_max_players(8)
    .with_betting_limits(100, 10000)
    .with_turn_timeout(60)
    .build()
    .await?;"#.to_string(),
                            output: None,
                        },
                    ],
                },
                DocSection {
                    title: "Joining Games".to_string(),
                    content: "Join existing games and manage player sessions".to_string(),
                    subsections: vec![],
                    code_examples: vec![
                        CodeExample {
                            title: "Join Game".to_string(),
                            description: "Join an existing game by ID".to_string(),
                            language: "rust".to_string(),
                            code: r#"let session = sdk.join_game(&game_id).await?;
println!("Joined game with session: {}", session.session_id);"#.to_string(),
                            output: None,
                        },
                    ],
                },
            ],
            code_examples: vec![],
        });
        
        // Consensus API
        sections.push(DocSection {
            title: "Consensus API".to_string(),
            content: "Distributed consensus and voting mechanisms".to_string(),
            subsections: vec![
                DocSection {
                    title: "Creating Proposals".to_string(),
                    content: "Submit proposals for consensus voting".to_string(),
                    subsections: vec![],
                    code_examples: vec![
                        CodeExample {
                            title: "Create Proposal".to_string(),
                            description: "Propose a game action for voting".to_string(),
                            language: "rust".to_string(),
                            code: r#"let proposal_id = sdk.consensus()
    .create_proposal(&game_id)
    .action(GameAction::PlaceBet { 
        bet_type: "pass_line".to_string(), 
        amount: 100 
    })
    .timeout(300)
    .submit()
    .await?;"#.to_string(),
                            output: None,
                        },
                    ],
                },
                DocSection {
                    title: "Voting".to_string(),
                    content: "Vote on consensus proposals".to_string(),
                    subsections: vec![],
                    code_examples: vec![
                        CodeExample {
                            title: "Submit Vote".to_string(),
                            description: "Vote on a consensus proposal".to_string(),
                            language: "rust".to_string(),
                            code: r#"sdk.consensus()
    .vote(&proposal_id, Vote::Approve)
    .await?;"#.to_string(),
                            output: None,
                        },
                    ],
                },
            ],
            code_examples: vec![],
        });
        
        // Network API
        sections.push(DocSection {
            title: "Network API".to_string(),
            content: "Peer-to-peer networking and connection management".to_string(),
            subsections: vec![
                DocSection {
                    title: "Peer Management".to_string(),
                    content: "Connect to and manage peer connections".to_string(),
                    subsections: vec![],
                    code_examples: vec![
                        CodeExample {
                            title: "Connect to Peer".to_string(),
                            description: "Establish connection to a peer".to_string(),
                            language: "rust".to_string(),
                            code: r#"let peer_id = sdk.network()
    .connect("192.168.1.100:8080")
    .await?;
println!("Connected to peer: {}", peer_id);"#.to_string(),
                            output: None,
                        },
                    ],
                },
            ],
            code_examples: vec![],
        });
        
        Ok(sections)
    }
    
    /// Build example code
    async fn build_examples(&self) -> SDKResult<Vec<CodeExample>> {
        Ok(vec![
            CodeExample {
                title: "Complete Game Flow".to_string(),
                description: "End-to-end example of creating and playing a game".to_string(),
                language: "rust".to_string(),
                code: r#"use bitcraps_sdk::{BitCrapsSDK, Config, Environment};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize SDK
    let config = Config::builder()
        .api_key("your-api-key")
        .environment(Environment::Production)
        .build()?;
    
    let sdk = BitCrapsSDK::new(config).await?;
    
    // Create a game
    let game = sdk.create_game("My Craps Game")
        .with_max_players(6)
        .with_betting_limits(10, 500)
        .build()
        .await?;
    
    println!("Created game: {} ({})", game.name, game.id);
    
    // Join the game
    let session = sdk.join_game(&game.id).await?;
    println!("Joined game with session: {}", session.session_id);
    
    // Place a bet
    let bet_result = sdk.games()
        .place_bet(&game.id, "pass_line".to_string(), 50)
        .await?;
    
    if bet_result.success {
        println!("Bet placed successfully! Remaining balance: {}", bet_result.remaining_balance);
    }
    
    Ok(())
}"#.to_string(),
                output: Some(r#"Created game: My Craps Game (game_12345)
Joined game with session: session_67890
Bet placed successfully! Remaining balance: 950"#.to_string()),
            },
            CodeExample {
                title: "WebSocket Real-time Updates".to_string(),
                description: "Subscribe to real-time game events".to_string(),
                language: "rust".to_string(),
                code: r#"// Subscribe to game events
let mut event_stream = sdk.subscribe::<GameUpdate>(EventType::GameStarted).await?;

// Handle events
tokio::spawn(async move {
    while let Some(event) = event_stream.next().await {
        match event {
            GameUpdate::PlayerJoined { player_id, .. } => {
                println!("Player joined: {}", player_id);
            }
            GameUpdate::BetPlaced { amount, .. } => {
                println!("Bet placed: ${}", amount);
            }
            GameUpdate::DiceRolled { dice1, dice2 } => {
                println!("Dice rolled: {} and {}", dice1, dice2);
            }
        }
    }
});"#.to_string(),
                output: Some(r#"Player joined: player_abc123
Bet placed: $100
Dice rolled: 4 and 3"#.to_string()),
            },
            CodeExample {
                title: "Error Handling".to_string(),
                description: "Proper error handling with recovery suggestions".to_string(),
                language: "rust".to_string(),
                code: r#"match sdk.join_game(&game_id).await {
    Ok(session) => println!("Joined successfully: {}", session.session_id),
    Err(e) => {
        eprintln!("Failed to join game: {}", e);
        eprintln!("Error code: {}", e.error_code());
        
        if e.is_retryable() {
            if let Some(delay) = e.retry_delay() {
                println!("Retrying in {:?}", delay);
                tokio::time::sleep(delay).await;
                // Retry logic here
            }
        }
        
        for suggestion in e.recovery_suggestions() {
            println!("Suggestion: {}", suggestion);
        }
    }
}"#.to_string(),
                output: Some(r#"Failed to join game: Game is full
Error code: GAME_ERROR
Suggestion: Try joining a different game
Suggestion: Create your own game
Suggestion: Wait for a player to leave"#.to_string()),
            },
        ])
    }
    
    /// Build schema documentation
    async fn build_schemas(&self) -> SDKResult<Vec<SchemaDoc>> {
        Ok(vec![
            SchemaDoc {
                name: "GameInfo".to_string(),
                description: "Information about a game instance".to_string(),
                properties: vec![
                    PropertyDoc {
                        name: "id".to_string(),
                        property_type: "String".to_string(),
                        description: "Unique game identifier".to_string(),
                        required: true,
                        example: Some(serde_json::json!("game_12345")),
                    },
                    PropertyDoc {
                        name: "name".to_string(),
                        property_type: "String".to_string(),
                        description: "Human-readable game name".to_string(),
                        required: true,
                        example: Some(serde_json::json!("High Stakes Craps")),
                    },
                    PropertyDoc {
                        name: "status".to_string(),
                        property_type: "GameStatus".to_string(),
                        description: "Current game status".to_string(),
                        required: true,
                        example: Some(serde_json::json!("InProgress")),
                    },
                    PropertyDoc {
                        name: "max_players".to_string(),
                        property_type: "u32".to_string(),
                        description: "Maximum number of players allowed".to_string(),
                        required: true,
                        example: Some(serde_json::json!(8)),
                    },
                ],
                example: serde_json::json!({
                    "id": "game_12345",
                    "name": "High Stakes Craps",
                    "game_type": "Craps",
                    "status": "InProgress",
                    "current_players": 5,
                    "max_players": 8,
                    "min_bet": 100,
                    "max_bet": 10000,
                    "created_at": "2024-01-01T12:00:00Z"
                }),
            },
            SchemaDoc {
                name: "GameSession".to_string(),
                description: "Player session in a game".to_string(),
                properties: vec![
                    PropertyDoc {
                        name: "session_id".to_string(),
                        property_type: "String".to_string(),
                        description: "Unique session identifier".to_string(),
                        required: true,
                        example: Some(serde_json::json!("session_67890")),
                    },
                    PropertyDoc {
                        name: "game_id".to_string(),
                        property_type: "String".to_string(),
                        description: "ID of the associated game".to_string(),
                        required: true,
                        example: Some(serde_json::json!("game_12345")),
                    },
                ],
                example: serde_json::json!({
                    "session_id": "session_67890",
                    "game_id": "game_12345",
                    "player_id": "player_abc123",
                    "joined_at": "2024-01-01T12:30:00Z",
                    "is_active": true
                }),
            },
        ])
    }
    
    /// Generate HTML documentation
    async fn generate_html(&self, doc: &Documentation, output_dir: PathBuf) -> SDKResult<()> {
        let template = self.templates.get(&DocFormat::Html).unwrap();
        
        // Generate main HTML file
        let html_content = self.render_html_template(template, doc)?;
        let html_path = output_dir.join("index.html");
        fs::write(html_path, html_content).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to write HTML: {}", e)))?;
        
        // Generate CSS file
        let css_content = self.generate_css();
        let css_path = output_dir.join("styles.css");
        fs::write(css_path, css_content).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to write CSS: {}", e)))?;
        
        // Generate JavaScript file
        let js_content = self.generate_javascript();
        let js_path = output_dir.join("script.js");
        fs::write(js_path, js_content).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to write JS: {}", e)))?;
        
        Ok(())
    }
    
    /// Generate Markdown documentation
    async fn generate_markdown(&self, doc: &Documentation, output_dir: PathBuf) -> SDKResult<()> {
        let mut content = String::new();
        
        // Title and description
        content.push_str(&format!("# {}\n\n", doc.config.title));
        content.push_str(&format!("{}\n\n", doc.config.description));
        content.push_str(&format!("Version: {}\n\n", doc.config.version));
        
        // Table of contents
        content.push_str("## Table of Contents\n\n");
        for section in &doc.sections {
            content.push_str(&format!("- [{}](#{})\n", section.title, self.slugify(&section.title)));
            for subsection in &section.subsections {
                content.push_str(&format!("  - [{}](#{})\n", subsection.title, self.slugify(&subsection.title)));
            }
        }
        content.push_str("\n");
        
        // Sections
        for section in &doc.sections {
            self.render_markdown_section(section, &mut content, 2);
        }
        
        // Examples
        if !doc.examples.is_empty() {
            content.push_str("## Examples\n\n");
            for example in &doc.examples {
                content.push_str(&format!("### {}\n\n", example.title));
                content.push_str(&format!("{}\n\n", example.description));
                content.push_str(&format!("```{}\n{}\n```\n\n", example.language, example.code));
                if let Some(output) = &example.output {
                    content.push_str("Output:\n");
                    content.push_str(&format!("```\n{}\n```\n\n", output));
                }
            }
        }
        
        // Schemas
        if !doc.schemas.is_empty() {
            content.push_str("## Schemas\n\n");
            for schema in &doc.schemas {
                content.push_str(&format!("### {}\n\n", schema.name));
                content.push_str(&format!("{}\n\n", schema.description));
                
                content.push_str("#### Properties\n\n");
                content.push_str("| Name | Type | Required | Description |\n");
                content.push_str("|------|------|----------|-------------|\n");
                for prop in &schema.properties {
                    content.push_str(&format!(
                        "| {} | {} | {} | {} |\n",
                        prop.name,
                        prop.property_type,
                        if prop.required { "✓" } else { "" },
                        prop.description
                    ));
                }
                content.push_str("\n");
                
                content.push_str("#### Example\n\n");
                content.push_str(&format!("```json\n{}\n```\n\n", 
                    serde_json::to_string_pretty(&schema.example).unwrap()));
            }
        }
        
        let md_path = output_dir.join("README.md");
        fs::write(md_path, content).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to write Markdown: {}", e)))?;
        
        Ok(())
    }
    
    /// Generate OpenAPI specification
    async fn generate_openapi(&self, output_dir: PathBuf) -> SDKResult<()> {
        let spec = generate_openapi_spec();
        let yaml_content = serde_yaml::to_string(&spec)
            .map_err(|e| SDKError::SerializationError(e.to_string()))?;
        
        let openapi_path = output_dir.join("openapi.yaml");
        fs::write(openapi_path, yaml_content).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to write OpenAPI spec: {}", e)))?;
        
        Ok(())
    }
    
    /// Generate Postman collection
    async fn generate_postman(&self, output_dir: PathBuf) -> SDKResult<()> {
        let collection = serde_json::json!({
            "info": {
                "name": self.config.title,
                "description": self.config.description,
                "version": self.config.version,
                "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
            },
            "item": [
                {
                    "name": "Games",
                    "item": [
                        {
                            "name": "List Games",
                            "request": {
                                "method": "GET",
                                "header": [
                                    {
                                        "key": "Authorization",
                                        "value": "Bearer {{api_key}}"
                                    }
                                ],
                                "url": {
                                    "raw": "{{base_url}}/games",
                                    "host": ["{{base_url}}"],
                                    "path": ["games"]
                                }
                            }
                        },
                        {
                            "name": "Create Game",
                            "request": {
                                "method": "POST",
                                "header": [
                                    {
                                        "key": "Authorization",
                                        "value": "Bearer {{api_key}}"
                                    },
                                    {
                                        "key": "Content-Type",
                                        "value": "application/json"
                                    }
                                ],
                                "body": {
                                    "mode": "raw",
                                    "raw": json!({
                                        "name": "My Craps Game",
                                        "gameType": "Craps",
                                        "maxPlayers": 8,
                                        "minBet": 10,
                                        "maxBet": 1000
                                    }).to_string()
                                },
                                "url": {
                                    "raw": "{{base_url}}/games",
                                    "host": ["{{base_url}}"],
                                    "path": ["games"]
                                }
                            }
                        }
                    ]
                }
            ],
            "variable": [
                {
                    "key": "base_url",
                    "value": self.config.base_url
                },
                {
                    "key": "api_key",
                    "value": "your-api-key-here"
                }
            ]
        });
        
        let postman_path = output_dir.join("bitcraps-api.postman_collection.json");
        let json_content = serde_json::to_string_pretty(&collection)?;
        fs::write(postman_path, json_content).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to write Postman collection: {}", e)))?;
        
        Ok(())
    }
    
    /// Generate Insomnia collection
    async fn generate_insomnia(&self, output_dir: PathBuf) -> SDKResult<()> {
        let collection = serde_json::json!({
            "_type": "export",
            "__export_format": 4,
            "resources": [
                {
                    "_id": "env_base",
                    "_type": "environment",
                    "name": "Base Environment",
                    "data": {
                        "base_url": self.config.base_url,
                        "api_key": "your-api-key-here"
                    }
                },
                {
                    "_id": "req_list_games",
                    "_type": "request",
                    "name": "List Games",
                    "method": "GET",
                    "url": "{{ _.base_url }}/games",
                    "headers": [
                        {
                            "name": "Authorization",
                            "value": "Bearer {{ _.api_key }}"
                        }
                    ]
                },
                {
                    "_id": "req_create_game",
                    "_type": "request", 
                    "name": "Create Game",
                    "method": "POST",
                    "url": "{{ _.base_url }}/games",
                    "headers": [
                        {
                            "name": "Authorization",
                            "value": "Bearer {{ _.api_key }}"
                        },
                        {
                            "name": "Content-Type",
                            "value": "application/json"
                        }
                    ],
                    "body": {
                        "mimeType": "application/json",
                        "text": json!({
                            "name": "My Craps Game",
                            "gameType": "Craps", 
                            "maxPlayers": 8,
                            "minBet": 10,
                            "maxBet": 1000
                        }).to_string()
                    }
                }
            ]
        });
        
        let insomnia_path = output_dir.join("bitcraps-api.insomnia.json");
        let json_content = serde_json::to_string_pretty(&collection)?;
        fs::write(insomnia_path, json_content).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to write Insomnia collection: {}", e)))?;
        
        Ok(())
    }
    
    /// Helper methods
    
    fn render_html_template(&self, template: &str, doc: &Documentation) -> SDKResult<String> {
        // Simple template rendering (in production would use a proper template engine)
        let mut html = template.replace("{{title}}", &doc.config.title);
        html = html.replace("{{description}}", &doc.config.description);
        html = html.replace("{{version}}", &doc.config.version);
        
        // Render sections
        let mut sections_html = String::new();
        for section in &doc.sections {
            sections_html.push_str(&format!(
                "<section><h2>{}</h2><p>{}</p></section>",
                section.title, section.content
            ));
        }
        html = html.replace("{{content}}", &sections_html);
        
        Ok(html)
    }
    
    fn render_markdown_section(&self, section: &DocSection, content: &mut String, level: usize) {
        let heading = "#".repeat(level);
        content.push_str(&format!("{} {}\n\n", heading, section.title));
        content.push_str(&format!("{}\n\n", section.content));
        
        for example in &section.code_examples {
            content.push_str(&format!("### {}\n\n", example.title));
            content.push_str(&format!("{}\n\n", example.description));
            content.push_str(&format!("```{}\n{}\n```\n\n", example.language, example.code));
        }
        
        for subsection in &section.subsections {
            self.render_markdown_section(subsection, content, level + 1);
        }
    }
    
    fn slugify(&self, text: &str) -> String {
        text.to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .trim_matches('-')
            .to_string()
    }
    
    fn html_template() -> String {
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{title}}</title>
    <link rel="stylesheet" href="styles.css">
</head>
<body>
    <header>
        <h1>{{title}}</h1>
        <p>{{description}}</p>
        <span class="version">Version {{version}}</span>
    </header>
    
    <nav>
        <ul>
            <li><a href="#overview">Overview</a></li>
            <li><a href="#getting-started">Getting Started</a></li>
            <li><a href="#api-reference">API Reference</a></li>
            <li><a href="#examples">Examples</a></li>
        </ul>
    </nav>
    
    <main>
        {{content}}
    </main>
    
    <footer>
        <p>Generated by BitCraps SDK Documentation Generator</p>
    </footer>
    
    <script src="script.js"></script>
</body>
</html>"#.to_string()
    }
    
    fn markdown_template() -> String {
        "# {{title}}\n\n{{description}}\n\n{{content}}".to_string()
    }
    
    fn generate_css(&self) -> String {
        r#"/* BitCraps SDK Documentation Styles */
body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    line-height: 1.6;
    color: #333;
    max-width: 1200px;
    margin: 0 auto;
    padding: 20px;
    background-color: #f9f9f9;
}

header {
    text-align: center;
    margin-bottom: 40px;
    padding: 40px 0;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    color: white;
    border-radius: 10px;
}

nav ul {
    list-style: none;
    padding: 0;
    display: flex;
    justify-content: center;
    gap: 30px;
    margin: 30px 0;
}

nav a {
    text-decoration: none;
    color: #667eea;
    font-weight: bold;
    padding: 10px 20px;
    border-radius: 5px;
    transition: background-color 0.3s;
}

nav a:hover {
    background-color: #667eea;
    color: white;
}

section {
    background: white;
    padding: 30px;
    margin: 20px 0;
    border-radius: 10px;
    box-shadow: 0 2px 10px rgba(0,0,0,0.1);
}

h1, h2, h3 {
    color: #333;
}

pre {
    background-color: #f4f4f4;
    padding: 20px;
    border-radius: 5px;
    overflow-x: auto;
    border-left: 4px solid #667eea;
}

code {
    background-color: #f4f4f4;
    padding: 2px 6px;
    border-radius: 3px;
    font-family: 'Monaco', 'Consolas', monospace;
}

.version {
    background-color: rgba(255,255,255,0.2);
    padding: 5px 15px;
    border-radius: 20px;
    font-size: 14px;
}

footer {
    text-align: center;
    margin-top: 50px;
    padding-top: 20px;
    border-top: 1px solid #eee;
    color: #666;
}"#.to_string()
    }
    
    fn generate_javascript(&self) -> String {
        r#"// BitCraps SDK Documentation Interactive Features
document.addEventListener('DOMContentLoaded', function() {
    // Smooth scrolling for navigation links
    document.querySelectorAll('nav a[href^="#"]').forEach(anchor => {
        anchor.addEventListener('click', function (e) {
            e.preventDefault();
            document.querySelector(this.getAttribute('href')).scrollIntoView({
                behavior: 'smooth'
            });
        });
    });
    
    // Copy code functionality
    document.querySelectorAll('pre code').forEach(block => {
        const button = document.createElement('button');
        button.innerHTML = 'Copy';
        button.className = 'copy-button';
        button.addEventListener('click', () => {
            navigator.clipboard.writeText(block.textContent);
            button.innerHTML = 'Copied!';
            setTimeout(() => button.innerHTML = 'Copy', 2000);
        });
        block.parentNode.appendChild(button);
    });
    
    console.log('BitCraps SDK Documentation loaded');
});"#.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_doc_config_default() {
        let config = DocConfig::default();
        assert_eq!(config.title, "BitCraps SDK Documentation");
        assert!(config.include_examples);
        assert!(config.include_schemas);
    }
    
    #[tokio::test]
    async fn test_documentation_generator() {
        let config = DocConfig::default();
        let generator = DocumentationGenerator::new(config);
        
        let doc = generator.build_documentation().await.unwrap();
        assert!(!doc.sections.is_empty());
        assert!(!doc.examples.is_empty());
        assert!(!doc.schemas.is_empty());
    }
    
    #[test]
    fn test_slugify() {
        let generator = DocumentationGenerator::new(DocConfig::default());
        assert_eq!(generator.slugify("Getting Started"), "getting-started");
        assert_eq!(generator.slugify("API Reference"), "api-reference");
    }
}