use rusqlite::{params, Connection};
use serde::Serialize;

type CommandResult<T> = Result<T, String>;

#[derive(Debug, Serialize)]
pub struct UserSession {
    pub id: i64,
    pub name: String,
    pub role: String,
    pub permissions: Vec<String>,
}

pub fn require_active_user(conn: &Connection, actor_id: i64) -> CommandResult<UserSession> {
    conn.query_row(
        "SELECT id, name, role FROM users WHERE id = ?1 AND active = 1",
        params![actor_id],
        |row| {
            Ok(UserSession {
                id: row.get(0)?,
                name: row.get(1)?,
                role: row.get(2)?,
                permissions: Vec::new(),
            })
        },
    )
    .map_err(|_| "Usuario no autorizado".to_string())
}

pub fn require_admin(conn: &Connection, actor_id: i64) -> CommandResult<()> {
    let actor = require_active_user(conn, actor_id)?;
    if actor.role != "admin" {
        return Err("Permiso admin requerido".into());
    }
    Ok(())
}

pub fn ensure_admin_remains(
    conn: &Connection,
    changing_user_id: i64,
    next_role: &str,
    next_active: bool,
) -> CommandResult<()> {
    let current_role: String = conn
        .query_row(
            "SELECT role FROM users WHERE id = ?1",
            params![changing_user_id],
            |row| row.get(0),
        )
        .map_err(|_| "Usuario no encontrado".to_string())?;
    if current_role == "admin" && (next_role != "admin" || !next_active) {
        let remaining_admins: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM users WHERE id <> ?1 AND role = 'admin' AND active = 1",
                params![changing_user_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if remaining_admins == 0 {
            return Err("Debe quedar al menos un admin activo".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ensure_admin_remains, require_active_user, require_admin};
    use rusqlite::{params, Connection};

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE users (
              id INTEGER PRIMARY KEY,
              name TEXT NOT NULL,
              role TEXT NOT NULL,
              active INTEGER NOT NULL
            );
            ",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO users (id, name, role, active) VALUES (?1, ?2, ?3, ?4)",
            params![1, "Admin", "admin", 1],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO users (id, name, role, active) VALUES (?1, ?2, ?3, ?4)",
            params![2, "Cajera", "cashier", 1],
        )
        .unwrap();
        conn
    }

    #[test]
    fn require_active_user_rejects_missing_user() {
        let conn = test_conn();
        assert!(require_active_user(&conn, 1).is_ok());
        assert!(require_active_user(&conn, 999).is_err());
    }

    #[test]
    fn require_admin_rejects_cashier() {
        let conn = test_conn();
        assert!(require_admin(&conn, 1).is_ok());
        assert_eq!(
            require_admin(&conn, 2).unwrap_err(),
            "Permiso admin requerido"
        );
    }

    #[test]
    fn ensure_admin_remains_blocks_last_admin_removal() {
        let conn = test_conn();
        assert_eq!(
            ensure_admin_remains(&conn, 1, "cashier", true).unwrap_err(),
            "Debe quedar al menos un admin activo",
        );
        conn.execute(
            "INSERT INTO users (id, name, role, active) VALUES (?1, ?2, ?3, ?4)",
            params![3, "Admin 2", "admin", 1],
        )
        .unwrap();
        assert!(ensure_admin_remains(&conn, 1, "cashier", true).is_ok());
    }
}
