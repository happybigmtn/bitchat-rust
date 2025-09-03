# Chapter 85: CLI Design and Architecture - The Art of Human-Computer Conversation

Implementation Status: Partial
- Lines of code analyzed: to be confirmed
- Key files: see references within chapter
- Gaps/Future Work: clarifications pending


## Introduction: The Command Line's Eternal Appeal

In 1969, Ken Thompson sat at a Teletype Model 33 ASR, typing commands into the first Unix system. The interface was simple: type a command, press enter, see the result. More than 50 years later, despite all our graphical innovations, developers still prefer the command line. Why? Because it's the most efficient way for humans to tell computers exactly what to do.

The command line interface is like a conversation with a very literal friend. It doesn't guess what you mean - it does exactly what you say. This precision, combined with composability and scriptability, makes CLIs the power tools of computing. In BitCraps, our CLI isn't just a way to interact with the system; it's the primary interface for operators, developers, and power users who need complete control.

This chapter explores the art and science of building great command-line interfaces. We'll cover everything from parsing arguments to building interactive shells, from error handling to beautiful output formatting. By the end, you'll understand how to create CLIs that are powerful, intuitive, and a joy to use.

## The Anatomy of a Great CLI

A great CLI is like a well-designed kitchen knife - every detail serves a purpose:

### The Parser: Understanding Intent
Converting strings into structured commands

### The Executor: Taking Action
Running commands with proper error handling

### The Presenter: Showing Results
Formatting output for human and machine consumption

### The Helper: Guiding Users
Providing documentation, examples, and error recovery

## Command Line Parsing: From Strings to Structure

Let's start with the foundation - parsing command-line arguments:

```rust
use clap::{Parser, Subcommand, Args};
use std::path::PathBuf;

/// BitCraps - Distributed Gaming Network CLI
#[derive(Parser, Debug)]
#[command(name = "bitcraps")]
#[command(author = "BitCraps Team")]
#[command(version = "1.0.0")]
#[command(about = "Manage and interact with the BitCraps network")]
#[command(long_about = r"
BitCraps is a distributed gaming network that enables peer-to-peer
casino games with cryptographic fairness guarantees.

This CLI provides tools for:
- Running BitCraps nodes
- Managing game sessions
- Monitoring network health
- Debugging consensus issues
")]
pub struct Cli {
    /// Verbosity level (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
    
    /// Configuration file path
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    
    /// Output format
    #[arg(long, value_enum, default_value = "human")]
    pub format: OutputFormat,
    
    /// Disable colored output
    #[arg(long)]
    pub no_color: bool,
    
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable output
    Human,
    /// JSON output for programmatic use
    Json,
    /// YAML output
    Yaml,
    /// Tab-separated values
    Tsv,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start a BitCraps node
    Node(NodeArgs),
    
    /// Manage games
    Game(GameArgs),
    
    /// Network operations
    Network(NetworkArgs),
    
    /// Wallet operations
    Wallet(WalletArgs),
    
    /// Debug and diagnostic tools
    Debug(DebugArgs),
    
    /// Interactive shell
    Shell(ShellArgs),
}

#[derive(Args, Debug)]
pub struct NodeArgs {
    #[command(subcommand)]
    pub command: NodeCommands,
}

#[derive(Subcommand, Debug)]
pub enum NodeCommands {
    /// Start a node
    Start {
        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,
        
        /// Run as daemon
        #[arg(short, long)]
        daemon: bool,
        
        /// Bootstrap nodes
        #[arg(long, value_delimiter = ',')]
        bootstrap: Vec<String>,
    },
    
    /// Stop the node
    Stop {
        /// Force stop without graceful shutdown
        #[arg(short, long)]
        force: bool,
    },
    
    /// Show node status
    Status {
        /// Show detailed status
        #[arg(short, long)]
        detailed: bool,
    },
}
```

## Command Execution: Making Things Happen

Once we've parsed commands, we need to execute them:

