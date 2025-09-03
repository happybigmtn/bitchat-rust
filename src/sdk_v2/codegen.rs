//! Code Generation Module
//!
//! Generates client libraries for multiple programming languages
//! based on the BitCraps API specification.

use crate::sdk_v2::{
    error::{SDKError, SDKResult},
    cli::TargetLanguage,
    rest::generate_openapi_spec,
};
use std::path::PathBuf;
use std::collections::HashMap;
use tokio::fs;

/// Code generator for multiple programming languages
#[derive(Debug)]
pub struct CodeGenerator {
    templates: HashMap<TargetLanguage, LanguageTemplate>,
}

/// Language-specific template configuration
#[derive(Debug, Clone)]
struct LanguageTemplate {
    pub extension: String,
    pub package_file: String,
    pub client_template: String,
    pub model_template: String,
    pub example_template: String,
}

impl CodeGenerator {
    /// Create a new code generator
    pub fn new() -> Self {
        let mut templates = HashMap::new();
        
        // Rust template
        templates.insert(TargetLanguage::Rust, LanguageTemplate {
            extension: "rs".to_string(),
            package_file: "Cargo.toml".to_string(),
            client_template: include_str!("templates/rust_client.rs.template").to_string(),
            model_template: include_str!("templates/rust_models.rs.template").to_string(),
            example_template: include_str!("templates/rust_example.rs.template").to_string(),
        });
        
        // Python template
        templates.insert(TargetLanguage::Python, LanguageTemplate {
            extension: "py".to_string(),
            package_file: "setup.py".to_string(),
            client_template: include_str!("templates/python_client.py.template").to_string(),
            model_template: include_str!("templates/python_models.py.template").to_string(),
            example_template: include_str!("templates/python_example.py.template").to_string(),
        });
        
        // TypeScript template
        templates.insert(TargetLanguage::TypeScript, LanguageTemplate {
            extension: "ts".to_string(),
            package_file: "package.json".to_string(),
            client_template: include_str!("templates/typescript_client.ts.template").to_string(),
            model_template: include_str!("templates/typescript_models.ts.template").to_string(),
            example_template: include_str!("templates/typescript_example.ts.template").to_string(),
        });
        
        Self { templates }
    }
    
    /// Generate client code for a specific language
    pub async fn generate_client_code(
        &self,
        language: TargetLanguage,
        output_dir: PathBuf,
        include_examples: bool,
    ) -> SDKResult<()> {
        let template = self.templates.get(&language)
            .ok_or_else(|| SDKError::FeatureNotSupportedError {
                feature: format!("Code generation for {:?}", language),
                environment: "current".to_string(),
            })?;
        
        // Create output directory
        fs::create_dir_all(&output_dir).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to create output directory: {}", e)))?;
        
        // Generate OpenAPI spec for reference
        let openapi_spec = generate_openapi_spec();
        
        // Generate client code
        match language {
            TargetLanguage::Rust => self.generate_rust_client(&output_dir, template, &openapi_spec, include_examples).await?,
            TargetLanguage::Python => self.generate_python_client(&output_dir, template, &openapi_spec, include_examples).await?,
            TargetLanguage::TypeScript => self.generate_typescript_client(&output_dir, template, &openapi_spec, include_examples).await?,
            _ => {
                return Err(SDKError::FeatureNotSupportedError {
                    feature: format!("Code generation for {:?}", language),
                    environment: "current".to_string(),
                });
            }
        }
        
        Ok(())
    }
    
