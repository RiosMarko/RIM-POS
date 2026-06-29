use crate::core::now_iso;
use rusqlite::Connection;
use serde::Serialize;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

type CommandResult<T> = Result<T, String>;

const BACKUP_RETENTION_LIMIT: usize = 10;

#[derive(Debug, Serialize)]
pub struct BackupResult {
    pub path: String,
    pub created_at: String,
}

#[cfg(unix)]
fn harden_backup_permissions(path: &PathBuf, mode: u32) -> CommandResult<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn harden_backup_permissions(_path: &PathBuf, _mode: u32) -> CommandResult<()> {
    Ok(())
}

fn prune_old_backups(backup_dir: &PathBuf) -> CommandResult<()> {
    let mut backups = fs::read_dir(backup_dir)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let file_name = path.file_name()?.to_string_lossy();
            let is_backup = file_name.starts_with("pos-backup-") && file_name.ends_with(".sqlite3");
            if path.is_file() && is_backup {
                Some(path)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    backups.sort();
    let remove_count = backups.len().saturating_sub(BACKUP_RETENTION_LIMIT);
    for path in backups.into_iter().take(remove_count) {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn create_backup_file(db_path: &PathBuf) -> CommandResult<BackupResult> {
    let backup_dir = db_path
        .parent()
        .ok_or_else(|| "Ruta de base invalida".to_string())?
        .join("backups");
    fs::create_dir_all(&backup_dir).map_err(|error| error.to_string())?;
    harden_backup_permissions(&backup_dir, 0o700)?;
    let created_at = now_iso();
    let safe_stamp = created_at.replace(':', "-");
    let backup_path = backup_dir.join(format!("pos-backup-{safe_stamp}.sqlite3"));
    fs::copy(db_path, &backup_path).map_err(|error| error.to_string())?;
    harden_backup_permissions(&backup_path, 0o600)?;
    prune_old_backups(&backup_dir)?;
    Ok(BackupResult {
        path: backup_path.to_string_lossy().to_string(),
        created_at,
    })
}

pub fn backup_create_with_conn(
    conn: &Connection,
    db_path: &PathBuf,
) -> CommandResult<BackupResult> {
    conn.execute_batch("PRAGMA wal_checkpoint(FULL);")
        .map_err(|error| error.to_string())?;
    create_backup_file(db_path)
}