```rust
use tokio::runtime::Runtime;
use std::process::ExitCode;

pub struct CommandExecutor {
    runtime: Runtime,
    config: Config,
    output: Box<dyn OutputWriter>,
}

impl CommandExecutor {
    pub fn new(config: Config, format: OutputFormat) -> Result<Self, CliError> {
        let runtime = Runtime::new()?;
        let output: Box<dyn OutputWriter> = match format {
            OutputFormat::Human => Box::new(HumanOutput::new()),
            OutputFormat::Json => Box::new(JsonOutput::new()),
            OutputFormat::Yaml => Box::new(YamlOutput::new()),
            OutputFormat::Tsv => Box::new(TsvOutput::new()),
        };
        
        Ok(Self {
            runtime,
            config,
            output,
        })
    }
    
    pub fn execute(&mut self, cli: Cli) -> ExitCode {
        // Set up logging based on verbosity
        self.setup_logging(cli.verbose);
        
        // Execute command
        let result = match cli.command {
            Commands::Node(args) => self.execute_node(args),
            Commands::Game(args) => self.execute_game(args),
            Commands::Network(args) => self.execute_network(args),
            Commands::Wallet(args) => self.execute_wallet(args),
            Commands::Debug(args) => self.execute_debug(args),
            Commands::Shell(args) => self.execute_shell(args),
        };
        
        // Handle result
        match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                self.output.error(&err);
                err.exit_code()
            }
        }
    }
    
    fn execute_node(&mut self, args: NodeArgs) -> Result<(), CliError> {
        match args.command {
            NodeCommands::Start { port, daemon, bootstrap } => {
                if daemon {
                    self.start_daemon(port, bootstrap)?;
                    self.output.success("Node started as daemon");
                } else {
                    self.start_interactive(port, bootstrap)?;
                }
            }
            NodeCommands::Stop { force } => {
                let pid = self.find_node_pid()?;
                if force {
                    self.force_stop(pid)?;
                    self.output.warning("Node forcefully stopped");
                } else {
                    self.graceful_stop(pid)?;
                    self.output.success("Node stopped gracefully");
                }
            }
            NodeCommands::Status { detailed } => {
                let status = self.get_node_status()?;
                if detailed {
                    self.output.write(&status.detailed());
                } else {
                    self.output.write(&status.summary());
                }
            }
        }
        Ok(())
    }
    
    fn start_daemon(&self, port: u16, bootstrap: Vec<String>) -> Result<(), CliError> {
        use daemonize::Daemonize;
        
        let daemonize = Daemonize::new()
            .pid_file("/tmp/bitcraps.pid")
            .working_directory("/tmp")
            .stdout(File::create("/tmp/bitcraps.out")?)
            .stderr(File::create("/tmp/bitcraps.err")?);
        
        match daemonize.start() {
            Ok(_) => {
                // We're now in the daemon process
                self.run_node(port, bootstrap)
            }
            Err(e) => Err(CliError::DaemonError(e.to_string())),
        }
    }
}
```

## Output Formatting: Speaking Human and Machine

Great CLIs speak multiple languages - human-readable for interactive use, structured for scripts:

```rust
use colored::*;
use serde::Serialize;
use prettytable::{Table, row, cell};

trait OutputWriter: Send + Sync {
    fn write(&mut self, data: &dyn OutputData);
    fn success(&mut self, message: &str);
    fn error(&mut self, error: &CliError);
    fn warning(&mut self, message: &str);
    fn info(&mut self, message: &str);
}

pub struct HumanOutput {
    use_color: bool,
    quiet: bool,
}

impl HumanOutput {
    pub fn new() -> Self {
        Self {
            use_color: atty::is(atty::Stream::Stdout),
            quiet: false,
        }
    }
    
    fn format_table(&self, headers: Vec<&str>, rows: Vec<Vec<String>>) -> String {
        let mut table = Table::new();
        
        // Add headers
        let header_cells: Vec<_> = headers.iter()
            .map(|h| cell!(b -> h))
            .collect();
        table.add_row(prettytable::Row::new(header_cells));
        
        // Add data rows
        for row in rows {
            let cells: Vec<_> = row.iter()
                .map(|v| cell!(v))
                .collect();
            table.add_row(prettytable::Row::new(cells));
        }
        
        table.to_string()
    }
}

impl OutputWriter for HumanOutput {
    fn write(&mut self, data: &dyn OutputData) {
        match data.format_human() {
            HumanFormat::Text(text) => println!("{}", text),
            HumanFormat::Table { headers, rows } => {
                println!("{}", self.format_table(headers, rows));
            }
            HumanFormat::Progress { current, total, message } => {
                let percentage = (current as f64 / total as f64 * 100.0) as u32;
                let bar = self.progress_bar(percentage);
                print!("\r{} [{}] {}/{}  ", message, bar, current, total);
                std::io::stdout().flush().unwrap();
            }
        }
    }
    
    fn success(&mut self, message: &str) {
        if self.use_color {
            println!("✓ {}", message.green());
        } else {
            println!("✓ {}", message);
        }
    }
    
    fn error(&mut self, error: &CliError) {
        if self.use_color {
            eprintln!("✗ {}: {}", "Error".red().bold(), error.to_string().red());
        } else {
            eprintln!("✗ Error: {}", error);
        }
        
        // Show additional context
        if let Some(context) = error.context() {
            eprintln!("  {}", context.dimmed());
        }
        
        // Show suggestions
        if let Some(suggestion) = error.suggestion() {
            eprintln!("\n💡 {}: {}", "Suggestion".yellow(), suggestion);
        }
    }
    
    fn warning(&mut self, message: &str) {
        if self.use_color {
            println!("⚠ {}", message.yellow());
        } else {
            println!("⚠ {}", message);
        }
    }
}

pub struct JsonOutput;

impl OutputWriter for JsonOutput {
    fn write(&mut self, data: &dyn OutputData) {
        let json = serde_json::to_string_pretty(data.as_json()).unwrap();
        println!("{}", json);
    }
    
    fn success(&mut self, message: &str) {
        let output = serde_json::json!({
            "status": "success",
            "message": message
        });
        println!("{}", output);
    }
    
    fn error(&mut self, error: &CliError) {
        let output = serde_json::json!({
            "status": "error",
            "code": error.code(),
            "message": error.to_string(),
            "context": error.context()
        });
        eprintln!("{}", output);
    }
}
```

## Interactive Shell: Building a REPL

For complex operations, an interactive shell provides a better experience:

```rust
use rustyline::error::ReadlineError;
use rustyline::{Editor, Config, EditMode, CompletionType};
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::highlight::Highlighter;
use rustyline::completion::{Completer, Pair};

pub struct BitCrapsShell {
    editor: Editor<ShellHelper>,
    executor: CommandExecutor,
    context: ShellContext,
}

struct ShellContext {
    current_game: Option<GameId>,
    connected_node: Option<NodeInfo>,
    command_history: Vec<String>,
    aliases: HashMap<String, String>,
}

impl BitCrapsShell {
    pub fn new() -> Result<Self, CliError> {
        let config = Config::builder()
            .history_ignore_space(true)
            .completion_type(CompletionType::List)
            .edit_mode(EditMode::Emacs)
            .build();
        
        let mut editor = Editor::with_config(config);
        editor.set_helper(Some(ShellHelper::new()));
        editor.load_history("~/.bitcraps_history").ok();
        
        Ok(Self {
            editor,
            executor: CommandExecutor::new(Default::default(), OutputFormat::Human)?,
            context: ShellContext::default(),
        })
    }
    
    pub fn run(&mut self) -> Result<(), CliError> {
        self.print_banner();
        
        loop {
            let prompt = self.build_prompt();
            
            match self.editor.readline(&prompt) {
                Ok(line) => {
                    self.editor.add_history_entry(&line);
                    
                    match self.process_line(&line) {
                        Ok(ShellCommand::Exit) => break,
                        Ok(ShellCommand::Clear) => self.clear_screen(),
                        Ok(ShellCommand::Execute(cmd)) => {
                            self.execute_command(cmd);
                        }
                        Err(err) => self.show_error(err),
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("Use 'exit' to quit or Ctrl-D");
                }
                Err(ReadlineError::Eof) => {
                    println!("Goodbye!");
                    break;
                }
                Err(err) => {
                    eprintln!("Error: {:?}", err);
                    break;
                }
            }
        }
        
        self.editor.save_history("~/.bitcraps_history")?;
        Ok(())
    }
    
    fn build_prompt(&self) -> String {
        let mut prompt = String::new();
        
        // Show connection status
        if let Some(node) = &self.context.connected_node {
            prompt.push_str(&format!("[{}]", node.address.green()));
        } else {
            prompt.push_str(&format!("[{}]", "disconnected".red()));
        }
        
        // Show current game
        if let Some(game_id) = &self.context.current_game {
            prompt.push_str(&format!(" game:{}", game_id.to_string().blue()));
        }
        
        prompt.push_str(" > ");
        prompt
    }
    
    fn print_banner(&self) {
        println!(r"
╔══════════════════════════════════════════════════════╗
║        BitCraps Interactive Shell v1.0.0             ║
║   Type 'help' for commands, 'exit' to quit          ║
╚══════════════════════════════════════════════════════╝
        ");
    }
}

struct ShellHelper {
    commands: Vec<&'static str>,
    hinter: HistoryHinter,
}

impl Completer for ShellHelper {
    type Candidate = Pair;
    
    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> Result<(usize, Vec<Pair>), ReadlineError> {
        let mut candidates = Vec::new();
        
        // Get the word being completed
        let start = line[..pos].rfind(char::is_whitespace)
            .map(|i| i + 1)
            .unwrap_or(0);
        let word = &line[start..pos];
        
        // Complete commands
        for cmd in &self.commands {
            if cmd.starts_with(word) {
                candidates.push(Pair {
                    display: cmd.to_string(),
                    replacement: cmd.to_string(),
                });
            }
        }
        
        Ok((start, candidates))
    }
}
```