    /// Generate Rust client code
    async fn generate_rust_client(
        &self,
        output_dir: &PathBuf,
        template: &LanguageTemplate,
        _openapi_spec: &crate::sdk_v2::rest::OpenAPISpec,
        include_examples: bool,
    ) -> SDKResult<()> {
        // Generate Cargo.toml
        let cargo_toml = r#"[package]
name = "bitcraps-client"
version = "2.0.0"
edition = "2021"
description = "BitCraps API client for Rust"
authors = ["BitCraps Team <api@bitcraps.com>"]
license = "MIT"

[dependencies]
tokio = { version = "1.0", features = ["full"] }
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.0", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1.0"

[dev-dependencies]
tokio-test = "0.4"
"#;
        
        let cargo_path = output_dir.join("Cargo.toml");
        fs::write(cargo_path, cargo_toml).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to write Cargo.toml: {}", e)))?;
        
        // Generate client code
        let client_code = self.generate_rust_client_code();
        let src_dir = output_dir.join("src");
        fs::create_dir_all(&src_dir).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to create src directory: {}", e)))?;
        
        let client_path = src_dir.join("lib.rs");
        fs::write(client_path, client_code).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to write client code: {}", e)))?;
        
        // Generate models
        let models_code = self.generate_rust_models_code();
        let models_path = src_dir.join("models.rs");
        fs::write(models_path, models_code).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to write models: {}", e)))?;
        
        // Generate examples if requested
        if include_examples {
            let examples_dir = output_dir.join("examples");
            fs::create_dir_all(&examples_dir).await
                .map_err(|e| SDKError::ConfigurationError(format!("Failed to create examples directory: {}", e)))?;
            
            let example_code = self.generate_rust_example_code();
            let example_path = examples_dir.join("basic_usage.rs");
            fs::write(example_path, example_code).await
                .map_err(|e| SDKError::ConfigurationError(format!("Failed to write example: {}", e)))?;
        }
        
        Ok(())
    }
    
    /// Generate Python client code
    async fn generate_python_client(
        &self,
        output_dir: &PathBuf,
        template: &LanguageTemplate,
        _openapi_spec: &crate::sdk_v2::rest::OpenAPISpec,
        include_examples: bool,
    ) -> SDKResult<()> {
        // Generate setup.py
        let setup_py = r#"from setuptools import setup, find_packages

setup(
    name="bitcraps-client",
    version="2.0.0",
    description="BitCraps API client for Python",
    author="BitCraps Team",
    author_email="api@bitcraps.com",
    packages=find_packages(),
    install_requires=[
        "aiohttp>=3.8.0",
        "pydantic>=1.10.0",
        "typing-extensions>=4.0.0",
    ],
    python_requires=">=3.8",
    classifiers=[
        "Development Status :: 4 - Beta",
        "Intended Audience :: Developers",
        "License :: OSI Approved :: MIT License",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
    ],
)
"#;
        
        let setup_path = output_dir.join("setup.py");
        fs::write(setup_path, setup_py).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to write setup.py: {}", e)))?;
        
        // Generate client code
        let package_dir = output_dir.join("bitcraps_client");
        fs::create_dir_all(&package_dir).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to create package directory: {}", e)))?;
        
        let client_code = self.generate_python_client_code();
        let client_path = package_dir.join("__init__.py");
        fs::write(client_path, client_code).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to write client code: {}", e)))?;
        
        // Generate models
        let models_code = self.generate_python_models_code();
        let models_path = package_dir.join("models.py");
        fs::write(models_path, models_code).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to write models: {}", e)))?;
        
        // Generate examples if requested
        if include_examples {
            let examples_dir = output_dir.join("examples");
            fs::create_dir_all(&examples_dir).await
                .map_err(|e| SDKError::ConfigurationError(format!("Failed to create examples directory: {}", e)))?;
            
            let example_code = self.generate_python_example_code();
            let example_path = examples_dir.join("basic_usage.py");
            fs::write(example_path, example_code).await
                .map_err(|e| SDKError::ConfigurationError(format!("Failed to write example: {}", e)))?;
        }
        
        Ok(())
    }
    
