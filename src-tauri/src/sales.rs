use crate::auth::{require_active_user, require_admin};
use crate::backend::{current_workstation_id, line_amounts, setting_bool, AppState, CommandResult};
use crate::cash::get_open_shift;
use crate::core::{next_monthly_seq, now_iso, period_key, round_money, visible_monthly_folio};
use crate::models::*;
use crate::validation::{validate_non_negative, validate_positive};
use rusqlite::{params, types::Type, Connection, OptionalExtension};
use tauri::State;

pub(crate) fn validate_held_ticket_input(
    input: &HeldTicketInput,
    prices_include_tax: bool,
    tax_enabled: bool,
) -> CommandResult<(String, i64, f64)> {
    let name = input.name.trim();
    if name.len() < 2 {
        return Err("Nombre de ticket muy corto".into());
    }
    if input.items.is_empty() {
        return Err("Ticket sin articulos".into());
    }

    let mut item_count = 0_i64;
    let mut total = 0.0;
    for item in &input.items {
        if item.product_id <= 0 {
            return Err("Producto invalido".into());
        }
        if item.quantity <= 0.0 {
            return Err("Cantidad invalida".into());
        }
        if item.unit_price < 0.0 || item.discount < 0.0 || item.tax_rate < 0.0 {
            return Err("Importe invalido".into());
        }
        let base = item.quantity * item.unit_price;
        if item.discount > base {
            return Err("Descuento mayor al importe".into());
        }
        item_count += 1;
        let (_, _, line_total) = line_amounts(
            base,
            item.discount,
            item.tax_rate,
            prices_include_tax,
            tax_enabled,
        );
        total += line_total;
    }

    Ok((name.to_string(), item_count, round_money(total)))
}

pub(crate) fn validate_active_sale_draft_input(
    input: &ActiveSaleDraftInput,
    prices_include_tax: bool,
    tax_enabled: bool,
) -> CommandResult<(i64, f64)> {
    if input.cashier_id <= 0 {
        return Err("Cajero invalido".into());
    }
    if input.items.is_empty() {
        return Err("Borrador sin articulos".into());
    }
    if input.cash_received < 0.0 || input.card_received < 0.0 || input.transfer_received < 0.0 {
        return Err("Pago invalido".into());
    }

    let mut item_count = 0_i64;
    let mut total = 0.0;
    for item in &input.items {
        if item.product_id <= 0 {
            return Err("Producto invalido".into());
        }
        if item.quantity <= 0.0 {
            return Err("Cantidad invalida".into());
        }
        if item.unit_price < 0.0 || item.discount < 0.0 || item.tax_rate < 0.0 {
            return Err("Importe invalido".into());
        }
        let base = item.quantity * item.unit_price;
        if item.discount > base {
            return Err("Descuento mayor al importe".into());
        }
        item_count += 1;
        let (_, _, line_total) = line_amounts(
            base,
            item.discount,
            item.tax_rate,
            prices_include_tax,
            tax_enabled,
        );
        total += line_total;
    }

    Ok((item_count, round_money(total)))
}