## Progress and Status Reporting

Long-running operations need clear progress indication:

```rust
use indicatif::{ProgressBar, ProgressStyle, MultiProgress};
use std::time::Duration;

pub struct ProgressReporter {
    multi: MultiProgress,
    bars: HashMap<String, ProgressBar>,
    style: ProgressStyle,
}

impl ProgressReporter {
    pub fn new() -> Self {
        let style = ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-")
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");
        
        Self {
            multi: MultiProgress::new(),
            bars: HashMap::new(),
            style,
        }
    }
    
    pub fn add_task(&mut self, id: String, total: u64, message: String) -> ProgressBar {
        let bar = ProgressBar::new(total);
        bar.set_style(self.style.clone());
        bar.set_message(message);
        
        let bar = self.multi.add(bar);
        self.bars.insert(id, bar.clone());
        bar
    }
    
    pub fn update(&self, id: &str, progress: u64) {
        if let Some(bar) = self.bars.get(id) {
            bar.set_position(progress);
        }
    }
    
    pub fn finish(&self, id: &str, message: String) {
        if let Some(bar) = self.bars.get(id) {
            bar.finish_with_message(message);
        }
    }
}

// Spinner for indeterminate progress
pub struct Spinner {
    spinner: ProgressBar,
}

impl Spinner {
    pub fn new(message: String) -> Self {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap()
                .tick_strings(&[
                    "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"
                ])
        );
        spinner.set_message(message);
        spinner.enable_steady_tick(Duration::from_millis(100));
        
        Self { spinner }
    }
    
    pub fn finish(self, message: String) {
        self.spinner.finish_with_message(message);
    }
}
```

## Error Handling: Failing Gracefully

Good error messages are crucial for user experience:

```rust
use thiserror::Error;
use std::fmt;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("Node connection failed: {0}")]
    ConnectionError(String),
    
    #[error("Invalid configuration: {0}")]
    ConfigError(String),
    
    #[error("Command failed: {0}")]
    CommandError(String),
    
    #[error("Network error: {0}")]
    NetworkError(#[from] NetworkError),
    
    #[error("File not found: {path}")]
    FileNotFound { path: PathBuf },
    
    #[error("Permission denied: {action}")]
    PermissionDenied { action: String },
    
    #[error("Invalid input: {input}")]
    InvalidInput { input: String, reason: String },
}

impl CliError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            CliError::ConnectionError(_) => ExitCode::from(2),
            CliError::ConfigError(_) => ExitCode::from(3),
            CliError::CommandError(_) => ExitCode::from(4),
            CliError::NetworkError(_) => ExitCode::from(5),
            CliError::FileNotFound { .. } => ExitCode::from(6),
            CliError::PermissionDenied { .. } => ExitCode::from(7),
            CliError::InvalidInput { .. } => ExitCode::from(8),
        }
    }
    
    pub fn context(&self) -> Option<String> {
        match self {
            CliError::ConnectionError(_) => {
                Some("Check if the node is running and accessible".to_string())
            }
            CliError::ConfigError(_) => {
                Some("Verify your configuration file syntax".to_string())
            }
            CliError::FileNotFound { path } => {
                Some(format!("Searched in: {}", path.display()))
            }
            _ => None,
        }
    }
    
    pub fn suggestion(&self) -> Option<String> {
        match self {
            CliError::ConnectionError(_) => {
                Some("Try: bitcraps node start".to_string())
            }
            CliError::ConfigError(_) => {
                Some("Try: bitcraps config validate".to_string())
            }
            CliError::PermissionDenied { .. } => {
                Some("Try running with sudo or check file permissions".to_string())
            }
            _ => None,
        }
    }
}
```

## Configuration Management

CLIs need flexible configuration handling:

```rust
use config::{Config, File, Environment};
use directories::ProjectDirs;

pub struct ConfigManager {
    config: Config,
    paths: ConfigPaths,
}

struct ConfigPaths {
    system: PathBuf,
    user: PathBuf,
    local: PathBuf,
}

impl ConfigManager {
    pub fn load() -> Result<Self, CliError> {
        let paths = Self::get_config_paths()?;
        
        let mut config = Config::builder();
        
        // Load system-wide defaults
        if paths.system.exists() {
            config = config.add_source(File::from(paths.system.clone()));
        }
        
        // Load user configuration
        if paths.user.exists() {
            config = config.add_source(File::from(paths.user.clone()));
        }
        
        // Load local project configuration
        if paths.local.exists() {
            config = config.add_source(File::from(paths.local.clone()));
        }
        
        // Override with environment variables
        config = config.add_source(
            Environment::with_prefix("BITCRAPS")
                .separator("_")
                .try_parsing(true)
        );
        
        let config = config.build()?;
        
        Ok(Self { config, paths })
    }
    
    fn get_config_paths() -> Result<ConfigPaths, CliError> {
        let proj_dirs = ProjectDirs::from("com", "bitcraps", "cli")
            .ok_or_else(|| CliError::ConfigError("Cannot determine config directory".into()))?;
        
        Ok(ConfigPaths {
            system: PathBuf::from("/etc/bitcraps/config.toml"),
            user: proj_dirs.config_dir().join("config.toml"),
            local: PathBuf::from(".bitcraps.toml"),
        })
    }
    
    pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<T, CliError> {
        self.config.get(key)
            .map_err(|e| CliError::ConfigError(e.to_string()))
    }
    
    pub fn set(&mut self, key: &str, value: impl Into<config::Value>) -> Result<(), CliError> {
        self.config.set(key, value)
            .map_err(|e| CliError::ConfigError(e.to_string()))
    }
}
```

## Testing CLI Applications

Testing CLIs requires special approaches:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_help_command() {
        let mut cmd = Command::cargo_bin("bitcraps").unwrap();
        cmd.arg("--help")
            .assert()
            .success()
            .stdout(predicate::str::contains("BitCraps - Distributed Gaming Network"));
    }
    
    #[test]
    fn test_node_start_stop() {
        let temp_dir = TempDir::new().unwrap();
        
        // Start node
        let mut cmd = Command::cargo_bin("bitcraps").unwrap();
        cmd.arg("node")
            .arg("start")
            .arg("--port")
            .arg("8080")
            .arg("--daemon")
            .assert()
            .success();
        
        // Check status
        let mut cmd = Command::cargo_bin("bitcraps").unwrap();
        cmd.arg("node")
            .arg("status")
            .assert()
            .success()
            .stdout(predicate::str::contains("Running"));
        
        // Stop node
        let mut cmd = Command::cargo_bin("bitcraps").unwrap();
        cmd.arg("node")
            .arg("stop")
            .assert()
            .success();
    }
    
    #[test]
    fn test_invalid_command() {
        let mut cmd = Command::cargo_bin("bitcraps").unwrap();
        cmd.arg("invalid")
            .assert()
            .failure()
            .stderr(predicate::str::contains("unrecognized subcommand"));
    }
}