    /// Generate TypeScript client code
    async fn generate_typescript_client(
        &self,
        output_dir: &PathBuf,
        template: &LanguageTemplate,
        _openapi_spec: &crate::sdk_v2::rest::OpenAPISpec,
        include_examples: bool,
    ) -> SDKResult<()> {
        // Generate package.json
        let package_json = serde_json::json!({
            "name": "bitcraps-client",
            "version": "2.0.0",
            "description": "BitCraps API client for TypeScript/JavaScript",
            "main": "dist/index.js",
            "types": "dist/index.d.ts",
            "scripts": {
                "build": "tsc",
                "test": "jest",
                "prepublish": "npm run build"
            },
            "dependencies": {
                "axios": "^1.4.0",
                "ws": "^8.13.0"
            },
            "devDependencies": {
                "@types/node": "^20.0.0",
                "@types/ws": "^8.5.0",
                "typescript": "^5.0.0",
                "jest": "^29.0.0",
                "@types/jest": "^29.0.0"
            },
            "keywords": ["bitcraps", "api", "client", "gaming", "blockchain"],
            "author": "BitCraps Team <api@bitcraps.com>",
            "license": "MIT"
        });
        
        let package_json_str = serde_json::to_string_pretty(&package_json)?;
        let package_path = output_dir.join("package.json");
        fs::write(package_path, package_json_str).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to write package.json: {}", e)))?;
        
        // Generate TypeScript config
        let tsconfig_json = serde_json::json!({
            "compilerOptions": {
                "target": "ES2020",
                "module": "commonjs",
                "lib": ["ES2020"],
                "outDir": "./dist",
                "rootDir": "./src",
                "strict": true,
                "esModuleInterop": true,
                "skipLibCheck": true,
                "forceConsistentCasingInFileNames": true,
                "declaration": true,
                "declarationMap": true,
                "sourceMap": true
            },
            "include": ["src/**/*"],
            "exclude": ["node_modules", "dist", "**/*.test.ts"]
        });
        
        let tsconfig_str = serde_json::to_string_pretty(&tsconfig_json)?;
        let tsconfig_path = output_dir.join("tsconfig.json");
        fs::write(tsconfig_path, tsconfig_str).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to write tsconfig.json: {}", e)))?;
        
        // Generate client code
        let src_dir = output_dir.join("src");
        fs::create_dir_all(&src_dir).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to create src directory: {}", e)))?;
        
        let client_code = self.generate_typescript_client_code();
        let client_path = src_dir.join("index.ts");
        fs::write(client_path, client_code).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to write client code: {}", e)))?;
        
        // Generate types
        let types_code = self.generate_typescript_types_code();
        let types_path = src_dir.join("types.ts");
        fs::write(types_path, types_code).await
            .map_err(|e| SDKError::ConfigurationError(format!("Failed to write types: {}", e)))?;
        
        // Generate examples if requested
        if include_examples {
            let examples_dir = output_dir.join("examples");
            fs::create_dir_all(&examples_dir).await
                .map_err(|e| SDKError::ConfigurationError(format!("Failed to create examples directory: {}", e)))?;
            
            let example_code = self.generate_typescript_example_code();
            let example_path = examples_dir.join("basic-usage.ts");
            fs::write(example_path, example_code).await
                .map_err(|e| SDKError::ConfigurationError(format!("Failed to write example: {}", e)))?;
        }
        
        Ok(())
    }
    
    // Code generation methods for each language
    
    fn generate_rust_client_code(&self) -> String {
        r#"//! BitCraps API Client for Rust
//! 
//! Generated client library for the BitCraps gaming platform.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

pub mod models;
pub use models::*;

/// BitCraps API client
#[derive(Debug, Clone)]
pub struct BitCrapsClient {
    client: Client,
    base_url: String,
    api_key: String,
}

impl BitCrapsClient {
    /// Create a new client
    pub fn new(api_key: String, base_url: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        
        let base_url = base_url.unwrap_or_else(|| "https://api.bitcraps.com/v2".to_string());
        
        Self {
            client,
            base_url,
            api_key,
        }
    }
    
    /// List available games
    pub async fn list_games(&self) -> Result<Vec<GameInfo>, ClientError> {
        let url = format!("{}/games", self.base_url);
        
        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;
        
        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(ClientError::ApiError(response.status().as_u16()))
        }
    }
    
    /// Create a new game
    pub async fn create_game(&self, request: CreateGameRequest) -> Result<GameInfo, ClientError> {
        let url = format!("{}/games", self.base_url);
        
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;
        
        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(ClientError::ApiError(response.status().as_u16()))
        }
    }
    
