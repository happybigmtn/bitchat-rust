//! Database CLI for migration management

use crate::database::migrations::MigrationManager;
use crate::error::{Error, Result};
use clap::{Parser, Subcommand};
use rusqlite::Connection;
use std::path::PathBuf;

/// Database management CLI
#[derive(Parser)]
#[command(name = "bitcraps-db")]
#[command(about = "BitCraps database management tool")]
pub struct DbCli {
    /// Database file path
    #[arg(short, long, default_value = "./data/bitcraps.db")]
    pub database: PathBuf,

    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: DbCommand,
}

#[derive(Subcommand)]
pub enum DbCommand {
    /// Run pending migrations
    Migrate {
        /// Dry run - show what would be done without applying
        #[arg(long)]
        dry_run: bool,
    },

    /// Rollback to a specific version
    Rollback {
        /// Target version to rollback to
        version: u32,

        /// Force rollback without confirmation
        #[arg(long)]
        force: bool,
    },

    /// Show migration status
    Status,

    /// Validate migrations
    Validate,

    /// Create a new migration file
    Create {
        /// Migration name
        name: String,
    },

    /// Reset database (drop all tables and re-run migrations)
    Reset {
        /// Force reset without confirmation
        #[arg(long)]
        force: bool,
    },

    /// Export database schema
    Export {
        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Export format
        #[arg(short, long, default_value = "sql")]
        format: ExportFormat,
    },

    /// Import data from backup
    Import {
        /// Input file path
        input: PathBuf,

        /// Skip validation
        #[arg(long)]
        skip_validation: bool,
    },

    /// Run database maintenance
    Maintenance {
        /// Vacuum database to reclaim space
        #[arg(long)]
        vacuum: bool,

        /// Analyze tables for query optimization
        #[arg(long)]
        analyze: bool,

        /// Check database integrity
        #[arg(long)]
        check: bool,
    },
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum ExportFormat {
    Sql,
    Json,
    Csv,
}

impl DbCli {
    /// Execute the CLI command
    pub async fn execute(self) -> Result<()> {
        // Initialize logging
        if self.verbose {
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::DEBUG)
                .init();
        } else {
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::INFO)
                .init();
        }

        // Ensure database directory exists
        if let Some(parent) = self.database.parent() {
            std::fs::create_dir_all(parent).map_err(Error::Io)?;
        }

        match &self.command {
            DbCommand::Migrate { dry_run } => {
                self.run_migrations(*dry_run).await?;
            }
            DbCommand::Rollback { version, force } => {
                self.rollback_migrations(*version, *force).await?;
            }
            DbCommand::Status => {
                self.show_status().await?;
            }
            DbCommand::Validate => {
                self.validate_migrations().await?;
            }
            DbCommand::Create { name } => {
                self.create_migration(name).await?;
            }
            DbCommand::Reset { force } => {
                self.reset_database(*force).await?;
            }
            DbCommand::Export { output, format } => {
                self.export_schema(output.clone(), *format).await?;
            }
            DbCommand::Import {
                input,
                skip_validation,
            } => {
                self.import_data(input.clone(), *skip_validation).await?;
            }
            DbCommand::Maintenance {
                vacuum,
                analyze,
                check,
            } => {
                self.run_maintenance(*vacuum, *analyze, *check).await?;
            }
        }