// Integration test helper
pub struct CliTestHarness {
    binary: PathBuf,
    working_dir: TempDir,
}

impl CliTestHarness {
    pub fn new() -> Self {
        Self {
            binary: assert_cmd::cargo::cargo_bin("bitcraps"),
            working_dir: TempDir::new().unwrap(),
        }
    }
    
    pub fn run(&self, args: &[&str]) -> assert_cmd::assert::Assert {
        Command::new(&self.binary)
            .current_dir(&self.working_dir)
            .args(args)
            .assert()
    }
    
    pub fn run_interactive(&self, input: &str) -> assert_cmd::assert::Assert {
        Command::new(&self.binary)
            .current_dir(&self.working_dir)
            .arg("shell")
            .write_stdin(input)
            .assert()
    }
}
```

## BitCraps CLI in Action

Here's how all these pieces come together in the BitCraps CLI:

```rust
pub struct BitCrapsCli {
    executor: CommandExecutor,
    config: ConfigManager,
    client: BitCrapsClient,
}

impl BitCrapsCli {
    pub fn run() -> ExitCode {
        // Parse command line arguments
        let cli = Cli::parse();
        
        // Load configuration
        let config = match ConfigManager::load() {
            Ok(config) => config,
            Err(err) => {
                eprintln!("Failed to load configuration: {}", err);
                return ExitCode::from(3);
            }
        };
        
        // Create executor
        let executor = match CommandExecutor::new(config, cli.format) {
            Ok(exec) => exec,
            Err(err) => {
                eprintln!("Failed to initialize: {}", err);
                return ExitCode::from(1);
            }
        };
        
        // Execute command
        executor.execute(cli)
    }
}

// Main entry point
fn main() -> ExitCode {
    // Set up panic handler
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("\n{}: {}", "Fatal error".red().bold(), panic_info);
        eprintln!("\nThis is a bug. Please report it at:");
        eprintln!("  https://github.com/bitcraps/bitcraps/issues");
    }));
    
    // Handle Ctrl+C gracefully
    ctrlc::set_handler(|| {
        println!("\nInterrupted. Cleaning up...");
        std::process::exit(130); // Standard exit code for SIGINT
    }).expect("Error setting Ctrl-C handler");
    
    // Run CLI
    BitCrapsCli::run()
}
```

## Advanced Features: Plugins and Extensions

Supporting plugins makes your CLI extensible:

```rust
use libloading::{Library, Symbol};

pub trait CliPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn commands(&self) -> Vec<PluginCommand>;
    fn execute(&self, command: &str, args: Vec<String>) -> Result<(), PluginError>;
}

pub struct PluginManager {
    plugins: HashMap<String, Box<dyn CliPlugin>>,
    libraries: Vec<Library>,
}

impl PluginManager {
    pub fn load_plugins(plugin_dir: &Path) -> Result<Self, CliError> {
        let mut manager = Self {
            plugins: HashMap::new(),
            libraries: Vec::new(),
        };
        
        // Scan for plugin files
        for entry in std::fs::read_dir(plugin_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension() == Some(OsStr::new("so")) ||
               path.extension() == Some(OsStr::new("dylib")) ||
               path.extension() == Some(OsStr::new("dll")) {
                manager.load_plugin(&path)?;
            }
        }
        
        Ok(manager)
    }
    
