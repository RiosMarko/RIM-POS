use crate::core::now_iso;
use chrono::{DateTime, Datelike, Utc};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

type CommandResult<T> = Result<T, String>;

const BACKUP_KEEP_DAILY: usize = 14;
const BACKUP_KEEP_WEEKLY: usize = 8;
const BACKUP_KEEP_MONTHLY: usize = 12;

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
                let modified = entry
                    .metadata()
                    .ok()
                    .and_then(|metadata| metadata.modified().ok())
                    .map(DateTime::<Utc>::from)?;
                Some((path, modified))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    backups.sort_by(|left, right| right.1.cmp(&left.1));
    let mut keep = HashSet::new();
    for (path, _) in backups.iter().take(BACKUP_KEEP_DAILY) {
        keep.insert(path.clone());
    }
    let mut weekly = HashSet::new();
    let mut monthly = HashSet::new();
    for (path, created_at) in &backups {
        let iso_week = created_at.iso_week();
        let week_key = format!("{}-{:02}", iso_week.year(), iso_week.week());
        if weekly.len() < BACKUP_KEEP_WEEKLY && weekly.insert(week_key) {
            keep.insert(path.clone());
        }
        let month_key = format!("{}-{:02}", created_at.year(), created_at.month());
        if monthly.len() < BACKUP_KEEP_MONTHLY && monthly.insert(month_key) {
            keep.insert(path.clone());
        }
    }
    for (path, _) in backups {
        if keep.contains(&path) {
            continue;
        }
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn verify_sqlite_backup(path: &PathBuf) -> CommandResult<()> {
    let conn = Connection::open(path).map_err(|error| format!("Backup no abre: {error}"))?;
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| format!("Backup no se pudo validar: {error}"))?;
    if integrity == "ok" {
        Ok(())
    } else {
        Err(format!("Backup dañado: {integrity}"))
    }
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
    verify_sqlite_backup(&backup_path)?;
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
