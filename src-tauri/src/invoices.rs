use crate::auth::require_active_user;
use crate::backend::{require_permission, AppState, CommandResult};
use crate::core::now_iso;
use crate::models::*;
use rusqlite::{params, OptionalExtension};
use std::env;
use tauri::State;



#[tauri::command]
pub(crate) fn tax_list(state: State<'_, AppState>, actor_id: i64) -> CommandResult<Vec<TaxOption>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, actor_id)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, type, rate, country, is_active FROM taxes ORDER BY rate DESC, name",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(TaxOption {
                id: row.get(0)?,
                name: row.get(1)?,
                tax_type: row.get(2)?,
                rate: row.get(3)?,
                country: row.get(4)?,
                is_active: row.get::<_, i64>(5)? == 1,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub(crate) fn pac_message() -> String {
    let has_provider = env::var("RIM_POS_PAC_PROVIDER")
        .ok()
        .filter(|value| !value.is_empty());
    let has_token = env::var("RIM_POS_PAC_API_KEY")
        .ok()
        .filter(|value| !value.is_empty());
    if has_provider.is_some() && has_token.is_some() {
        "Credenciales PAC detectadas. Falta implementar adaptador del proveedor contratado.".into()
    } else {
        "CFDI listo como borrador. Para timbrar en produccion se necesita contratar un PAC real y configurar RIM_POS_PAC_PROVIDER/RIM_POS_PAC_API_KEY.".into()
    }
}

#[tauri::command]
pub(crate) fn invoice_prepare(
    state: State<'_, AppState>,
    actor_id: i64,
    sale_id: i64,
    customer_id: Option<i64>,
) -> CommandResult<InvoiceDraft> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "invoices")?;
    let total: f64 = conn
        .query_row(
            "SELECT total FROM sales WHERE id = ?1 AND status = 'paid'",
            params![sale_id],
            |row| row.get(0),
        )
        .map_err(|_| "Venta pagada no encontrada".to_string())?;
    if let Some(id) = customer_id {
        let exists: Option<i64> = conn
            .query_row(
                "SELECT id FROM customers WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if exists.is_none() {
            return Err("Cliente fiscal no encontrado".into());
        }
    }
    let now = now_iso();
    conn.execute(
        "INSERT INTO invoices_stub (sale_id, customer_id, status, error_message, created_at)
         VALUES (?1, ?2, 'draft', ?3, ?4)",
        params![sale_id, customer_id, pac_message(), now],
    )
    .map_err(|error| error.to_string())?;
    let id = conn.last_insert_rowid();
    Ok(InvoiceDraft {
        id,
        sale_id: Some(sale_id),
        customer_id,
        customer_name: customer_id.and_then(|id| {
            conn.query_row(
                "SELECT name FROM customers WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()
            .ok()
            .flatten()
        }),
        folio: format!("PRE-{id:06}"),
        status: "draft".into(),
        total,
        pac_message: pac_message(),
        created_at: now,
    })
}

#[tauri::command]
pub(crate) fn invoice_list(state: State<'_, AppState>, actor_id: i64) -> CommandResult<Vec<InvoiceDraft>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "invoices")?;
    let mut stmt = conn
        .prepare(
            "SELECT i.id, i.sale_id, i.customer_id, c.name, i.status, s.total, COALESCE(i.error_message, ''), i.created_at
             FROM invoices_stub i
             JOIN sales s ON s.id = i.sale_id
             LEFT JOIN customers c ON c.id = i.customer_id
             ORDER BY i.id DESC
             LIMIT 80",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let id: i64 = row.get(0)?;
            Ok(InvoiceDraft {
                id,
                sale_id: row.get(1)?,
                customer_id: row.get(2)?,
                customer_name: row.get(3)?,
                folio: format!("PRE-{id:06}"),
                status: row.get(4)?,
                total: row.get(5)?,
                pac_message: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}
