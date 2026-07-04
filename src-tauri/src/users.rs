use crate::auth::{ensure_admin_remains, require_admin, UserSession};
use crate::backend::{AppState, CommandResult};
use crate::core::now_iso;
use crate::models::*;
use crate::security::{hash_pin, verify_pin};
use crate::validation::{validate_pin, validate_required_text};
use rusqlite::{params, Connection, OptionalExtension};
use tauri::State;

pub(crate) fn map_user(row: &rusqlite::Row<'_>) -> rusqlite::Result<UserAccount> {
    Ok(UserAccount {
        id: row.get(0)?,
        name: row.get(1)?,
        role: row.get(2)?,
        active: row.get::<_, i64>(3)? == 1,
        created_at: row.get(4)?,
        permissions: Vec::new(),
    })
}

pub(crate) fn all_user_permissions() -> Vec<String> {
    USER_PERMISSION_KEYS
        .iter()
        .map(|permission| permission.to_string())
        .collect()
}

pub(crate) fn normalize_permissions(role: &str, permissions: &[String]) -> Vec<String> {
    if role == "admin" {
        return all_user_permissions();
    }
    let mut normalized = Vec::new();
    for permission in permissions {
        let permission = permission.trim();
        if USER_PERMISSION_KEYS.contains(&permission)
            && !normalized.iter().any(|item| item == permission)
        {
            normalized.push(permission.to_string());
        }
    }
    normalized
}