pub(crate) fn map_held_ticket(row: &rusqlite::Row<'_>) -> rusqlite::Result<HeldTicket> {
    let items_json: String = row.get(5)?;
    let items = serde_json::from_str::<Vec<HeldTicketItem>>(&items_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(5, Type::Text, Box::new(error))
    })?;
    Ok(HeldTicket {
        id: row.get(0)?,
        name: row.get(1)?,
        cashier_id: row.get(2)?,
        cashier_name: row.get(3)?,
        item_count: row.get(4)?,
        items,
        total: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

pub(crate) fn map_active_sale_draft(row: &rusqlite::Row<'_>) -> rusqlite::Result<ActiveSaleDraft> {
    let items_json: String = row.get(2)?;
    let items = serde_json::from_str::<Vec<HeldTicketItem>>(&items_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(2, Type::Text, Box::new(error))
    })?;
    Ok(ActiveSaleDraft {
        cashier_id: row.get(0)?,
        cash_session_id: row.get(1)?,
        items,
        item_count: row.get(3)?,
        total: row.get(4)?,
        cash_received: row.get(5)?,
        card_received: row.get(6)?,
        transfer_received: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

#[tauri::command]
pub(crate) fn held_ticket_list(state: State<'_, AppState>) -> CommandResult<Vec<HeldTicket>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    held_ticket_list_with_conn(&conn)
}

pub(crate) fn held_ticket_list_with_conn(conn: &Connection) -> CommandResult<Vec<HeldTicket>> {
    let mut stmt = conn
        .prepare(
            "SELECT h.id, h.name, h.cashier_id, u.name, h.item_count, h.items_json, h.total, h.created_at, h.updated_at
             FROM held_tickets h
             JOIN users u ON u.id = h.cashier_id
             ORDER BY h.created_at ASC, h.id ASC",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], map_held_ticket)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn held_ticket_save(
    state: State<'_, AppState>,
    input: HeldTicketInput,
) -> CommandResult<HeldTicket> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    let tax_enabled = setting_bool(&conn, "tax_enabled", true)?;
    let prices_include_tax = setting_bool(&conn, "tax_prices_include_tax", true)?;
    let (name, item_count, total) =
        validate_held_ticket_input(&input, prices_include_tax, tax_enabled)?;
    let items_json = serde_json::to_string(&input.items).map_err(|error| error.to_string())?;
    let now = now_iso();
    let id = match input.id {
        Some(id) => {
            conn.execute(
                "UPDATE held_tickets
                 SET name = ?1, cashier_id = ?2, items_json = ?3, item_count = ?4, total = ?5, updated_at = ?6
                 WHERE id = ?7",
                params![name, input.cashier_id, items_json, item_count, total, now, id],
            )
            .map_err(|error| error.to_string())?;
            if conn.changes() == 0 {
                return Err("Ticket abierto no existe".into());
            }
            id
        }
        None => {
            conn.execute(
                "INSERT INTO held_tickets (name, cashier_id, items_json, item_count, total, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                params![name, input.cashier_id, items_json, item_count, total, now],
            )
            .map_err(|error| error.to_string())?;
            conn.last_insert_rowid()
        }
    };
    conn.query_row(
        "SELECT h.id, h.name, h.cashier_id, u.name, h.item_count, h.items_json, h.total, h.created_at, h.updated_at
         FROM held_tickets h
         JOIN users u ON u.id = h.cashier_id
         WHERE h.id = ?1",
        params![id],
        map_held_ticket,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn held_ticket_delete(state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    conn.execute("DELETE FROM held_tickets WHERE id = ?1", params![id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) fn active_sale_draft_get(
    state: State<'_, AppState>,
    cashier_id: i64,
    cash_session_id: Option<i64>,
) -> CommandResult<Option<ActiveSaleDraft>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    conn.query_row(
        "SELECT cashier_id, cash_session_id, items_json, item_count, total, cash_received, card_received, transfer_received, updated_at
         FROM active_sale_drafts
         WHERE cashier_id = ?1
           AND (cash_session_id IS NULL OR cash_session_id = ?2)
         ORDER BY updated_at DESC
         LIMIT 1",
        params![cashier_id, cash_session_id],
        map_active_sale_draft,
    )
    .optional()
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn active_sale_draft_save(
    state: State<'_, AppState>,
    input: ActiveSaleDraftInput,
) -> CommandResult<ActiveSaleDraft> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    let tax_enabled = setting_bool(&conn, "tax_enabled", true)?;
    let prices_include_tax = setting_bool(&conn, "tax_prices_include_tax", true)?;
    let (item_count, total) =
        validate_active_sale_draft_input(&input, prices_include_tax, tax_enabled)?;
    let items_json = serde_json::to_string(&input.items).map_err(|error| error.to_string())?;
    let now = now_iso();
    conn.execute(
        "INSERT INTO active_sale_drafts
         (cashier_id, cash_session_id, items_json, item_count, total, cash_received, card_received, transfer_received, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(cashier_id) DO UPDATE SET
           cash_session_id = excluded.cash_session_id,
           items_json = excluded.items_json,
           item_count = excluded.item_count,
           total = excluded.total,
           cash_received = excluded.cash_received,
           card_received = excluded.card_received,
           transfer_received = excluded.transfer_received,
           updated_at = excluded.updated_at",
        params![
            input.cashier_id,
            input.cash_session_id,
            items_json,
            item_count,
            total,
            round_money(input.cash_received),
            round_money(input.card_received),
            round_money(input.transfer_received),
            now
        ],
    )
    .map_err(|error| error.to_string())?;
    conn.query_row(
        "SELECT cashier_id, cash_session_id, items_json, item_count, total, cash_received, card_received, transfer_received, updated_at
         FROM active_sale_drafts
         WHERE cashier_id = ?1",
        params![input.cashier_id],
        map_active_sale_draft,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn active_sale_draft_clear(state: State<'_, AppState>, cashier_id: i64) -> CommandResult<()> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    conn.execute(
        "DELETE FROM active_sale_drafts WHERE cashier_id = ?1",
        params![cashier_id],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn create_sale_with_conn(conn: &mut Connection, draft: SaleDraft) -> CommandResult<SaleReceipt> {
    if draft.items.is_empty() {
        return Err("Venta sin articulos".into());
    }
    if draft.payments.is_empty() {
        return Err("Venta sin pagos".into());
    }

    require_active_user(conn, draft.cashier_id)?;
    let tax_enabled = setting_bool(conn, "tax_enabled", true)?;
    let prices_include_tax = setting_bool(conn, "tax_prices_include_tax", true)?;
    let tx = conn.transaction().map_err(|error| error.to_string())?;
    let workstation_id = current_workstation_id(&tx)?;
    let (shift_id, cash_session_id) = get_open_shift(&tx, &workstation_id)?
        .ok_or_else(|| "No hay turno abierto para registrar venta".to_string())?;

    let mut subtotal = 0.0;
    let mut tax = 0.0;
    let mut discount = 0.0;

    for item in &draft.items {
        if !item.quantity.is_finite() || item.quantity <= 0.0 {
            return Err("Cantidad invalida".into());
        }
        validate_non_negative(item.unit_price, "Precio invalido")?;
        validate_non_negative(item.discount, "Descuento invalido")?;
        let (stock, tax_rate): (f64, f64) = tx
            .query_row(
                "SELECT stock, tax_rate FROM products WHERE id = ?1 AND active = 1",
                params![item.product_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| format!("Producto no disponible: {}", item.product_id))?;
        if stock < item.quantity {
            return Err(format!(
                "Stock insuficiente para producto {}",
                item.product_id
            ));
        }
        let base = item.quantity * item.unit_price;
        let line_discount = item.discount.max(0.0).min(base);
        let (line_subtotal, line_tax, _) = line_amounts(
            base,
            line_discount,
            tax_rate,
            prices_include_tax,
            tax_enabled,
        );
        subtotal += line_subtotal;
        tax += line_tax;
        discount += line_discount;
    }

    let total = round_money(subtotal + tax);
    for payment in &draft.payments {
        validate_positive(payment.amount, "Pago invalido")?;
    }
    let paid = round_money(draft.payments.iter().map(|payment| payment.amount).sum());
    if paid < total {
        return Err("Pago insuficiente".into());
    }
    let cash_paid: f64 = draft
        .payments
        .iter()
        .filter(|payment| payment.method == "cash")
        .map(|payment| payment.amount)
        .sum();
    let non_cash_paid = round_money(paid - cash_paid);
    if non_cash_paid > total {
        return Err("Tarjeta/credito excede total".into());
    }
    let cash_needed = round_money((total - non_cash_paid).max(0.0));
    let change_due = round_money((cash_paid - cash_needed).max(0.0));
    let created_at = now_iso();
    let period = period_key(&created_at)?;
    let current_month_max: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(monthly_seq), 0)
             FROM sales
             WHERE strftime('%Y-%m', created_at) = ?1",
            params![period],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let monthly_seq = next_monthly_seq(current_month_max);
    let folio = visible_monthly_folio(&period, monthly_seq);

    tx.execute(
        "INSERT INTO sales
         (folio, monthly_seq, shift_id, cashier_id, customer_id, cash_session_id, subtotal, tax, discount, total, paid, change_due, status, notes, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'paid', ?13, ?14)",
        params![
            folio,
            monthly_seq,
            shift_id,
            draft.cashier_id,
            draft.customer_id,
            cash_session_id,
            round_money(subtotal),
            round_money(tax),
            round_money(discount),
            total,
            paid,
            change_due,
            draft.notes.as_deref(),
            created_at
        ],
    )
    .map_err(|error| error.to_string())?;
    let sale_id = tx.last_insert_rowid();

    for item in &draft.items {
        let tax_rate: f64 = tx
            .query_row(
                "SELECT tax_rate FROM products WHERE id = ?1",
                params![item.product_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let base = item.quantity * item.unit_price;
        let line_discount = item.discount.max(0.0).min(base);
        let (_, _, line_total) = line_amounts(
            base,
            line_discount,
            tax_rate,
            prices_include_tax,
            tax_enabled,
        );
        tx.execute(
            "INSERT INTO sale_items (sale_id, product_id, quantity, unit_price, discount, tax_rate, line_total)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                sale_id,
                item.product_id,
                item.quantity,
                item.unit_price,
                item.discount,
                tax_rate,
                line_total
            ],
        )
        .map_err(|error| error.to_string())?;
        tx.execute(
            "UPDATE products SET stock = stock - ?1, updated_at = ?2 WHERE id = ?3",
            params![item.quantity, created_at, item.product_id],
        )
        .map_err(|error| error.to_string())?;
        tx.execute(
            "INSERT INTO inventory_movements (product_id, movement_type, quantity, reason, reference_id, created_at)
             VALUES (?1, 'sale', ?2, 'Venta', ?3, ?4)",
            params![item.product_id, -item.quantity, sale_id, created_at],
        )
        .map_err(|error| error.to_string())?;
    }

    for payment in &draft.payments {
        tx.execute(
            "INSERT INTO payments (sale_id, method, amount, reference, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                sale_id,
                payment.method,
                payment.amount,
                payment.reference,
                created_at
            ],
        )
        .map_err(|error| error.to_string())?;
    }

    tx.execute(
        "UPDATE cash_sessions
         SET sales_total = sales_total + ?1, expected_cash = expected_cash + ?2
         WHERE id = ?3 AND status = 'open'",
        params![total, cash_paid - change_due, cash_session_id],
    )
    .map_err(|error| error.to_string())?;
    tx.execute(
        "UPDATE shifts
         SET expected_cash = expected_cash + ?1
         WHERE id = ?2 AND status = 'open'",
        params![cash_paid - change_due, shift_id],
    )
    .map_err(|error| error.to_string())?;

    tx.commit().map_err(|error| error.to_string())?;
    Ok(SaleReceipt {
        sale_id,
        folio,
        subtotal: round_money(subtotal),
        tax: round_money(tax),
        discount: round_money(discount),
        total,
        paid,
        change_due,
        created_at,
    })
}

#[tauri::command]
pub(crate) fn sale_create(state: State<'_, AppState>, draft: SaleDraft) -> CommandResult<SaleReceipt> {
    let mut conn = state.db.lock().map_err(|error| error.to_string())?;
    create_sale_with_conn(&mut conn, draft)
}

#[tauri::command]
pub(crate) fn sale_list(
    state: State<'_, AppState>,
    actor_id: i64,
    limit: Option<i64>,
) -> CommandResult<Vec<SaleListItem>> {
    let limit = limit.unwrap_or(80).clamp(1, 300);
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, actor_id)?;
    let mut stmt = conn
        .prepare(
            "SELECT s.id, s.folio, u.name, s.total, s.paid,
                    COALESCE(SUM(CASE WHEN p.method = 'cash' THEN p.amount ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN p.method = 'card' THEN p.amount ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN p.method = 'transfer' THEN p.amount ELSE 0 END), 0),
                    s.status, s.created_at
             FROM sales s
             JOIN users u ON u.id = s.cashier_id
             LEFT JOIN payments p ON p.sale_id = s.id
             GROUP BY s.id, s.folio, u.name, s.total, s.paid, s.status, s.created_at
             ORDER BY s.id DESC
             LIMIT ?1",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![limit], |row| {
            Ok(SaleListItem {
                id: row.get(0)?,
                folio: row.get(1)?,
                cashier_name: row.get(2)?,
                total: row.get(3)?,
                paid: row.get(4)?,
                cash_paid: row.get(5)?,
                card_paid: row.get(6)?,
                transfer_paid: row.get(7)?,
                status: row.get(8)?,
                created_at: row.get(9)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub(crate) fn cancel_sale_with_conn(
    conn: &mut Connection,
    sale_id: i64,
    actor_id: i64,
    reason: String,
) -> CommandResult<()> {
    if reason.trim().len() < 2 {
        return Err("Motivo requerido".into());
    }
    require_admin(conn, actor_id)?;
    let tx = conn.transaction().map_err(|error| error.to_string())?;
    let (status, cash_session_id, shift_id, total, created_at): (
        String,
        Option<i64>,
        Option<i64>,
        f64,
        String,
    ) = tx
        .query_row(
            "SELECT status, cash_session_id, shift_id, total, created_at FROM sales WHERE id = ?1",
            params![sale_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .map_err(|_| "Venta no encontrada".to_string())?;
    if status == "canceled" {
        return Err("Venta ya cancelada".into());
    }
    if let Some(session_id) = cash_session_id {
        let session_status: String = tx
            .query_row(
                "SELECT status FROM cash_sessions WHERE id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .map_err(|_| "Turno de venta no encontrado".to_string())?;
        if session_status != "open" {
            return Err("Venta de corte cerrado: registra devolucion en el turno actual".into());
        }
    }
    if setting_bool(&tx, "period_lock_enabled", false)? {
        let period = period_key(&created_at)?;
        let locked: Option<String> = tx
            .query_row(
                "SELECT month FROM locked_periods WHERE month = ?1",
                params![period],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if locked.is_some() {
            return Err("Periodo bloqueado: no se puede cancelar venta retroactiva".into());
        }
    }
    let now = now_iso();
    {
        let mut stmt = tx
            .prepare("SELECT product_id, quantity FROM sale_items WHERE sale_id = ?1")
            .map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map(params![sale_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?))
            })
            .map_err(|error| error.to_string())?;
        for row in rows {
            let (product_id, quantity) = row.map_err(|error| error.to_string())?;
            tx.execute(
                "UPDATE products SET stock = stock + ?1, updated_at = ?2 WHERE id = ?3",
                params![quantity, now, product_id],
            )
            .map_err(|error| error.to_string())?;
            tx.execute(
                "INSERT INTO inventory_movements (product_id, movement_type, quantity, reason, reference_id, created_at)
                 VALUES (?1, 'cancel', ?2, ?3, ?4, ?5)",
                params![product_id, quantity, reason.trim(), sale_id, now],
            )
            .map_err(|error| error.to_string())?;
        }
    }
    tx.execute(
        "UPDATE sales
         SET status = 'canceled',
             notes = COALESCE(notes, '') || ?1,
             canceled_at = ?2,
             canceled_by = ?3,
             cancel_reason = ?4
         WHERE id = ?5",
        params![
            format!(" | Cancelada: {}", reason.trim()),
            now,
            actor_id,
            reason.trim(),
            sale_id
        ],
    )
    .map_err(|error| error.to_string())?;
    if let Some(session_id) = cash_session_id {
        let cash_paid: f64 = tx
            .query_row(
                "SELECT COALESCE(SUM(amount), 0) FROM payments WHERE sale_id = ?1 AND method = 'cash'",
                params![sale_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let change_due: f64 = tx
            .query_row(
                "SELECT change_due FROM sales WHERE id = ?1",
                params![sale_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        tx.execute(
            "UPDATE cash_sessions
             SET sales_total = MAX(0, sales_total - ?1), expected_cash = expected_cash - ?2
             WHERE id = ?3 AND status = 'open'",
            params![total, cash_paid - change_due, session_id],
        )
        .map_err(|error| error.to_string())?;
        if let Some(shift_id) = shift_id {
            tx.execute(
                "UPDATE shifts
                 SET expected_cash = expected_cash - ?1
                 WHERE id = ?2 AND status = 'open'",
                params![cash_paid - change_due, shift_id],
            )
            .map_err(|error| error.to_string())?;
        }
    }
    tx.execute(
        "INSERT INTO audit_log (actor_id, action, entity, entity_id, details, created_at)
         VALUES (?1, 'cancel', 'sale', ?2, ?3, ?4)",
        params![actor_id, sale_id, reason.trim(), now],
    )
    .map_err(|error| error.to_string())?;
    tx.commit().map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) fn sale_cancel(
    state: State<'_, AppState>,
    sale_id: i64,
    actor_id: i64,
    reason: String,
) -> CommandResult<()> {
    let mut conn = state.db.lock().map_err(|error| error.to_string())?;
    cancel_sale_with_conn(&mut conn, sale_id, actor_id, reason)
}

