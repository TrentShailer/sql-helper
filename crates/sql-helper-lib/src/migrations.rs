//! Helpers for running migrations
//!

use std::{
    env::current_dir,
    ffi::OsStr,
    fs::{self, DirEntry},
    io,
    path::PathBuf,
};

/// Runs the migrations in `current_dir()/migrations/*.sql` on the client, migrations are executed
/// in name order.
pub fn perform_migrations(
    client: &mut postgres::Client,
    migrations_directory: Option<PathBuf>,
) -> Result<(), MigrationError> {
    let Some(entries) = get_migration_targets(migrations_directory)? else {
        return Ok(());
    };

    for entry in entries {
        let sql = fs::read_to_string(entry.path())
            .map_err(|source| MigrationError::ReadMigrationFile { source })?;
        client
            .batch_execute(&sql)
            .map_err(|source| MigrationError::ExecuteMigration { source, sql })?;
    }

    Ok(())
}

#[cfg(feature = "async")]
/// Runs the migrations in `current_dir()/migrations/*.sql` on the client, migrations are executed
/// in name order.
pub async fn perform_migrations_async(
    client: &tokio_postgres::Client,
    migrations_directory: Option<PathBuf>,
) -> Result<(), MigrationError> {
    let Some(entries) = get_migration_targets(migrations_directory)? else {
        return Ok(());
    };

    for entry in entries {
        let sql = fs::read_to_string(entry.path())
            .map_err(|source| MigrationError::ReadMigrationFile { source })?;
        client
            .batch_execute(&sql)
            .await
            .map_err(|source| MigrationError::ExecuteMigration { source, sql })?;
    }

    Ok(())
}

fn get_migration_targets(
    migrations_directory: Option<PathBuf>,
) -> Result<Option<Vec<DirEntry>>, MigrationError> {
    let path = match migrations_directory {
        Some(path) => path,
        None => {
            let Ok(current_dir) = current_dir() else {
                return Ok(None);
            };
            current_dir.join("migrations")
        }
    };

    if !fs::exists(&path).unwrap() {
        return Ok(None);
    }

    let directory =
        fs::read_dir(&path).map_err(|source| MigrationError::ReadMigrationDirectory { source })?;
    let mut entries: Vec<_> = directory
        .filter_map(|entry| match entry {
            Ok(entry) => {
                if entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == OsStr::new("sql"))
                {
                    Some(Ok(entry))
                } else {
                    None
                }
            }
            Err(error) => Some(Err(error)),
        })
        .collect::<Result<_, _>>()
        .map_err(|source| MigrationError::ReadMigrationFile { source })?;
    entries.sort_by_key(|entry| entry.file_name());

    Ok(Some(entries))
}

/// Error variants for migrating a database.
#[derive(Debug)]
#[non_exhaustive]
#[allow(missing_docs)]
pub enum MigrationError {
    #[non_exhaustive]
    ReadMigrationDirectory { source: io::Error },

    #[non_exhaustive]
    ReadMigrationFile { source: io::Error },

    #[non_exhaustive]
    ExecuteMigration {
        source: postgres::Error,
        sql: String,
    },
}
impl core::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self {
            Self::ReadMigrationDirectory { .. } => write!(f, "could not read migration directory"),
            Self::ReadMigrationFile { .. } => write!(f, "could not read a migration file"),
            Self::ExecuteMigration { sql, .. } => write!(f, "migration `{sql}` failed to execute"),
        }
    }
}
impl core::error::Error for MigrationError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match &self {
            Self::ReadMigrationDirectory { source, .. } => Some(source),
            Self::ReadMigrationFile { source, .. } => Some(source),
            Self::ExecuteMigration { source, .. } => Some(source),
        }
    }
}
