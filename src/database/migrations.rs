//! Database migration system

#[cfg(feature = "sqlite")]
use async_trait::async_trait;
#[cfg(feature = "sqlite")]
use sqlx::sqlite::SqlitePool;
#[cfg(feature = "sqlite")]
use sqlx::Row;
#[cfg(feature = "sqlite")]
use tracing::{info, warn};

#[cfg(feature = "sqlite")]
use crate::error::{RatNetError, Result};

/// Database migration trait
#[cfg(feature = "sqlite")]
#[async_trait]
pub trait Migration: Send + Sync {
    /// Get the migration version number
    fn version(&self) -> u32;

    /// Get the migration description
    fn description(&self) -> &str;

    /// Apply the migration
    async fn up(&self, pool: &SqlitePool) -> Result<()>;

    /// Rollback the migration (optional)
    async fn down(&self, pool: &SqlitePool) -> Result<()> {
        Err(RatNetError::NotImplemented(
            "Migration rollback not implemented".to_string(),
        ))
    }
}

/// Migration manager for database schema versioning
#[cfg(feature = "sqlite")]
pub struct MigrationManager {
    migrations: Vec<Box<dyn Migration>>,
}

#[cfg(feature = "sqlite")]
impl MigrationManager {
    /// Create a new migration manager
    pub fn new() -> Self {
        Self {
            migrations: Vec::new(),
        }
    }

    /// Add a migration
    pub fn add_migration(mut self, migration: Box<dyn Migration>) -> Self {
        self.migrations.push(migration);
        self
    }

    /// Run all pending migrations
    pub async fn migrate(&self, pool: &SqlitePool) -> Result<()> {
        // Create migrations table if it doesn't exist
        self.create_migrations_table(pool).await?;

        // Get current schema version
        let current_version = self.get_current_version(pool).await?;

        // Sort migrations by version
        let mut migrations = self.migrations.iter().collect::<Vec<_>>();
        migrations.sort_by_key(|m| m.version());

        // Apply pending migrations
        for migration in migrations {
            if migration.version() > current_version {
                info!(
                    "Applying migration {}: {}",
                    migration.version(),
                    migration.description()
                );
                migration.up(pool).await?;
                self.set_current_version(pool, migration.version()).await?;
                info!("Migration {} applied successfully", migration.version());
            }
        }

        Ok(())
    }

    async fn create_migrations_table(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY NOT NULL,
                applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    async fn get_current_version(&self, pool: &SqlitePool) -> Result<u32> {
        let row = sqlx::query("SELECT MAX(version) as version FROM schema_migrations")
            .fetch_optional(pool)
            .await?;

        if let Some(row) = row {
            Ok(row.get::<Option<i64>, _>("version").unwrap_or(0) as u32)
        } else {
            Ok(0)
        }
    }

    async fn set_current_version(&self, pool: &SqlitePool, version: u32) -> Result<()> {
        sqlx::query("INSERT INTO schema_migrations (version) VALUES (?)")
            .bind(version as i64)
            .execute(pool)
            .await?;

        Ok(())
    }
}

#[cfg(feature = "sqlite")]
impl Default for MigrationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Initial schema migration (version 1)
#[cfg(feature = "sqlite")]
pub struct InitialMigration;

#[cfg(feature = "sqlite")]
#[async_trait]
impl Migration for InitialMigration {
    fn version(&self) -> u32 {
        1
    }

    fn description(&self) -> &str {
        "Create initial schema"
    }

    async fn up(&self, pool: &SqlitePool) -> Result<()> {
        let mut tx = pool.begin().await?;

        // Create contacts table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS contacts (
                name TEXT PRIMARY KEY NOT NULL,
                pubkey TEXT NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&mut *tx)
        .await?;

        // Create channels table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS channels (
                name TEXT PRIMARY KEY NOT NULL,
                privkey TEXT NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&mut *tx)
        .await?;

        // Create profiles table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS profiles (
                name TEXT PRIMARY KEY NOT NULL,
                privkey TEXT NOT NULL,
                enabled BOOLEAN NOT NULL DEFAULT TRUE,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&mut *tx)
        .await?;

        // Create peers table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS peers (
                name TEXT PRIMARY KEY NOT NULL,
                uri TEXT NOT NULL,
                enabled BOOLEAN NOT NULL DEFAULT TRUE,
                peergroup TEXT NOT NULL DEFAULT '',
                pubkey TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&mut *tx)
        .await?;

        // Create config table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS config (
                name TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&mut *tx)
        .await?;

        // Create outbox table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS outbox (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel TEXT,
                msg BLOB NOT NULL,
                timestamp INTEGER NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&mut *tx)
        .await?;

        // Create index on outbox timestamp
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_outbox_timestamp ON outbox (timestamp)")
            .execute(&mut *tx)
            .await?;

        // Create streams table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS streams (
                streamid INTEGER PRIMARY KEY NOT NULL,
                parts INTEGER NOT NULL,
                channel TEXT NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&mut *tx)
        .await?;

        // Create chunks table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS chunks (
                streamid INTEGER NOT NULL,
                chunknum INTEGER NOT NULL,
                data BLOB NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (streamid, chunknum)
            )
            "#,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }
}

/// Get default migration manager with all built-in migrations
#[cfg(feature = "sqlite")]
pub fn default_migrations() -> MigrationManager {
    MigrationManager::new().add_migration(Box::new(InitialMigration))
}

#[cfg(not(feature = "sqlite"))]
pub trait Migration: Send + Sync {
    fn version(&self) -> u32;
    fn description(&self) -> &str;
    fn up(
        &self,
        _pool: &(),
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::error::Result<()>> + Send>> {
        Box::pin(async {
            Err(crate::error::RatNetError::Feature(
                "sqlite feature not enabled".to_string(),
            ))
        })
    }
    fn down(
        &self,
        _pool: &(),
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::error::Result<()>> + Send>> {
        Box::pin(async {
            Err(crate::error::RatNetError::Feature(
                "sqlite feature not enabled".to_string(),
            ))
        })
    }
}

#[cfg(not(feature = "sqlite"))]
pub struct MigrationManager;

#[cfg(not(feature = "sqlite"))]
impl MigrationManager {
    pub fn new() -> Self {
        Self
    }

    pub fn add_migration(self, _migration: Box<dyn Migration>) -> Self {
        self
    }

    pub async fn migrate(&self, _pool: &()) -> crate::error::Result<()> {
        Err(crate::error::RatNetError::Feature(
            "sqlite feature not enabled".to_string(),
        ))
    }
}

#[cfg(not(feature = "sqlite"))]
impl Default for MigrationManager {
    fn default() -> Self {
        Self
    }
}

#[cfg(not(feature = "sqlite"))]
pub fn default_migrations() -> MigrationManager {
    MigrationManager
}
