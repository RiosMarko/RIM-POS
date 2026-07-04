use crate::backend::{current_workstation_id, require_permission, AppState, CommandResult};
use crate::cash::{get_open_shift, payment_method_counts_as_cash};
use crate::core::now_iso;
use crate::models::*;
use crate::validation::{validate_non_negative, validate_optional_email, validate_optional_rfc, validate_required_text};
use chrono::Utc;
use rusqlite::{params, Connection};
use tauri::State;

pub(crate) fn map_customer(row: &rusqlite::Row<'_>) -> rusqlite::Result<Customer> {
    Ok(Customer {
        id: row.get(0)?,
        name: row.get(1)?,
        rfc: row.get(2)?,
        phone: row.get(3)?,
        email: row.get(4)?,
        credit_limit: row.get(5)?,
        balance: row.get(6)?,
        created_at: row.get(7)?,
    })
}

#[tauri::command]
pub(crate) fn customer_list(state: State<'_, AppState>, actor_id: i64) -> CommandResult<Vec<Customer>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    if require_permission(&conn, actor_id, "customers").is_err() {
        require_permission(&conn, actor_id, "invoices")?;
    }
    let mut stmt = conn
        .prepare(
            "SELECT id, name, rfc, phone, email, credit_limit, balance, created_at
             FROM customers ORDER BY name",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], map_customer)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn customer_upsert(
    state: State<'_, AppState>,
    actor_id: i64,
    input: CustomerInput,
) -> CommandResult<Customer> {
    let name = input.name.trim();
    validate_required_text(name, 2, "Nombre de cliente requerido")?;
    validate_non_negative(input.credit_limit, "Limite de credito invalido")?;
    validate_optional_rfc(input.rfc.as_deref())?;
    validate_optional_email(input.email.as_deref())?;
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "customers")?;
    let id = match input.id {
        Some(id) => {
            conn.execute(
                "UPDATE customers SET name = ?1, rfc = ?2, phone = ?3, email = ?4, credit_limit = ?5 WHERE id = ?6",
                params![
                    name,
                    input.rfc.as_deref().map(str::trim),
                    input.phone.as_deref().map(str::trim),
                    input.email.as_deref().map(str::trim),
                    input.credit_limit,
                    id
                ],
            )
            .map_err(|error| error.to_string())?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO customers (name, rfc, phone, email, credit_limit, balance, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
                params![
                    name,
                    input.rfc.as_deref().map(str::trim),
                    input.phone.as_deref().map(str::trim),
                    input.email.as_deref().map(str::trim),
                    input.credit_limit,
                    now_iso()
                ],
            )
            .map_err(|error| error.to_string())?;
            conn.last_insert_rowid()
        }
    };
    conn.query_row(
        "SELECT id, name, rfc, phone, email, credit_limit, balance, created_at FROM customers WHERE id = ?1",
        params![id],
        map_customer,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn customer_credit_adjust(
    state: State<'_, AppState>,
    actor_id: i64,
    input: CustomerCreditInput,
) -> CommandResult<Customer> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    customer_credit_adjust_with_conn(&conn, actor_id, input)
}

pub(crate) fn customer_credit_adjust_with_conn(
    conn: &Connection,
    actor_id: i64,
    input: CustomerCreditInput,
) -> CommandResult<Customer> {
    if !input.amount.is_finite() || input.amount == 0.0 || input.reason.trim().len() < 2 {
        return Err("Movimiento de credito invalido".into());
    }
    require_permission(conn, actor_id, "customers")?;
    let now = Utc::now().to_rfc3339();
    let movement_kind = if input.amount < 0.0 { "payment" } else { "charge" };
    let payment_method = input
        .payment_method
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    let open_shift = if movement_kind == "payment" {
        let workstation_id = current_workstation_id(&conn)?;
        get_open_shift(&conn, &workstation_id)?
    } else {
        None
    };
    let cash_session_id = open_shift.map(|(_, session_id)| session_id);
    conn.execute(
        "UPDATE customers SET balance = balance + ?1 WHERE id = ?2",
        params![input.amount, input.customer_id],
    )
    .map_err(|error| error.to_string())?;
    // Cash abonos put money back in the drawer, so the running expected-cash
    // total (used by Arqueo and the close-shift dialog) must reflect it too,
    // matching the live formula in calculate_shift_cut.
    if let Some((shift_id, session_id)) = open_shift {
        if payment_method_counts_as_cash(payment_method.as_deref().unwrap_or("cash")) {
            let cash_amount = -input.amount;
            conn.execute(
                "UPDATE cash_sessions SET expected_cash = expected_cash + ?1 WHERE id = ?2 AND status = 'open'",
                params![cash_amount, session_id],
            )
            .map_err(|error| error.to_string())?;
            conn.execute(
                "UPDATE shifts SET expected_cash = expected_cash + ?1 WHERE id = ?2 AND status = 'open'",
                params![cash_amount, shift_id],
            )
            .map_err(|error| error.to_string())?;
        }
    }
    conn.execute(
        "INSERT INTO customer_credit_movements
         (customer_id, amount, reason, created_at, movement_kind, payment_method, actor_id, cash_session_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            input.customer_id,
            input.amount,
            input.reason.trim(),
            now,
            movement_kind,
            payment_method,
            actor_id,
            cash_session_id
        ],
    )
    .map_err(|error| error.to_string())?;
    conn.query_row(
        "SELECT id, name, rfc, phone, email, credit_limit, balance, created_at FROM customers WHERE id = ?1",
        params![input.customer_id],
        map_customer,
    )
    .map_err(|error| error.to_string())
}
