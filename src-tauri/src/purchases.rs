use crate::backend::{require_permission, AppState, CommandResult};
use crate::core::now_iso;
use crate::models::*;
use crate::validation::validate_required_text;
use rusqlite::params;
use tauri::State;

pub(crate) fn map_supplier(row: &rusqlite::Row<'_>) -> rusqlite::Result<Supplier> {
    Ok(Supplier {
        id: row.get(0)?,
        name: row.get(1)?,
        phone: row.get(2)?,
        contact: row.get(3)?,
        created_at: row.get(4)?,
    })
}

#[tauri::command]
pub(crate) fn supplier_list(state: State<'_, AppState>, actor_id: i64) -> CommandResult<Vec<Supplier>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "purchases")?;
    let mut stmt = conn
        .prepare("SELECT id, name, phone, contact, created_at FROM suppliers WHERE active = 1 ORDER BY name")
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], map_supplier)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn supplier_delete(state: State<'_, AppState>, actor_id: i64, id: i64) -> CommandResult<()> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "purchases")?;
    let name: String = conn
        .query_row("SELECT name FROM suppliers WHERE id = ?1", params![id], |row| row.get(0))
        .map_err(|_| "Proveedor no encontrado".to_string())?;
    conn.execute("UPDATE suppliers SET active = 0 WHERE id = ?1", params![id])
        .map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT INTO audit_log (actor_id, action, entity, entity_id, details, created_at)
         VALUES (?1, 'supplier_delete', 'supplier', ?2, ?3, ?4)",
        params![actor_id, id, name, now_iso()],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) fn supplier_upsert(
    state: State<'_, AppState>,
    actor_id: i64,
    input: SupplierInput,
) -> CommandResult<Supplier> {
    let name = input.name.trim();
    validate_required_text(name, 2, "Proveedor requerido")?;
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "purchases")?;
    let id = match input.id {
        Some(id) => {
            conn.execute(
                "UPDATE suppliers SET name = ?1, phone = ?2, contact = ?3 WHERE id = ?4",
                params![
                    name,
                    input.phone.as_deref().map(str::trim),
                    input.contact.as_deref().map(str::trim),
                    id
                ],
            )
            .map_err(|error| error.to_string())?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO suppliers (name, phone, contact, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![
                    name,
                    input.phone.as_deref().map(str::trim),
                    input.contact.as_deref().map(str::trim),
                    now_iso()
                ],
            )
            .map_err(|error| error.to_string())?;
            conn.last_insert_rowid()
        }
    };
    conn.query_row(
        "SELECT id, name, phone, contact, created_at FROM suppliers WHERE id = ?1",
        params![id],
        map_supplier,
    )
    .map_err(|error| error.to_string())
}