    /// Join an existing game
    pub async fn join_game(&self, game_id: &str) -> Result<GameSession, ClientError> {
        let url = format!("{}/games/{}/join", self.base_url, game_id);
        
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;
        
        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(ClientError::ApiError(response.status().as_u16()))
        }
    }
}

/// Client error types
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("HTTP request failed: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("API returned error status: {0}")]
    ApiError(u16),
}
"#.to_string()
    }
    
    fn generate_rust_models_code(&self) -> String {
        r#"//! Data models for BitCraps API

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Game information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInfo {
    pub id: String,
    pub name: String,
    pub game_type: GameType,
    pub status: GameStatus,
    pub current_players: u32,
    pub max_players: u32,
    pub min_bet: u64,
    pub max_bet: u64,
    pub created_at: DateTime<Utc>,
}

/// Game types
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum GameType {
    Craps,
    Poker,
    Blackjack,
    Roulette,
}

/// Game status
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum GameStatus {
    Waiting,
    InProgress,
    Finished,
    Cancelled,
}

/// Create game request
#[derive(Debug, Serialize)]
pub struct CreateGameRequest {
    pub name: String,
    pub game_type: GameType,
    pub max_players: u32,
    pub min_bet: u64,
    pub max_bet: u64,
}

/// Game session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSession {
    pub session_id: String,
    pub game_id: String,
    pub player_id: String,
    pub joined_at: DateTime<Utc>,
}
"#.to_string()
    }
    
    fn generate_rust_example_code(&self) -> String {
        r#"//! Basic usage example for BitCraps Rust client

use bitcraps_client::{BitCrapsClient, CreateGameRequest, GameType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize client
    let client = BitCrapsClient::new(
        "your-api-key".to_string(),
        None // Use default API URL
    );
    
    // List available games
    let games = client.list_games().await?;
    println!("Available games: {}", games.len());
    
    // Create a new game
    let create_request = CreateGameRequest {
        name: "My Craps Game".to_string(),
        game_type: GameType::Craps,
        max_players: 8,
        min_bet: 10,
        max_bet: 1000,
    };
    
    let game = client.create_game(create_request).await?;
    println!("Created game: {} (ID: {})", game.name, game.id);
    
    // Join the game
    let session = client.join_game(&game.id).await?;
    println!("Joined game with session ID: {}", session.session_id);
    
    Ok(())
}
"#.to_string()
    }
    
    fn generate_python_client_code(&self) -> String {
        r#"""BitCraps API Client for Python

Generated client library for the BitCraps gaming platform.
"""

import asyncio
import aiohttp
from typing import List, Optional, Dict, Any
from datetime import datetime
from .models import GameInfo, CreateGameRequest, GameSession

class BitCrapsClient:
    """BitCraps API client"""
    
    def __init__(self, api_key: str, base_url: Optional[str] = None):
        self.api_key = api_key
        self.base_url = base_url or "https://api.bitcraps.com/v2"
        self.session: Optional[aiohttp.ClientSession] = None
    
    async def __aenter__(self):
        self.session = aiohttp.ClientSession(
            headers={"Authorization": f"Bearer {self.api_key}"},
            timeout=aiohttp.ClientTimeout(total=30)
        )
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        if self.session:
            await self.session.close()
    
    async def list_games(self) -> List[GameInfo]:
        """List available games"""
        if not self.session:
            raise RuntimeError("Client not initialized. Use 'async with' context manager.")
        
        url = f"{self.base_url}/games"
        async with self.session.get(url) as response:
            response.raise_for_status()
            data = await response.json()
            return [GameInfo(**game) for game in data]
    
    async def create_game(self, request: CreateGameRequest) -> GameInfo:
        """Create a new game"""
        if not self.session:
            raise RuntimeError("Client not initialized. Use 'async with' context manager.")
        
        url = f"{self.base_url}/games"
        async with self.session.post(url, json=request.dict()) as response:
            response.raise_for_status()
            data = await response.json()
            return GameInfo(**data)
    
    async def join_game(self, game_id: str) -> GameSession:
        """Join an existing game"""
        if not self.session:
            raise RuntimeError("Client not initialized. Use 'async with' context manager.")
        
        url = f"{self.base_url}/games/{game_id}/join"
        async with self.session.post(url) as response:
            response.raise_for_status()
            data = await response.json()
            return GameSession(**data)

__all__ = ["BitCrapsClient"]
"#.to_string()
    }
    
    fn generate_python_models_code(&self) -> String {
        r#"""Data models for BitCraps API"""

from typing import Optional
from datetime import datetime
from pydantic import BaseModel
from enum import Enum

class GameType(str, Enum):
    """Game types"""
    CRAPS = "Craps"
    POKER = "Poker"
    BLACKJACK = "Blackjack"
    ROULETTE = "Roulette"

class GameStatus(str, Enum):
    """Game status"""
    WAITING = "Waiting"
    IN_PROGRESS = "InProgress"
    FINISHED = "Finished"
    CANCELLED = "Cancelled"

class GameInfo(BaseModel):
    """Game information"""
    id: str
    name: str
    game_type: GameType
    status: GameStatus
    current_players: int
    max_players: int
    min_bet: int
    max_bet: int
    created_at: datetime

class CreateGameRequest(BaseModel):
    """Create game request"""
    name: str
    game_type: GameType
    max_players: int
    min_bet: int
    max_bet: int

class GameSession(BaseModel):
    """Game session information"""
    session_id: str
    game_id: str
    player_id: str
    joined_at: datetime

__all__ = [
    "GameType",
    "GameStatus", 
    "GameInfo",
    "CreateGameRequest",
    "GameSession"
]
"#.to_string()
    }
    
    fn generate_python_example_code(&self) -> String {
        r#"""Basic usage example for BitCraps Python client"""

import asyncio
from bitcraps_client import BitCrapsClient
from bitcraps_client.models import CreateGameRequest, GameType

async def main():
    # Initialize client
    async with BitCrapsClient("your-api-key") as client:
        # List available games
        games = await client.list_games()
        print(f"Available games: {len(games)}")
        
        # Create a new game
        create_request = CreateGameRequest(
            name="My Craps Game",
            game_type=GameType.CRAPS,
            max_players=8,
            min_bet=10,
            max_bet=1000
        )
        
        game = await client.create_game(create_request)
        print(f"Created game: {game.name} (ID: {game.id})")
        
        # Join the game
        session = await client.join_game(game.id)
        print(f"Joined game with session ID: {session.session_id}")

if __name__ == "__main__":
    asyncio.run(main())
"#.to_string()
    }
    
    fn generate_typescript_client_code(&self) -> String {
        r#"/**
 * BitCraps API Client for TypeScript/JavaScript
 * 
 * Generated client library for the BitCraps gaming platform.
 */

import axios, { AxiosInstance, AxiosResponse } from 'axios';
import { GameInfo, CreateGameRequest, GameSession } from './types';

export class BitCrapsClient {
    private client: AxiosInstance;

    constructor(apiKey: string, baseUrl?: string) {
        this.client = axios.create({
            baseURL: baseUrl || 'https://api.bitcraps.com/v2',
            timeout: 30000,
            headers: {
                'Authorization': `Bearer ${apiKey}`,
                'Content-Type': 'application/json'
            }
        });
    }

    /**
     * List available games
     */
    async listGames(): Promise<GameInfo[]> {
        const response: AxiosResponse<GameInfo[]> = await this.client.get('/games');
        return response.data;
    }

    /**
     * Create a new game
     */
    async createGame(request: CreateGameRequest): Promise<GameInfo> {
        const response: AxiosResponse<GameInfo> = await this.client.post('/games', request);
        return response.data;
    }

    /**
     * Join an existing game
     */
    async joinGame(gameId: string): Promise<GameSession> {
        const response: AxiosResponse<GameSession> = await this.client.post(`/games/${gameId}/join`);
        return response.data;
    }
}

export * from './types';
"#.to_string()
    }
    
    fn generate_typescript_types_code(&self) -> String {
        r#"/**
 * Type definitions for BitCraps API
 */

export enum GameType {
    CRAPS = 'Craps',
    POKER = 'Poker',
    BLACKJACK = 'Blackjack',
    ROULETTE = 'Roulette'
}

export enum GameStatus {
    WAITING = 'Waiting',
    IN_PROGRESS = 'InProgress',
    FINISHED = 'Finished',
    CANCELLED = 'Cancelled'
}

export interface GameInfo {
    id: string;
    name: string;
    gameType: GameType;
    status: GameStatus;
    currentPlayers: number;
    maxPlayers: number;
    minBet: number;
    maxBet: number;
    createdAt: string;
}

export interface CreateGameRequest {
    name: string;
    gameType: GameType;
    maxPlayers: number;
    minBet: number;
    maxBet: number;
}

export interface GameSession {
    sessionId: string;
    gameId: string;
    playerId: string;
    joinedAt: string;
}

export interface ApiResponse<T> {
    success: boolean;
    data?: T;
    error?: string;
    requestId: string;
    timestamp: string;
}
"#.to_string()
    }
    
    fn generate_typescript_example_code(&self) -> String {
        r#"/**
 * Basic usage example for BitCraps TypeScript client
 */

import { BitCrapsClient, CreateGameRequest, GameType } from 'bitcraps-client';

async function main() {
    // Initialize client
    const client = new BitCrapsClient('your-api-key');
    
    try {
        // List available games
        const games = await client.listGames();
        console.log(`Available games: ${games.length}`);
        
        // Create a new game
        const createRequest: CreateGameRequest = {
            name: 'My Craps Game',
            gameType: GameType.CRAPS,
            maxPlayers: 8,
            minBet: 10,
            maxBet: 1000
        };
        
        const game = await client.createGame(createRequest);
        console.log(`Created game: ${game.name} (ID: ${game.id})`);
        
        // Join the game
        const session = await client.joinGame(game.id);
        console.log(`Joined game with session ID: ${session.sessionId}`);
        
    } catch (error) {
        console.error('Error:', error);
    }
}

main().catch(console.error);
"#.to_string()
    }
}

/// String case conversion utilities
pub fn to_pascal_case(input: &str) -> String {
    input.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
            }
        })
        .collect()
}

pub fn to_snake_case(input: &str) -> String {
    let mut result = String::new();
    let mut prev_char_was_uppercase = false;
    
    for (i, ch) in input.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 && !prev_char_was_uppercase {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
            prev_char_was_uppercase = true;
        } else {
            result.push(ch);
            prev_char_was_uppercase = false;
        }
    }
    
    result
}

pub fn to_camel_case(input: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    
    for ch in input.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_uppercase().next().unwrap());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_case_conversions() {
        assert_eq!(to_pascal_case("hello_world"), "HelloWorld");
        assert_eq!(to_snake_case("HelloWorld"), "hello_world");
        assert_eq!(to_camel_case("hello_world"), "helloWorld");
    }
    
    #[tokio::test]
    async fn test_code_generator_creation() {
        let generator = CodeGenerator::new();
        assert!(generator.templates.contains_key(&TargetLanguage::Rust));
        assert!(generator.templates.contains_key(&TargetLanguage::Python));
        assert!(generator.templates.contains_key(&TargetLanguage::TypeScript));
    }
}
"#.to_string()
    }
}
"#.to_string()
    }
}
"#.to_string()
    }
}
"#.to_string()
    }
}

// Note: In a real implementation, template files would be separate files
// For this example, we're including them as string literals
impl std::fmt::Display for TargetLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TargetLanguage::Rust => write!(f, "rust"),
            TargetLanguage::Python => write!(f, "python"),
            TargetLanguage::JavaScript => write!(f, "javascript"),
            TargetLanguage::TypeScript => write!(f, "typescript"),
            TargetLanguage::Go => write!(f, "go"),
            TargetLanguage::Java => write!(f, "java"),
            TargetLanguage::CSharp => write!(f, "csharp"),
            TargetLanguage::Swift => write!(f, "swift"),
        }
    }
}