    unsafe fn load_plugin(&mut self, path: &Path) -> Result<(), CliError> {
        let lib = Library::new(path)?;
        
        // Get the plugin entry point
        let create_plugin: Symbol<fn() -> Box<dyn CliPlugin>> = 
            lib.get(b"create_plugin")?;
        
        let plugin = create_plugin();
        
        println!("Loaded plugin: {} v{}", plugin.name(), plugin.version());
        
        self.plugins.insert(plugin.name().to_string(), plugin);
        self.libraries.push(lib);
        
        Ok(())
    }
}
```

## Practical Exercises

### Exercise 1: Build a Custom Command Parser
Create a parser that handles complex command syntax:

```rust
pub struct CommandParser {
    // Your implementation
}

impl CommandParser {
    fn parse(&self, input: &str) -> Result<ParsedCommand, ParseError> {
        // Your task: Parse commands like:
        // - game start --players alice,bob --bet 100
        // - node connect 192.168.1.1:8080 && status
        // - wallet send 100 to alice || echo "failed"
        todo!("Implement command parsing with operators")
    }
}
```

### Exercise 2: Implement Auto-completion
Build smart auto-completion for the shell:

```rust
struct SmartCompleter {
    history: Vec<String>,
    context: CompletionContext,
}

impl SmartCompleter {
    fn complete(&self, partial: &str, position: usize) -> Vec<Completion> {
        // Your task: Implement context-aware completion
        // - Complete command names
        // - Complete file paths
        // - Complete option names
        // - Complete based on command context
        todo!("Implement smart completion")
    }
}
```

### Exercise 3: Create a TUI Dashboard
Build a terminal UI for monitoring:

```rust
use tui::{Terminal, Frame};

struct Dashboard {
    // Your implementation
}

impl Dashboard {
    fn draw(&self, frame: &mut Frame) {
        // Your task: Create a dashboard showing:
        // - Network status
        // - Active games
        // - Node metrics
        // - Recent transactions
        todo!("Implement TUI dashboard")
    }
}
```

## Common Pitfalls and Best Practices

### 1. Argument Parsing Ambiguity
Be careful with optional arguments:

```rust
// Bad: Ambiguous
bitcraps send alice 100  // Is alice the recipient or amount?

// Good: Clear
bitcraps send --to alice --amount 100
```

### 2. Exit Codes Matter
Use meaningful exit codes:

```rust
// Standard exit codes
const EXIT_SUCCESS: i32 = 0;
const EXIT_GENERAL_ERROR: i32 = 1;
const EXIT_MISUSE: i32 = 2;
const EXIT_CANNOT_EXECUTE: i32 = 126;
const EXIT_COMMAND_NOT_FOUND: i32 = 127;
const EXIT_INVALID_ARGUMENT: i32 = 128;
```

### 3. Color and Terminal Detection
Respect user preferences:

```rust
fn should_use_color() -> bool {
    // Check if stdout is a terminal
    if !atty::is(atty::Stream::Stdout) {
        return false;
    }
    
    // Check environment variables
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }
    
    if let Ok(term) = std::env::var("TERM") {
        if term == "dumb" {
            return false;
        }
    }
    
    true
}
```

## Conclusion: The Power of Text

The command line has survived every attempt to replace it because it embodies a fundamental truth: text is the universal interface. It's readable by humans, parseable by machines, and composable in ways GUIs can never match.

A great CLI is more than just a way to run commands - it's a conversation with your users. It should be predictable yet powerful, simple yet sophisticated. The BitCraps CLI demonstrates that modern command-line interfaces can be as user-friendly as they are powerful.

Key principles to remember:

1. **Consistency is king** - Similar operations should work similarly
2. **Documentation is UI** - Help text is part of the user experience
3. **Errors are teachers** - Good error messages educate users
4. **Composition over complexity** - Simple commands that combine are better than complex monoliths
5. **Respect the ecosystem** - Follow platform conventions and integrate with existing tools

The command line isn't going anywhere. Master it, and you master one of computing's most enduring interfaces.

## Additional Resources

- **The Art of Command Line** - Essential CLI knowledge
- **GNU Coding Standards** - CLI conventions
- **Command Line Interface Guidelines** - Modern CLI design principles
- **docopt** - Command-line interface description language

Remember: A CLI is often the first experience developers have with your system. Make it count.