pub(crate) fn load_user_permissions(
    conn: &Connection,
    user_id: i64,
    role: &str,
) -> CommandResult<Vec<String>> {
    if role == "admin" {
        return Ok(all_user_permissions());
    }
    let mut stmt = conn
        .prepare(
            "SELECT permission_key FROM user_permissions
             WHERE user_id = ?1
             ORDER BY permission_key",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![user_id], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub(crate) fn hydrate_user_permissions(conn: &Connection, user: &mut UserAccount) -> CommandResult<()> {
    user.permissions = load_user_permissions(conn, user.id, &user.role)?;
    Ok(())
}

pub(crate) fn save_user_permissions(
    conn: &Connection,
    user_id: i64,
    role: &str,
    permissions: &[String],
) -> CommandResult<()> {
    conn.execute(
        "DELETE FROM user_permissions WHERE user_id = ?1",
        params![user_id],
    )
    .map_err(|error| error.to_string())?;
    if role == "admin" {
        return Ok(());
    }
    for permission in normalize_permissions(role, permissions) {
        conn.execute(
            "INSERT OR IGNORE INTO user_permissions (user_id, permission_key) VALUES (?1, ?2)",
            params![user_id, permission],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}


#[tauri::command]
pub(crate) fn auth_needs_setup(state: State<'_, AppState>) -> CommandResult<bool> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    let active_users: i64 = conn
        .query_row("SELECT COUNT(*) FROM users WHERE active = 1", [], |row| {
            row.get(0)
        })
        .map_err(|error| error.to_string())?;
    Ok(active_users == 0)
}

#[tauri::command]
pub(crate) fn auth_create_initial_admin(
    state: State<'_, AppState>,
    input: InitialAdminInput,
) -> CommandResult<UserSession> {
    let name = input.name.trim();
    let pin = input.pin.trim();
    validate_required_text(name, 2, "Nombre muy corto")?;
    validate_pin(pin, 4, "Contraseña inicial")?;
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    let active_users: i64 = conn
        .query_row("SELECT COUNT(*) FROM users WHERE active = 1", [], |row| {
            row.get(0)
        })
        .map_err(|error| error.to_string())?;
    if active_users > 0 {
        return Err("Configuracion inicial ya fue completada".into());
    }
    conn.execute(
        "INSERT INTO users (name, pin_hash, role, active, created_at) VALUES (?1, ?2, 'admin', 1, ?3)",
        params![name, hash_pin(pin)?, now_iso()],
    )
    .map_err(|error| error.to_string())?;
    let id = conn.last_insert_rowid();
    Ok(UserSession {
        id,
        name: name.to_string(),
        role: "admin".into(),
        permissions: all_user_permissions(),
    })
}

#[tauri::command]
pub(crate) fn auth_login(state: State<'_, AppState>, input: LoginInput) -> CommandResult<UserSession> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, role, pin_hash FROM users WHERE lower(name) = lower(?1) AND active = 1 LIMIT 1")
        .map_err(|error| error.to_string())?;
    let user: Option<(i64, String, String, String)> = stmt
        .query_row(params![input.name.trim()], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((id, name, role, pin_hash)) = user else {
        return Err("Usuario o contraseña incorrectos".into());
    };
    if !verify_pin(&pin_hash, &input.pin) {
        return Err("Usuario o contraseña incorrectos".into());
    }
    if !pin_hash.starts_with("$argon2") {
        conn.execute(
            "UPDATE users SET pin_hash = ?1 WHERE id = ?2",
            params![hash_pin(&input.pin)?, id],
        )
        .map_err(|error| error.to_string())?;
    }
    let permissions = load_user_permissions(&conn, id, &role)?;
    Ok(UserSession {
        id,
        name,
        role,
        permissions,
    })
}

#[tauri::command]
pub(crate) fn user_list(state: State<'_, AppState>, actor_id: i64) -> CommandResult<Vec<UserAccount>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_admin(&conn, actor_id)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, role, active, created_at FROM users ORDER BY active DESC, role, name",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], map_user)
        .map_err(|error| error.to_string())?;
    let mut users = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    for user in &mut users {
        hydrate_user_permissions(&conn, user)?;
    }
    Ok(users)
}

#[tauri::command]
pub(crate) fn user_create(
    state: State<'_, AppState>,
    actor_id: i64,
    input: UserCreateInput,
) -> CommandResult<UserAccount> {
    let name = input.name.trim();
    let pin = input.pin.trim();
    validate_required_text(name, 2, "Nombre muy corto")?;
    validate_pin(pin, 4, "Contraseña")?;
    let role = match input.role.as_str() {
        "admin" => "admin",
        _ => "cashier",
    };
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_admin(&conn, actor_id)?;
    conn.execute(
        "INSERT INTO users (name, pin_hash, role, active, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            name,
            hash_pin(pin)?,
            role,
            if input.active { 1 } else { 0 },
            now_iso()
        ],
    )
    .map_err(|error| {
        if error.to_string().contains("UNIQUE") {
            "Ya existe usuario con ese nombre".into()
        } else {
            error.to_string()
        }
    })?;
    let id = conn.last_insert_rowid();
    save_user_permissions(&conn, id, role, &input.permissions)?;
    let mut user = conn
        .query_row(
            "SELECT id, name, role, active, created_at FROM users WHERE id = ?1",
            params![id],
            map_user,
        )
        .map_err(|error| error.to_string())?;
    hydrate_user_permissions(&conn, &mut user)?;
    Ok(user)
}

#[tauri::command]
pub(crate) fn user_update(
    state: State<'_, AppState>,
    actor_id: i64,
    input: UserUpdateInput,
) -> CommandResult<UserAccount> {
    let name = input.name.trim();
    validate_required_text(name, 2, "Nombre muy corto")?;
    let role = match input.role.as_str() {
        "admin" => "admin",
        _ => "cashier",
    };
    let pin = input
        .pin
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(pin) = pin {
        validate_pin(pin, 4, "Contraseña")?;
    }
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_admin(&conn, actor_id)?;
    ensure_admin_remains(&conn, input.id, role, input.active)?;
    if let Some(pin) = pin {
        conn.execute(
            "UPDATE users SET name = ?1, pin_hash = ?2, role = ?3, active = ?4 WHERE id = ?5",
            params![
                name,
                hash_pin(pin)?,
                role,
                if input.active { 1 } else { 0 },
                input.id
            ],
        )
    } else {
        conn.execute(
            "UPDATE users SET name = ?1, role = ?2, active = ?3 WHERE id = ?4",
            params![name, role, if input.active { 1 } else { 0 }, input.id],
        )
    }
    .map_err(|error| {
        if error.to_string().contains("UNIQUE") {
            "Ya existe usuario con ese nombre".into()
        } else {
            error.to_string()
        }
    })?;
    if conn.changes() == 0 {
        return Err("Usuario no encontrado".into());
    }
    save_user_permissions(&conn, input.id, role, &input.permissions)?;
    let mut user = conn
        .query_row(
            "SELECT id, name, role, active, created_at FROM users WHERE id = ?1",
            params![input.id],
            map_user,
        )
        .map_err(|error| error.to_string())?;
    hydrate_user_permissions(&conn, &mut user)?;
    Ok(user)
}

#[tauri::command]
pub(crate) fn user_delete(state: State<'_, AppState>, actor_id: i64, id: i64) -> CommandResult<()> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_admin(&conn, actor_id)?;
    let current_role: String = conn
        .query_row("SELECT role FROM users WHERE id = ?1", params![id], |row| {
            row.get(0)
        })
        .map_err(|_| "Usuario no encontrado".to_string())?;
    ensure_admin_remains(&conn, id, &current_role, false)?;
    conn.execute("UPDATE users SET active = 0 WHERE id = ?1", params![id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

