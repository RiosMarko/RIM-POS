use crate::backend::{require_permission, AppState, CommandResult};
use crate::core::{now_iso, round_money};
use crate::models::*;
use crate::validation::{validate_non_negative, validate_positive, validate_required_text};
use rusqlite::{params, OptionalExtension};
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
        .prepare("SELECT id, name, phone, contact, created_at FROM suppliers ORDER BY name")
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], map_supplier)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
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

#[tauri::command]
pub(crate) fn purchase_create(
    state: State<'_, AppState>,
    actor_id: i64,
    input: PurchaseInput,
) -> CommandResult<PurchaseReceipt> {
    if input.product_id <= 0 {
        return Err("Compra invalida".into());
    }
    validate_positive(input.quantity, "Compra invalida")?;
    validate_non_negative(input.unit_cost, "Compra invalida")?;
    let mut conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "purchases")?;
    let tx = conn.transaction().map_err(|error| error.to_string())?;
    let product_name: String = tx
        .query_row(
            "SELECT name FROM products WHERE id = ?1 AND active = 1",
            params![input.product_id],
            |row| row.get(0),
        )
        .map_err(|_| "Producto no disponible".to_string())?;
    if let Some(supplier_id) = input.supplier_id {
        let exists: Option<i64> = tx
            .query_row(
                "SELECT id FROM suppliers WHERE id = ?1",
                params![supplier_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if exists.is_none() {
            return Err("Proveedor no encontrado".into());
        }
    }
    let total = round_money(input.quantity * input.unit_cost);
    let now = now_iso();
    tx.execute(
        "INSERT INTO purchases (supplier_id, total, status, note, user_id, created_at)
         VALUES (?1, ?2, 'completed', ?3, ?4, ?5)",
        params![
            input.supplier_id,
            total,
            input.note.as_deref().map(str::trim),
            input.user_id,
            now
        ],
    )
    .map_err(|error| error.to_string())?;
    let purchase_id = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO purchase_items (purchase_id, product_id, quantity, unit_cost, line_total)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            purchase_id,
            input.product_id,
            input.quantity,
            input.unit_cost,
            total
        ],
    )
    .map_err(|error| error.to_string())?;
    tx.execute(
        "UPDATE products SET stock = stock + ?1, cost = ?2, updated_at = ?3 WHERE id = ?4",
        params![input.quantity, input.unit_cost, now, input.product_id],
    )
    .map_err(|error| error.to_string())?;
    tx.execute(
        "INSERT INTO inventory_movements (product_id, movement_type, quantity, reason, reference_id, created_at)
         VALUES (?1, 'purchase', ?2, ?3, ?4, ?5)",
        params![
            input.product_id,
            input.quantity,
            input.note.as_deref().unwrap_or("Compra"),
            purchase_id,
            now
        ],
    )
    .map_err(|error| error.to_string())?;
    tx.execute(
        "INSERT INTO price_history (product_id, price, recorded_at) VALUES (?1, ?2, ?3)",
        params![input.product_id, input.unit_cost, now],
    )
    .map_err(|error| error.to_string())?;
    if let Some(supplier_id) = input.supplier_id {
        tx.execute(
            "INSERT INTO supplier_products (supplier_id, product_id, supplier_price, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(supplier_id, product_id) DO UPDATE
             SET supplier_price = excluded.supplier_price, updated_at = excluded.updated_at",
            params![supplier_id, input.product_id, input.unit_cost, now],
        )
        .map_err(|error| error.to_string())?;
    }
    tx.commit().map_err(|error| error.to_string())?;
    let supplier_name: Option<String> = match input.supplier_id {
        Some(supplier_id) => conn
            .query_row(
                "SELECT name FROM suppliers WHERE id = ?1",
                params![supplier_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?,
        None => None,
    };
    Ok(PurchaseReceipt {
        id: purchase_id,
        supplier_name,
        product_name,
        quantity: input.quantity,
        unit_cost: input.unit_cost,
        total,
        created_at: now,
    })
}

#[tauri::command]
pub(crate) fn purchase_list(state: State<'_, AppState>, actor_id: i64) -> CommandResult<Vec<PurchaseReceipt>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "purchases")?;
    let mut stmt = conn
        .prepare(
            "SELECT pu.id, s.name, p.name, pi.quantity, pi.unit_cost, pi.line_total, pu.created_at
             FROM purchases pu
             JOIN purchase_items pi ON pi.purchase_id = pu.id
             JOIN products p ON p.id = pi.product_id
             LEFT JOIN suppliers s ON s.id = pu.supplier_id
             ORDER BY pu.id DESC
             LIMIT 80",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(PurchaseReceipt {
                id: row.get(0)?,
                supplier_name: row.get(1)?,
                product_name: row.get(2)?,
                quantity: row.get(3)?,
                unit_cost: row.get(4)?,
                total: row.get(5)?,
                created_at: row.get(6)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}