        Ok(())
    }

    async fn run_migrations(&self, dry_run: bool) -> Result<()> {
        let conn = Connection::open(&self.database).map_err(|e| Error::Database(e.to_string()))?;
        let mut manager = MigrationManager::new().with_connection(conn);

        if dry_run {
            println!("🔍 Dry run mode - no changes will be made\n");

            let conn =
                Connection::open(&self.database).map_err(|e| Error::Database(e.to_string()))?;
            let status = manager.status(&conn)?;

            if status.pending.is_empty() {
                println!(
                    "✅ Database is up to date (version {})",
                    status.current_version
                );
            } else {
                println!("📋 Pending migrations:");
                for version in &status.pending {
                    println!("  - Version {}", version);
                }
            }
        } else {
            println!("🚀 Running migrations...\n");

            let report = manager.migrate()?;

            if report.is_success() {
                println!(
                    "✅ Successfully applied {} migrations",
                    report.successful.len()
                );
                println!("📌 Database is now at version {}", report.final_version);
            } else {
                println!("❌ Migration failed!");
                for (version, error) in &report.failed {
                    println!("  - Version {}: {}", version, error);
                }
            }

            if !report.skipped.is_empty() {
                println!(
                    "⏭️  Skipped {} already applied migrations",
                    report.skipped.len()
                );
            }
        }

        Ok(())
    }

    async fn rollback_migrations(&self, target_version: u32, force: bool) -> Result<()> {
        if !force {
            println!(
                "⚠️  WARNING: This will rollback the database to version {}",
                target_version
            );
            println!("This operation cannot be undone without re-running migrations.");
            print!("Continue? [y/N]: ");

            use std::io::{self, BufRead};
            let stdin = io::stdin();
            let mut line = String::new();
            stdin.lock().read_line(&mut line)?;

            if !line.trim().eq_ignore_ascii_case("y") {
                println!("Rollback cancelled");
                return Ok(());
            }
        }

        let conn = Connection::open(&self.database).map_err(|e| Error::Database(e.to_string()))?;
        let mut manager = MigrationManager::new().with_connection(conn);

        println!("🔄 Rolling back to version {}...", target_version);

        let report = manager.rollback_to(target_version)?;

        if report.is_success() {
            println!(
                "✅ Successfully rolled back {} migrations",
                report.successful.len()
            );
            println!("📌 Database is now at version {}", report.final_version);
        } else {
            println!("❌ Rollback failed!");
            for (version, error) in &report.failed {
                println!("  - Version {}: {}", version, error);
            }
        }

        Ok(())
    }

    async fn show_status(&self) -> Result<()> {
        let conn = Connection::open(&self.database).map_err(|e| Error::Database(e.to_string()))?;
        let manager = MigrationManager::new();
        let status = manager.status(&conn)?;

        println!("📊 Database Migration Status\n");
        println!("Current version: {}", status.current_version);
        println!("Latest version:  {}", status.latest_version);
        println!(
            "Up to date:      {}",
            if status.is_up_to_date {
                "✅ Yes"
            } else {
                "❌ No"
            }
        );

        if !status.applied.is_empty() {
            println!("\n✅ Applied migrations:");
            for version in &status.applied {
                println!("  - Version {}", version);
            }
        }

        if !status.pending.is_empty() {
            println!("\n⏳ Pending migrations:");
            for version in &status.pending {
                println!("  - Version {}", version);
            }
        }

        Ok(())
    }

    async fn validate_migrations(&self) -> Result<()> {
        let conn = Connection::open(&self.database).map_err(|e| Error::Database(e.to_string()))?;
        let manager = MigrationManager::new();
        let report = manager.validate(&conn)?;

        println!("🔍 Validation Report\n");

        if report.is_valid {
            println!("✅ All migrations are valid");
        } else {
            println!("❌ Validation failed");
        }

        if !report.errors.is_empty() {
            println!("\n❌ Errors:");
            for error in &report.errors {
                println!("  - {}", error);
            }
        }

        if !report.warnings.is_empty() {
            println!("\n⚠️  Warnings:");
            for warning in &report.warnings {
                println!("  - {}", warning);
            }
        }

        Ok(())
    }

    async fn create_migration(&self, name: &str) -> Result<()> {
        let timestamp = chrono::Utc::now().timestamp();
        let filename = format!("V{}_{}.sql", timestamp, name);
        let filepath = PathBuf::from("migrations").join(&filename);

        // Ensure migrations directory exists
        std::fs::create_dir_all("migrations").map_err(Error::Io)?;

        let template = format!(
            r#"-- Migration: {}
-- Created: {}

-- Up Migration
-- Add your forward migration SQL here

-- Down Migration
-- Add your rollback SQL here (optional but recommended)
"#,
            name,
            chrono::Utc::now().to_rfc3339()
        );

        std::fs::write(&filepath, template).map_err(Error::Io)?;

        println!("✅ Created migration file: {}", filepath.display());
        println!("📝 Edit this file to add your migration SQL");

        Ok(())
    }

    async fn reset_database(&self, force: bool) -> Result<()> {
        if !force {
            println!("⚠️  WARNING: This will DELETE ALL DATA and reset the database!");
            println!("This operation cannot be undone!");
            print!("Type 'RESET' to confirm: ");

            use std::io::{self, BufRead};
            let stdin = io::stdin();
            let mut line = String::new();
            stdin.lock().read_line(&mut line)?;

            if line.trim() != "RESET" {
                println!("Reset cancelled");
                return Ok(());
            }
        }

        println!("🗑️  Resetting database...");

        // Drop the database file
        if self.database.exists() {
            std::fs::remove_file(&self.database).map_err(Error::Io)?;
        }

        // Re-run all migrations
        let conn = Connection::open(&self.database).map_err(|e| Error::Database(e.to_string()))?;
        let mut manager = MigrationManager::new().with_connection(conn);
        let report = manager.migrate()?;

        if report.is_success() {
            println!("✅ Database reset complete");
            println!("📌 Applied {} migrations", report.successful.len());
        } else {
            println!("❌ Reset failed during migration");
        }

        Ok(())
    }

    async fn export_schema(&self, output: Option<PathBuf>, format: ExportFormat) -> Result<()> {
        let conn = Connection::open(&self.database).map_err(|e| Error::Database(e.to_string()))?;

        let schema = match format {
            ExportFormat::Sql => {
                // Export as SQL DDL
                let mut sql = String::new();
                let mut stmt = conn.prepare(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"
                ).map_err(|e| Error::Database(e.to_string()))?;

                let rows = stmt
                    .query_map([], |row| row.get::<_, String>(0))
                    .map_err(|e| Error::Database(e.to_string()))?;

                for row in rows {
                    sql.push_str(&row.map_err(|e| Error::Database(e.to_string()))?);
                    sql.push_str(";\n\n");
                }

                sql
            }
            ExportFormat::Json => {
                // Export as JSON schema
                let tables = self.get_table_schemas(&conn)?;
                serde_json::to_string_pretty(&tables).map_err(|e| Error::Database(e.to_string()))?
            }
            ExportFormat::Csv => {
                // Export table list as CSV
                let mut csv = String::from("table_name,column_count,row_count\n");
                let tables = self.get_table_info(&conn)?;
                for (name, col_count, row_count) in tables {
                    csv.push_str(&format!("{},{},{}\n", name, col_count, row_count));
                }
                csv
            }
        };

        if let Some(path) = output {
            std::fs::write(&path, schema).map_err(Error::Io)?;
            println!("✅ Exported schema to {}", path.display());
        } else {
            println!("{}", schema);
        }

        Ok(())
    }

    async fn import_data(&self, input: PathBuf, skip_validation: bool) -> Result<()> {
        if !input.exists() {
            return Err(crate::error::Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Input file not found: {}", input.display()),
            )));
        }

        println!("📥 Importing data from {}...", input.display());

        let sql = std::fs::read_to_string(&input).map_err(Error::Io)?;
        let conn = Connection::open(&self.database).map_err(|e| Error::Database(e.to_string()))?;

        if !skip_validation {
            // Basic SQL validation
            if !sql.contains("INSERT") && !sql.contains("CREATE") {
                return Err(crate::error::Error::Database(
                    "Invalid SQL file - no INSERT or CREATE statements found".into(),
                ));
            }
        }

        conn.execute_batch(&sql)
            .map_err(|e| Error::Database(e.to_string()))?;

        println!("✅ Data imported successfully");

        Ok(())
    }

    async fn run_maintenance(&self, vacuum: bool, analyze: bool, check: bool) -> Result<()> {
        let conn = Connection::open(&self.database).map_err(|e| Error::Database(e.to_string()))?;

        if check {
            println!("🔍 Checking database integrity...");
            let result: String = conn
                .query_row("PRAGMA integrity_check", [], |row| row.get(0))
                .map_err(|e| Error::Database(e.to_string()))?;
            if result == "ok" {
                println!("✅ Database integrity check passed");
            } else {
                println!("❌ Database integrity check failed: {}", result);
            }
        }

        if analyze {
            println!("📊 Analyzing tables...");
            conn.execute("ANALYZE", [])
                .map_err(|e| Error::Database(e.to_string()))?;
            println!("✅ Table analysis complete");
        }

        if vacuum {
            println!("🧹 Vacuuming database...");
            conn.execute("VACUUM", [])
                .map_err(|e| Error::Database(e.to_string()))?;
            println!("✅ Database vacuum complete");
        }

        Ok(())
    }

    fn get_table_schemas(&self, conn: &Connection) -> Result<serde_json::Value> {
        use serde_json::json;

        let mut tables = json!({});

        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            )
            .map_err(|e| Error::Database(e.to_string()))?;

        let table_names: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| Error::Database(e.to_string()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Database(e.to_string()))?;

        for table_name in table_names {
            let mut columns = json!([]);
            let mut col_stmt = conn
                .prepare(&format!("PRAGMA table_info({})", table_name))
                .map_err(|e| Error::Database(e.to_string()))?;

            let col_rows = col_stmt
                .query_map([], |row| {
                    Ok(json!({
                        "name": row.get::<_, String>(1)?,
                        "type": row.get::<_, String>(2)?,
                        "nullable": row.get::<_, i32>(3)? == 0,
                        "primary_key": row.get::<_, i32>(5)? == 1,
                    }))
                })
                .map_err(|e| Error::Database(e.to_string()))?;

            for col in col_rows {
                columns
                    .as_array_mut()
                    .unwrap()
                    .push(col.map_err(|e| Error::Database(e.to_string()))?);
            }

            tables[table_name] = columns;
        }

        Ok(tables)
    }

    fn get_table_info(&self, conn: &Connection) -> Result<Vec<(String, usize, usize)>> {
        let mut result = Vec::new();

        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            )
            .map_err(|e| Error::Database(e.to_string()))?;

        let table_names: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| Error::Database(e.to_string()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Database(e.to_string()))?;

        for table_name in table_names {
            // Get column count - Use parameter binding to prevent SQL injection
            let col_count: usize = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info(?)",
                    [&table_name],
                    |row| row.get(0),
                )
                .map_err(|e| Error::Database(e.to_string()))?;

            // Get row count - Use proper table name validation and quoting
            // Note: Table names cannot be parameterized in SQLite, so we validate the name first
            if !table_name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                return Err(Error::Database(format!(
                    "Invalid table name: {}",
                    table_name
                )));
            }
            let row_count: usize = conn
                .query_row(
                    &format!("SELECT COUNT(*) FROM \"{}\"", table_name),
                    [],
                    |row| row.get(0),
                )
                .map_err(|e| Error::Database(e.to_string()))?;

            result.push((table_name, col_count, row_count));
        }

        Ok(result)
    }
}
