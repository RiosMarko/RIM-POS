use crate::auth::{require_active_user, require_admin};
use crate::backend::{
    current_workstation_id, cut_printer_setting, has_permission, line_amounts, require_permission,
    setting_bool, ticket_setting, ticket_separator, ticket_setting_i64, AppState, CommandResult,
};
use crate::backup::backup_create_with_conn;
use crate::core::{average_ticket, now_iso, period_key, round_money};
use crate::hardware::{run_print_file, temp_hardware_file};
use crate::models::*;
use crate::validation::validate_positive;
use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::fs;
use tauri::State;

pub(crate) fn redact_shift_cut_profit(snapshot: &mut ShiftCutSnapshot) {
    snapshot.gross_profit = 0.0;
    for department in snapshot.departments.iter_mut() {
        department.gross_profit = 0.0;
    }
    for customer in snapshot.top_customers_by_sales.iter_mut() {
        customer.gross_profit = 0.0;
    }
    snapshot.top_customers_by_profit.clear();
}

pub(crate) fn redact_daily_cut_profit(summary: &mut DailyCutSummary) {
    summary.gross_profit = 0.0;
    for department in summary.departments.iter_mut() {
        department.gross_profit = 0.0;
    }
    for customer in summary.top_customers_by_sales.iter_mut() {
        customer.gross_profit = 0.0;
    }
    summary.top_customers_by_profit.clear();
    for cut in summary.cuts.iter_mut() {
        redact_shift_cut_profit(cut);
    }
}




pub(crate) fn open_cash_session_with_conn(
    conn: &Connection,
    opened_by: i64,
    opening_cash: f64,
) -> CommandResult<CashSession> {
    require_active_user(conn, opened_by)?;
    let workstation_id = current_workstation_id(conn)?;
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM cash_sessions WHERE status = 'open' AND workstation_id = ?1 ORDER BY id DESC LIMIT 1",
            params![workstation_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if existing.is_some() {
        return Err("Ya hay caja abierta".into());
    }
    let opened_at = now_iso();
    conn.execute(
        "INSERT INTO cash_sessions (opened_by, opened_at, opening_cash, expected_cash, sales_total, workstation_id, status)
         VALUES (?1, ?2, ?3, ?3, 0, ?4, 'open')",
        params![opened_by, opened_at, opening_cash, workstation_id],
    )
    .map_err(|error| error.to_string())?;
    let cash_session_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO shifts (cash_session_id, opened_by, opened_at, status, opening_cash, expected_cash)
         VALUES (?1, ?2, ?3, 'open', ?4, ?4)",
        params![cash_session_id, opened_by, opened_at, opening_cash],
    )
    .map_err(|error| error.to_string())?;
    get_cash_session(conn, cash_session_id)
}

#[tauri::command]
pub(crate) fn cash_session_open(
    state: State<'_, AppState>,
    opened_by: i64,
    opening_cash: f64,
) -> CommandResult<CashSession> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    open_cash_session_with_conn(&conn, opened_by, opening_cash)
}

pub(crate) fn map_cash_movement(row: &rusqlite::Row<'_>) -> rusqlite::Result<CashMovement> {
    Ok(CashMovement {
        id: row.get(0)?,
        session_id: row.get(1)?,
        movement_type: row.get(2)?,
        amount: row.get(3)?,
        reason: row.get(4)?,
        actor_name: row.get(5)?,
        created_at: row.get(6)?,
    })
}

#[tauri::command]
pub(crate) fn cash_movement_create(
    state: State<'_, AppState>,
    input: CashMovementInput,
) -> CommandResult<CashMovement> {
    if input.reason.trim().len() < 2 {
        return Err("Movimiento de caja invalido".into());
    }
    let movement_type = match input.movement_type.as_str() {
        "in" => "in",
        "out" => "out",
        "drawer" => "drawer",
        _ => return Err("Tipo de movimiento invalido".into()),
    };
    if movement_type == "drawer" {
        if input.amount != 0.0 {
            return Err("Movimiento de caja invalido".into());
        }
    } else {
        validate_positive(input.amount, "Movimiento de caja invalido")?;
    }
    let signed_amount = match movement_type {
        "in" => input.amount,
        "out" => -input.amount,
        _ => 0.0,
    };
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, input.actor_id)?;
    let now = now_iso();
    conn.execute(
        "INSERT INTO cash_movements (session_id, movement_type, amount, reason, actor_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            input.session_id,
            movement_type,
            input.amount,
            input.reason.trim(),
            input.actor_id,
            now
        ],
    )
    .map_err(|error| error.to_string())?;
    let id = conn.last_insert_rowid();
    conn.execute(
        "UPDATE cash_sessions SET expected_cash = expected_cash + ?1 WHERE id = ?2 AND status = 'open'",
        params![signed_amount, input.session_id],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "UPDATE shifts SET expected_cash = expected_cash + ?1 WHERE cash_session_id = ?2 AND status = 'open'",
        params![signed_amount, input.session_id],
    )
    .map_err(|error| error.to_string())?;
    conn.query_row(
        "SELECT m.id, m.session_id, m.movement_type, m.amount, m.reason, u.name, m.created_at
         FROM cash_movements m
         JOIN users u ON u.id = m.actor_id
         WHERE m.id = ?1",
        params![id],
        map_cash_movement,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn cash_movement_list(
    state: State<'_, AppState>,
    actor_id: i64,
    session_id: i64,
) -> CommandResult<Vec<CashMovement>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, actor_id)?;
    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.session_id, m.movement_type, m.amount, m.reason, u.name, m.created_at
             FROM cash_movements m
             JOIN users u ON u.id = m.actor_id
             WHERE m.session_id = ?1
             ORDER BY m.id DESC",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![session_id], map_cash_movement)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub(crate) fn map_cash_count(row: &rusqlite::Row<'_>) -> rusqlite::Result<CashCount> {
    Ok(CashCount {
        id: row.get(0)?,
        session_id: row.get(1)?,
        shift_id: row.get(2)?,
        count_type: row.get(3)?,
        expected_cash: row.get(4)?,
        counted_cash: row.get(5)?,
        difference: row.get(6)?,
        denominations_json: row.get(7)?,
        difference_reason: row.get(8)?,
        actor_name: row.get(9)?,
        created_at: row.get(10)?,
    })
}

pub(crate) fn create_cash_count_with_conn(
    conn: &Connection,
    input: &CashCountInput,
) -> CommandResult<CashCount> {
    if !input.expected_cash.is_finite()
        || !input.counted_cash.is_finite()
        || input.counted_cash < 0.0
    {
        return Err("Arqueo invalido".into());
    }
    let count_type = match input.count_type.as_str() {
        "audit" => "audit",
        "close" => "close",
        _ => return Err("Tipo de arqueo invalido".into()),
    };
    serde_json::from_str::<serde_json::Value>(&input.denominations_json)
        .map_err(|_| "Denominaciones invalidas".to_string())?;
    let difference = round_money(input.counted_cash - input.expected_cash);
    let reason = input
        .difference_reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if difference != 0.0 && reason.is_none() {
        return Err("Motivo de diferencia requerido".into());
    }
    require_active_user(conn, input.actor_id)?;
    let now = now_iso();
    conn.execute(
        "INSERT INTO cash_counts
         (session_id, shift_id, count_type, expected_cash, counted_cash, difference, denominations_json, difference_reason, actor_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            input.session_id,
            input.shift_id,
            count_type,
            round_money(input.expected_cash),
            round_money(input.counted_cash),
            difference,
            input.denominations_json,
            reason,
            input.actor_id,
            now
        ],
    )
    .map_err(|error| error.to_string())?;
    let id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO audit_log (actor_id, action, entity, entity_id, details, created_at)
         VALUES (?1, ?2, 'cash_count', ?3, ?4, ?5)",
        params![
            input.actor_id,
            if count_type == "close" {
                "cash_count_close"
            } else {
                "cash_count_audit"
            },
            id,
            format!(
                "esperado {:.2}, contado {:.2}, diferencia {:.2}",
                input.expected_cash, input.counted_cash, difference
            ),
            now
        ],
    )
    .map_err(|error| error.to_string())?;
    conn.query_row(
        "SELECT cc.id, cc.session_id, cc.shift_id, cc.count_type, cc.expected_cash, cc.counted_cash,
                cc.difference, cc.denominations_json, cc.difference_reason, u.name, cc.created_at
         FROM cash_counts cc
         JOIN users u ON u.id = cc.actor_id
         WHERE cc.id = ?1",
        params![id],
        map_cash_count,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn cash_count_create(
    state: State<'_, AppState>,
    input: CashCountInput,
) -> CommandResult<CashCount> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    create_cash_count_with_conn(&conn, &input)
}

#[tauri::command]
pub(crate) fn cash_count_list(
    state: State<'_, AppState>,
    actor_id: i64,
    session_id: i64,
) -> CommandResult<Vec<CashCount>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, actor_id)?;
    let mut stmt = conn
        .prepare(
            "SELECT cc.id, cc.session_id, cc.shift_id, cc.count_type, cc.expected_cash, cc.counted_cash,
                    cc.difference, cc.denominations_json, cc.difference_reason, u.name, cc.created_at
             FROM cash_counts cc
             JOIN users u ON u.id = cc.actor_id
             WHERE cc.session_id = ?1
             ORDER BY cc.id DESC",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![session_id], map_cash_count)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn cash_session_close(
    state: State<'_, AppState>,
    session_id: i64,
    closing_cash: f64,
) -> CommandResult<CashSession> {
    let _ = (state, session_id, closing_cash);
    Err("Use Corte Z para cerrar turno oficialmente".into())
}

#[tauri::command]
pub(crate) fn shift_cut_x(
    state: State<'_, AppState>,
    actor_id: i64,
    shift_id: Option<i64>,
) -> CommandResult<ShiftCutSnapshot> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, actor_id)?;
    let next_shift_id = match shift_id {
        Some(id) => id,
        None => {
            let workstation_id = current_workstation_id(&conn)?;
            get_open_shift(&conn, &workstation_id)?
                .map(|(id, _)| id)
                .ok_or_else(|| "No hay turno abierto".to_string())?
        }
    };
    let mut snapshot = calculate_shift_cut(&conn, next_shift_id)?;
    if !has_permission(&conn, actor_id, "view_profit")? {
        redact_shift_cut_profit(&mut snapshot);
    }
    Ok(snapshot)
}

pub(crate) fn close_shift_cut_z_with_conn(
    conn: &mut Connection,
    shift_id: i64,
    closing_cash: f64,
    closed_by: i64,
    denominations_json: Option<String>,
    difference_reason: Option<String>,
) -> CommandResult<ShiftCutSnapshot> {
    if closing_cash < 0.0 {
        return Err("Efectivo contado invalido".into());
    }
    require_active_user(conn, closed_by)?;
    let tx = conn.transaction().map_err(|error| error.to_string())?;
    let (status, cash_session_id): (String, i64) = tx
        .query_row(
            "SELECT status, cash_session_id FROM shifts WHERE id = ?1",
            params![shift_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| "Turno no encontrado".to_string())?;
    if status != "open" {
        return Err("Corte Z ya fue aplicado a este turno".into());
    }
    let mut snapshot = calculate_shift_cut(&tx, shift_id)?;
    let difference = round_money(closing_cash - snapshot.expected_cash);
    let reason = difference_reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if difference != 0.0 && reason.is_none() {
        return Err("Motivo de diferencia requerido".into());
    }
    let denominations_json = denominations_json.unwrap_or_else(|| "[]".into());
    create_cash_count_with_conn(
        &tx,
        &CashCountInput {
            session_id: cash_session_id,
            shift_id: Some(shift_id),
            count_type: "close".into(),
            expected_cash: snapshot.expected_cash,
            counted_cash: closing_cash,
            denominations_json: denominations_json.clone(),
            difference_reason: reason.map(str::to_string),
            actor_id: closed_by,
        },
    )?;
    let closed_at = now_iso();
    snapshot.status = "closed".into();
    snapshot.closed_at = Some(closed_at.clone());
    snapshot.closing_cash = Some(round_money(closing_cash));
    snapshot.counted_cash = Some(round_money(closing_cash));
    snapshot.cash_difference = Some(difference);
    snapshot.difference_reason = reason.map(str::to_string);
    let snapshot_json = serde_json::to_string(&snapshot).map_err(|error| error.to_string())?;
    tx.execute(
        "UPDATE shifts
         SET status = 'closed',
             closed_by = ?1,
             closed_at = ?2,
             closing_cash = ?3,
             total_tickets = ?4,
             canceled_tickets = ?5,
             gross_sales = ?6,
             net_sales = ?7,
             tax = ?8,
             discount = ?9,
             cash_paid = ?10,
             card_paid = ?11,
             transfer_paid = ?12,
             average_ticket = ?13,
             expected_cash = ?14,
             snapshot_json = ?15
         WHERE id = ?16 AND status = 'open'",
        params![
            closed_by,
            closed_at,
            closing_cash,
            snapshot.total_tickets,
            snapshot.canceled_tickets,
            snapshot.gross_sales,
            snapshot.net_sales,
            snapshot.tax,
            snapshot.discount,
            snapshot.cash_paid,
            snapshot.card_paid,
            snapshot.transfer_paid,
            snapshot.average_ticket,
            snapshot.expected_cash,
            snapshot_json,
            shift_id
        ],
    )
    .map_err(|error| error.to_string())?;
    tx.execute(
        "UPDATE cash_sessions
         SET status = 'closed', closed_at = ?1, closing_cash = ?2
         WHERE id = ?3 AND status = 'open'",
        params![closed_at, closing_cash, cash_session_id],
    )
    .map_err(|error| error.to_string())?;
    tx.execute(
        "INSERT INTO audit_log (actor_id, action, entity, entity_id, details, created_at)
         VALUES (?1, 'cut_z', 'shift', ?2, ?3, ?4)",
        params![closed_by, shift_id, snapshot_json, closed_at],
    )
    .map_err(|error| error.to_string())?;
    tx.commit().map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
pub(crate) fn shift_cut_z(
    state: State<'_, AppState>,
    shift_id: i64,
    closing_cash: f64,
    closed_by: i64,
    denominations_json: Option<String>,
    difference_reason: Option<String>,
) -> CommandResult<ShiftCutSnapshot> {
    let mut conn = state.db.lock().map_err(|error| error.to_string())?;
    let mut snapshot = close_shift_cut_z_with_conn(
        &mut conn,
        shift_id,
        closing_cash,
        closed_by,
        denominations_json,
        difference_reason,
    )?;
    if !has_permission(&conn, closed_by, "view_profit")? {
        redact_shift_cut_profit(&mut snapshot);
    }
    match backup_create_with_conn(&conn, &state.db_path) {
        Ok(backup) => {
            conn.execute(
                "INSERT INTO audit_log (actor_id, action, entity, entity_id, details, created_at)
                 VALUES (?1, 'backup_cut_z', 'backup', NULL, ?2, ?3)",
                params![closed_by, backup.path, backup.created_at],
            )
            .map_err(|error| error.to_string())?;
        }
        Err(error) => {
            conn.execute(
                "INSERT INTO audit_log (actor_id, action, entity, entity_id, details, created_at)
                 VALUES (?1, 'backup_cut_z_failed', 'backup', NULL, ?2, ?3)",
                params![closed_by, error, now_iso()],
            )
            .map_err(|error| error.to_string())?;
        }
    }
    Ok(snapshot)
}

#[tauri::command]
pub(crate) fn shift_cut_history(
    state: State<'_, AppState>,
    actor_id: i64,
    limit: Option<i64>,
) -> CommandResult<Vec<ShiftCutSnapshot>> {
    let limit = limit.unwrap_or(20).clamp(1, 100);
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, actor_id)?;
    let can_view_profit = has_permission(&conn, actor_id, "view_profit")?;
    let mut stmt = conn
        .prepare("SELECT id FROM shifts ORDER BY id DESC LIMIT ?1")
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![limit], |row| row.get::<_, i64>(0))
        .map_err(|error| error.to_string())?;
    let mut cuts = Vec::new();
    for row in rows {
        let mut cut = calculate_shift_cut(&conn, row.map_err(|error| error.to_string())?)?;
        if !can_view_profit {
            redact_shift_cut_profit(&mut cut);
        }
        cuts.push(cut);
    }
    Ok(cuts)
}

pub(crate) fn shift_cut_text(conn: &Connection, shift_id: i64, actor_id: i64) -> CommandResult<String> {
    let mut snapshot = calculate_shift_cut(conn, shift_id)?;
    let can_view_profit = has_permission(conn, actor_id, "view_profit")?;
    if !can_view_profit {
        redact_shift_cut_profit(&mut snapshot);
    }
    let store_name = ticket_setting(conn, "ticket_store_name", "RIM-POS")?;
    let width = ticket_setting_i64(conn, "ticket_width", 32, 24, 48)? as usize;
    let separator = ticket_separator(width);
    let mut text = String::new();
    text.push_str(&format!(
        "{store_name}\nCORTE {}\n{separator}\n",
        snapshot.shift_id
    ));
    text.push_str(&format!("Apertura: {}\n", snapshot.opened_at));
    if let Some(closed_at) = &snapshot.closed_at {
        text.push_str(&format!("Cierre: {closed_at}\n"));
    }
    if let Some(workstation_id) = &snapshot.workstation_id {
        text.push_str(&format!("Caja: {workstation_id}\n"));
    }
    text.push_str(&format!("Duracion: {} min\n", snapshot.duration_minutes));
    text.push_str(&format!("Tickets: {}\n", snapshot.total_tickets));
    text.push_str(&format!("Cancelados: {}\n", snapshot.canceled_tickets));
    text.push_str(&format!("Ventas netas: ${:.2}\n", snapshot.net_sales));
    if can_view_profit {
        text.push_str(&format!("Ganancia: ${:.2}\n", snapshot.gross_profit));
    }
    text.push_str(&format!("Efectivo: ${:.2}\n", snapshot.cash_paid));
    text.push_str(&format!("Tarjeta: ${:.2}\n", snapshot.card_paid));
    text.push_str(&format!("Transfer: ${:.2}\n", snapshot.transfer_paid));
    text.push_str(&format!("Ventas cred: ${:.2}\n", snapshot.credit_sales));
    text.push_str(&format!("Fondo inicial: ${:.2}\n", snapshot.opening_cash));
    text.push_str(&format!("Entradas: ${:.2}\n", snapshot.cash_entries_total));
    text.push_str(&format!("Salidas: ${:.2}\n", snapshot.cash_out_total));
    text.push_str(&format!("Dev cash: ${:.2}\n", snapshot.cash_refunds_total));
    text.push_str(&format!("Abonos cred: ${:.2}\n", snapshot.credit_payments_total));
    text.push_str(&format!("Esperado: ${:.2}\n", snapshot.expected_cash));
    if let Some(counted) = snapshot.counted_cash.or(snapshot.closing_cash) {
        text.push_str(&format!("Contado: ${counted:.2}\n"));
        text.push_str(&format!(
            "Diferencia: ${:.2}\n",
            counted - snapshot.expected_cash
        ));
    }
    if !snapshot.departments.is_empty() {
        text.push_str(&format!("{separator}\nDEPTO\n"));
        for department in snapshot.departments.iter().take(6) {
            text.push_str(&format!(
                "{} ${:.2}\n",
                department.category, department.total_sales
            ));
        }
    }
    text.push_str(&format!("{separator}\n"));
    Ok(text)
}

pub(crate) fn daily_cut_summary_with_conn(
    conn: &Connection,
    date: Option<String>,
) -> CommandResult<DailyCutSummary> {
    let date = date
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| Local::now().format("%Y-%m-%d").to_string());
    let mut stmt = conn
        .prepare(
            "SELECT id
             FROM shifts
             WHERE status = 'closed'
               AND closed_at IS NOT NULL
               AND date(closed_at, 'localtime') = date(?1)
             ORDER BY closed_at, id",
        )
        .map_err(|error| error.to_string())?;
    let shift_ids = stmt
        .query_map(params![date], |row| row.get::<_, i64>(0))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let mut cuts = Vec::new();
    for shift_id in shift_ids {
        cuts.push(calculate_shift_cut(conn, shift_id)?);
    }
    let total_tickets = cuts.iter().map(|cut| cut.total_tickets).sum();
    let net_sales: f64 = cuts.iter().map(|cut| cut.net_sales).sum();
    Ok(DailyCutSummary {
        date,
        cut_count: cuts.len() as i64,
        total_tickets,
        canceled_tickets: cuts.iter().map(|cut| cut.canceled_tickets).sum(),
        gross_sales: round_money(cuts.iter().map(|cut| cut.gross_sales).sum()),
        net_sales: round_money(net_sales),
        gross_profit: round_money(cuts.iter().map(|cut| cut.gross_profit).sum()),
        tax: round_money(cuts.iter().map(|cut| cut.tax).sum()),
        discount: round_money(cuts.iter().map(|cut| cut.discount).sum()),
        cash_paid: round_money(cuts.iter().map(|cut| cut.cash_paid).sum()),
        card_paid: round_money(cuts.iter().map(|cut| cut.card_paid).sum()),
        transfer_paid: round_money(cuts.iter().map(|cut| cut.transfer_paid).sum()),
        credit_sales: round_money(cuts.iter().map(|cut| cut.credit_sales).sum()),
        cash_entries_total: round_money(cuts.iter().map(|cut| cut.cash_entries_total).sum()),
        cash_out_total: round_money(cuts.iter().map(|cut| cut.cash_out_total).sum()),
        cash_refunds_total: round_money(cuts.iter().map(|cut| cut.cash_refunds_total).sum()),
        credit_payments_total: round_money(cuts.iter().map(|cut| cut.credit_payments_total).sum()),
        counted_income_total: round_money(cuts.iter().map(|cut| cut.counted_income_total).sum()),
        average_ticket: average_ticket(net_sales, total_tickets),
        opening_cash: round_money(cuts.iter().map(|cut| cut.opening_cash).sum()),
        expected_cash: round_money(cuts.iter().map(|cut| cut.expected_cash).sum()),
        counted_cash: round_money(
            cuts.iter()
                .map(|cut| cut.counted_cash.or(cut.closing_cash).unwrap_or(0.0))
                .sum(),
        ),
        cash_difference: round_money(
            cuts.iter()
                .map(|cut| cut.cash_difference.unwrap_or(0.0))
                .sum(),
        ),
        payment_breakdown: merge_payment_breakdowns(&cuts),
        departments: merge_department_summaries(&cuts),
        refunds: cuts.iter().flat_map(|cut| cut.refunds.clone()).collect(),
        credit_payments: cuts
            .iter()
            .flat_map(|cut| cut.credit_payments.clone())
            .collect(),
        taxes: merge_tax_summaries(&cuts),
        top_customers_by_sales: merge_customer_summaries(&cuts, |cut| &cut.top_customers_by_sales),
        top_customers_by_profit: merge_customer_summaries(&cuts, |cut| &cut.top_customers_by_profit),
        cuts,
    })
}

#[tauri::command]
pub(crate) fn daily_cut_summary(
    state: State<'_, AppState>,
    actor_id: i64,
    date: Option<String>,
) -> CommandResult<DailyCutSummary> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, actor_id)?;
    let mut summary = daily_cut_summary_with_conn(&conn, date)?;
    if !has_permission(&conn, actor_id, "view_profit")? {
        redact_daily_cut_profit(&mut summary);
    }
    Ok(summary)
}

pub(crate) fn daily_cut_text(conn: &Connection, date: Option<String>, actor_id: i64) -> CommandResult<String> {
    let mut summary = daily_cut_summary_with_conn(conn, date)?;
    let can_view_profit = has_permission(conn, actor_id, "view_profit")?;
    if !can_view_profit {
        redact_daily_cut_profit(&mut summary);
    }
    let store_name = ticket_setting(conn, "ticket_store_name", "RIM-POS")?;
    let width = ticket_setting_i64(conn, "ticket_width", 32, 24, 48)? as usize;
    let separator = ticket_separator(width);
    let mut text = String::new();
    text.push_str(&format!(
        "{store_name}\nCORTE GENERAL DEL DIA\n{separator}\n"
    ));
    text.push_str(&format!("Fecha: {}\n", summary.date));
    text.push_str(&format!("Turnos cerrados: {}\n", summary.cut_count));
    text.push_str(&format!("Tickets: {}\n", summary.total_tickets));
    text.push_str(&format!("Cancelados: {}\n", summary.canceled_tickets));
    text.push_str(&format!("Ventas netas: ${:.2}\n", summary.net_sales));
    if can_view_profit {
        text.push_str(&format!("Ganancia: ${:.2}\n", summary.gross_profit));
    }
    text.push_str(&format!("Efectivo: ${:.2}\n", summary.cash_paid));
    text.push_str(&format!("Tarjeta: ${:.2}\n", summary.card_paid));
    text.push_str(&format!("Transfer: ${:.2}\n", summary.transfer_paid));
    text.push_str(&format!("Ventas cred: ${:.2}\n", summary.credit_sales));
    text.push_str(&format!("Entradas: ${:.2}\n", summary.cash_entries_total));
    text.push_str(&format!("Salidas: ${:.2}\n", summary.cash_out_total));
    text.push_str(&format!("Dev cash: ${:.2}\n", summary.cash_refunds_total));
    text.push_str(&format!("Abonos cred: ${:.2}\n", summary.credit_payments_total));
    text.push_str(&format!("Esperado: ${:.2}\n", summary.expected_cash));
    text.push_str(&format!("Contado: ${:.2}\n", summary.counted_cash));
    text.push_str(&format!(
        "Diferencia: ${:.2}\n{separator}\n",
        summary.cash_difference
    ));
    for cut in summary.cuts {
        text.push_str(&format!(
            "Corte #{} {} ${:.2} dif ${:.2}\n",
            cut.shift_id,
            cut.closed_by_name.unwrap_or_else(|| "sin cajero".into()),
            cut.net_sales,
            cut.cash_difference.unwrap_or(0.0)
        ));
    }
    text.push_str(&format!("{separator}\n"));
    Ok(text)
}

#[tauri::command]
pub(crate) fn print_daily_cut(
    state: State<'_, AppState>,
    actor_id: i64,
    date: Option<String>,
) -> CommandResult<HardwareResult> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, actor_id)?;
    let printer = cut_printer_setting(&conn)?;
    let text = daily_cut_text(&conn, date, actor_id)?;
    drop(conn);
    let file = temp_hardware_file("rim-pos-daily-cut", "txt");
    fs::write(&file, text).map_err(|error| error.to_string())?;
    run_print_file(&printer, &file, false)?;
    let _ = fs::remove_file(file);
    Ok(HardwareResult {
        ok: true,
        message: "Corte general enviado a impresora".into(),
    })
}

#[tauri::command]
pub(crate) fn print_shift_cut(
    state: State<'_, AppState>,
    actor_id: i64,
    shift_id: i64,
) -> CommandResult<HardwareResult> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, actor_id)?;
    let printer = cut_printer_setting(&conn)?;
    let text = shift_cut_text(&conn, shift_id, actor_id)?;
    drop(conn);
    let file = temp_hardware_file("rim-pos-cut", "txt");
    fs::write(&file, text).map_err(|error| error.to_string())?;
    run_print_file(&printer, &file, false)?;
    let _ = fs::remove_file(file);
    Ok(HardwareResult {
        ok: true,
        message: format!("Corte {shift_id} enviado a impresora"),
    })
}

#[tauri::command]
pub(crate) fn audit_log_list(
    state: State<'_, AppState>,
    actor_id: i64,
    limit: Option<i64>,
) -> CommandResult<Vec<AuditLogEntry>> {
    let limit = limit.unwrap_or(80).clamp(1, 300);
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_admin(&conn, actor_id)?;
    let mut stmt = conn
        .prepare(
            "SELECT a.id, u.name, a.action, a.entity, a.entity_id, a.details, a.created_at
             FROM audit_log a
             LEFT JOIN users u ON u.id = a.actor_id
             ORDER BY a.id DESC
             LIMIT ?1",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![limit], |row| {
            Ok(AuditLogEntry {
                id: row.get(0)?,
                actor_name: row.get(1)?,
                action: row.get(2)?,
                entity: row.get(3)?,
                entity_id: row.get(4)?,
                details: row.get(5)?,
                created_at: row.get(6)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn period_lock(
    state: State<'_, AppState>,
    actor_id: i64,
    month: String,
    reason: Option<String>,
) -> CommandResult<()> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "reports")?;
    if !setting_bool(&conn, "period_lock_enabled", false)? {
        return Err("Bloqueo de periodo desactivado".into());
    }
    if period_key(&format!("{month}-01T00:00:00Z"))? != month {
        return Err("Mes invalido".into());
    }
    let locked_at = now_iso();
    let details = reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Sin motivo");
    conn.execute(
        "INSERT OR IGNORE INTO locked_periods (month, locked_at, reason) VALUES (?1, ?2, ?3)",
        params![month.as_str(), locked_at.as_str(), reason.as_deref()],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT INTO audit_log (actor_id, action, entity, entity_id, details, created_at)
         VALUES (?1, 'lock', 'period', NULL, ?2, ?3)",
        params![actor_id, format!("{month}: {details}"), locked_at],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn get_cash_session(conn: &Connection, id: i64) -> CommandResult<CashSession> {
    conn.query_row(
        "SELECT id, opened_by, opened_at, closed_at, opening_cash, closing_cash, expected_cash, sales_total, status
         FROM cash_sessions WHERE id = ?1",
        params![id],
        |row| {
            Ok(CashSession {
                id: row.get(0)?,
                opened_by: row.get(1)?,
                opened_at: row.get(2)?,
                closed_at: row.get(3)?,
                opening_cash: row.get(4)?,
                closing_cash: row.get(5)?,
                expected_cash: row.get(6)?,
                sales_total: row.get(7)?,
                status: row.get(8)?,
            })
        },
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn get_open_shift(conn: &Connection, workstation_id: &str) -> CommandResult<Option<(i64, i64)>> {
    conn.query_row(
        "SELECT sh.id, sh.cash_session_id
         FROM shifts sh
         JOIN cash_sessions cs ON cs.id = sh.cash_session_id
         WHERE sh.status = 'open' AND cs.status = 'open' AND cs.workstation_id = ?1
         ORDER BY sh.id DESC
         LIMIT 1",
        params![workstation_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
    .map_err(|error| error.to_string())
}

pub(crate) fn payment_method_counts_as_cash(method: &str) -> bool {
    method.eq_ignore_ascii_case("cash")
}

pub(crate) fn payment_method_label(method: &str) -> String {
    match method {
        "cash" => "Efectivo",
        "card" => "Tarjeta",
        "transfer" => "Transferencia",
        "credit" => "Credito",
        "voucher" => "Vale",
        other => other,
    }
    .to_string()
}

pub(crate) fn duration_minutes_between(opened_at: &str, closed_at: Option<&str>) -> i64 {
    let opened = chrono::DateTime::parse_from_rfc3339(opened_at).ok();
    let closed = closed_at
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .or_else(|| chrono::DateTime::parse_from_rfc3339(&now_iso()).ok());
    match (opened, closed) {
        (Some(start), Some(end)) => ((end - start).num_minutes()).max(0),
        _ => 0,
    }
}

pub(crate) fn merge_payment_breakdowns(cuts: &[ShiftCutSnapshot]) -> Vec<CutPaymentSummary> {
    let mut totals: HashMap<String, CutPaymentSummary> = HashMap::new();
    for cut in cuts {
        for payment in &cut.payment_breakdown {
            let entry = totals
                .entry(payment.method.clone())
                .or_insert_with(|| CutPaymentSummary {
                    method: payment.method.clone(),
                    label: payment.label.clone(),
                    amount: 0.0,
                    counts_as_cash: payment.counts_as_cash,
                });
            entry.amount = round_money(entry.amount + payment.amount);
        }
    }
    let mut result = totals.into_values().collect::<Vec<_>>();
    result.sort_by(|left, right| right.amount.total_cmp(&left.amount));
    result
}

pub(crate) fn merge_department_summaries(cuts: &[ShiftCutSnapshot]) -> Vec<CutDepartmentSummary> {
    let mut totals: HashMap<String, CutDepartmentSummary> = HashMap::new();
    for cut in cuts {
        for department in &cut.departments {
            let entry = totals
                .entry(department.category.clone())
                .or_insert_with(|| CutDepartmentSummary {
                    category: department.category.clone(),
                    quantity: 0.0,
                    total_sales: 0.0,
                    gross_profit: 0.0,
                });
            entry.quantity = round_money(entry.quantity + department.quantity);
            entry.total_sales = round_money(entry.total_sales + department.total_sales);
            entry.gross_profit = round_money(entry.gross_profit + department.gross_profit);
        }
    }
    let mut result = totals.into_values().collect::<Vec<_>>();
    result.sort_by(|left, right| right.total_sales.total_cmp(&left.total_sales));
    result
}

pub(crate) fn merge_tax_summaries(cuts: &[ShiftCutSnapshot]) -> Vec<CutTaxSummary> {
    let mut totals: HashMap<String, CutTaxSummary> = HashMap::new();
    for cut in cuts {
        for tax in &cut.taxes {
            let key = format!("{}|{}|{:.6}", tax.tax_name, tax.tax_type, tax.rate);
            let entry = totals.entry(key).or_insert_with(|| CutTaxSummary {
                tax_name: tax.tax_name.clone(),
                tax_type: tax.tax_type.clone(),
                rate: tax.rate,
                taxable_sales: 0.0,
                tax_collected: 0.0,
                gross_sales: 0.0,
            });
            entry.taxable_sales = round_money(entry.taxable_sales + tax.taxable_sales);
            entry.tax_collected = round_money(entry.tax_collected + tax.tax_collected);
            entry.gross_sales = round_money(entry.gross_sales + tax.gross_sales);
        }
    }
    let mut result = totals.into_values().collect::<Vec<_>>();
    result.sort_by(|left, right| right.tax_collected.total_cmp(&left.tax_collected));
    result
}

pub(crate) fn merge_customer_summaries(
    cuts: &[ShiftCutSnapshot],
    selector: fn(&ShiftCutSnapshot) -> &Vec<CutCustomerSummary>,
) -> Vec<CutCustomerSummary> {
    let mut totals: HashMap<i64, CutCustomerSummary> = HashMap::new();
    for cut in cuts {
        for customer in selector(cut) {
            let entry = totals
                .entry(customer.customer_id)
                .or_insert_with(|| CutCustomerSummary {
                    customer_id: customer.customer_id,
                    customer_name: customer.customer_name.clone(),
                    total_sales: 0.0,
                    gross_profit: 0.0,
                    ticket_count: 0,
                });
            entry.total_sales = round_money(entry.total_sales + customer.total_sales);
            entry.gross_profit = round_money(entry.gross_profit + customer.gross_profit);
            entry.ticket_count += customer.ticket_count;
        }
    }
    let mut result = totals.into_values().collect::<Vec<_>>();
    result.sort_by(|left, right| {
        right
            .total_sales
            .total_cmp(&left.total_sales)
            .then(right.gross_profit.total_cmp(&left.gross_profit))
    });
    result.truncate(5);
    result
}

pub(crate) fn calculate_shift_cut(conn: &Connection, shift_id: i64) -> CommandResult<ShiftCutSnapshot> {
    let (
        cash_session_id,
        workstation_id,
        status,
        opened_at,
        closed_at,
        opened_by_name,
        closed_by_name,
        opening_cash,
        closing_cash,
    ): (
        i64,
        Option<String>,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        f64,
        Option<f64>,
    ) = conn
        .query_row(
            "SELECT sh.cash_session_id, cs.workstation_id, sh.status, sh.opened_at, sh.closed_at, opener.name, closer.name,
                    sh.opening_cash, sh.closing_cash
             FROM shifts sh
             JOIN cash_sessions cs ON cs.id = sh.cash_session_id
             LEFT JOIN users opener ON opener.id = sh.opened_by
             LEFT JOIN users closer ON closer.id = sh.closed_by
             WHERE sh.id = ?1",
            params![shift_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )
        .map_err(|_| "Turno no encontrado".to_string())?;

    let duration_minutes = duration_minutes_between(&opened_at, closed_at.as_deref());
    let prices_include_tax = setting_bool(conn, "tax_prices_include_tax", true)?;

    let (total_tickets, canceled_tickets, net_sales, tax, discount): (i64, i64, f64, f64, f64) =
        conn.query_row(
            "SELECT
                SUM(CASE WHEN status = 'paid' THEN 1 ELSE 0 END),
                SUM(CASE WHEN status = 'canceled' THEN 1 ELSE 0 END),
                COALESCE(SUM(CASE WHEN status = 'paid' THEN total ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status = 'paid' THEN tax ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status = 'paid' THEN discount ELSE 0 END), 0)
             FROM sales
             WHERE shift_id = ?1",
            params![shift_id],
            |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                    row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .map_err(|error| error.to_string())?;

    let mut payment_totals = HashMap::<String, f64>::new();
    let mut payment_stmt = conn
        .prepare(
            "SELECT p.method, COALESCE(SUM(p.amount), 0)
             FROM payments p
             JOIN sales s ON s.id = p.sale_id
             WHERE s.shift_id = ?1 AND s.status = 'paid'
             GROUP BY p.method",
        )
        .map_err(|error| error.to_string())?;
    let payment_rows = payment_stmt
        .query_map(params![shift_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })
        .map_err(|error| error.to_string())?;
    for row in payment_rows {
        let (method, amount) = row.map_err(|error| error.to_string())?;
        payment_totals.insert(method, round_money(amount));
    }

    // Partial returns keep the sale 'paid', so net them out of the shift totals
    // here (they still appear in the Devoluciones section below), mirroring how
    // full cancellations are excluded from net_sales/tax/profit.
    let (returns_refund_total, returns_tax, returns_profit): (f64, f64, f64) = conn
        .query_row(
            "SELECT
                COALESCE(SUM(r.refund_total), 0),
                COALESCE(SUM(CASE WHEN sitm.tax_rate > 0 THEN r.refund_total - r.refund_total / (1 + sitm.tax_rate) ELSE 0 END), 0),
                COALESCE(SUM(r.refund_total - p.cost * r.quantity), 0)
             FROM sale_returns r
             JOIN sale_items sitm ON sitm.id = r.sale_item_id
             LEFT JOIN products p ON p.id = r.product_id
             WHERE r.shift_id = ?1",
            params![shift_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|error| error.to_string())?;
    let net_sales = net_sales - returns_refund_total;
    let tax = tax - returns_tax;

    let cash_change = conn
        .query_row(
            "SELECT COALESCE(SUM(s.change_due), 0)
             FROM sales s
             WHERE s.shift_id = ?1
               AND s.status = 'paid'
               AND EXISTS (SELECT 1 FROM payments p WHERE p.sale_id = s.id AND p.method = 'cash')",
            params![shift_id],
            |row| row.get::<_, f64>(0),
        )
        .map_err(|error| error.to_string())?;

    let raw_cash_paid = payment_totals.get("cash").copied().unwrap_or(0.0);
    let cash_paid = round_money((raw_cash_paid - cash_change).max(0.0));
    let card_paid = round_money(payment_totals.get("card").copied().unwrap_or(0.0));
    let transfer_paid = round_money(payment_totals.get("transfer").copied().unwrap_or(0.0));
    let credit_sales = round_money(payment_totals.get("credit").copied().unwrap_or(0.0));

    let mut payment_breakdown = payment_totals
        .into_iter()
        .map(|(method, amount)| CutPaymentSummary {
            amount: if method == "cash" { cash_paid } else { amount },
            label: payment_method_label(&method),
            counts_as_cash: payment_method_counts_as_cash(&method),
            method,
        })
        .collect::<Vec<_>>();
    payment_breakdown.sort_by(|left, right| right.amount.total_cmp(&left.amount));

    let mut cash_movement_stmt = conn
        .prepare(
            "SELECT m.id, m.movement_type, m.amount, m.reason, u.name, m.created_at
             FROM cash_movements m
             JOIN users u ON u.id = m.actor_id
             WHERE m.session_id = ?1
             ORDER BY m.created_at ASC, m.id ASC",
        )
        .map_err(|error| error.to_string())?;
    let cash_movement_rows = cash_movement_stmt
        .query_map(params![cash_session_id], |row| {
            Ok(CutCashMovementSummary {
                id: row.get(0)?,
                movement_type: row.get(1)?,
                amount: row.get(2)?,
                reason: row.get(3)?,
                actor_name: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|error| error.to_string())?;
    let cash_movements = cash_movement_rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let cash_entries_total = round_money(
        cash_movements
            .iter()
            .filter(|movement| movement.movement_type == "in")
            .map(|movement| movement.amount)
            .sum(),
    );
    let cash_out_total = round_money(
        cash_movements
            .iter()
            .filter(|movement| movement.movement_type == "out")
            .map(|movement| movement.amount)
            .sum(),
    );

    let mut refund_stmt = conn
        .prepare(
            "SELECT s.id, s.folio, s.total, s.change_due,
                    COALESCE((SELECT SUM(amount) FROM payments WHERE sale_id = s.id AND method = 'cash'), 0),
                    COALESCE(s.cancel_reason, s.notes, 'Cancelacion'),
                    COALESCE(s.canceled_at, s.created_at),
                    COALESCE((
                      SELECT GROUP_CONCAT(p.name || ' x' || printf('%g', si.quantity), ', ')
                      FROM sale_items si JOIN products p ON p.id = si.product_id
                      WHERE si.sale_id = s.id
                    ), '')
             FROM sales s
             WHERE s.shift_id = ?1 AND s.status = 'canceled'
             ORDER BY COALESCE(s.canceled_at, s.created_at) DESC, s.id DESC",
        )
        .map_err(|error| error.to_string())?;
    let refund_rows = refund_stmt
        .query_map(params![shift_id], |row| {
            let cash_amount = (row.get::<_, f64>(4)? - row.get::<_, f64>(3)?).max(0.0);
            Ok(CutRefundSummary {
                sale_id: row.get(0)?,
                folio: row.get(1)?,
                amount: row.get(2)?,
                cash_amount: round_money(cash_amount),
                reason: row.get(5)?,
                created_at: row.get(6)?,
                products: row.get(7)?,
            })
        })
        .map_err(|error| error.to_string())?;
    let mut refunds = refund_rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    // Partial per-item returns are recorded in sale_returns (the sale stays 'paid').
    let mut item_return_stmt = conn
        .prepare(
            "SELECT r.sale_id, s.folio, r.refund_total, r.cash_refund, r.reason, r.created_at,
                    COALESCE(p.name, 'Producto') || ' x' || printf('%g', r.quantity)
             FROM sale_returns r
             JOIN sales s ON s.id = r.sale_id
             LEFT JOIN products p ON p.id = r.product_id
             WHERE r.shift_id = ?1
             ORDER BY r.created_at DESC, r.id DESC",
        )
        .map_err(|error| error.to_string())?;
    let item_return_rows = item_return_stmt
        .query_map(params![shift_id], |row| {
            Ok(CutRefundSummary {
                sale_id: row.get(0)?,
                folio: row.get(1)?,
                amount: row.get(2)?,
                cash_amount: round_money(row.get::<_, f64>(3)?),
                reason: row.get(4)?,
                created_at: row.get(5)?,
                products: row.get(6)?,
            })
        })
        .map_err(|error| error.to_string())?;
    for row in item_return_rows {
        refunds.push(row.map_err(|error| error.to_string())?);
    }

    let cash_refunds_total = round_money(refunds.iter().map(|refund| refund.cash_amount).sum());

    let mut credit_payment_stmt = conn
        .prepare(
            "SELECT m.id, c.name, COALESCE(m.payment_method, 'cash'), ABS(m.amount), m.reason, m.created_at
             FROM customer_credit_movements m
             JOIN customers c ON c.id = m.customer_id
             WHERE m.cash_session_id = ?1 AND m.movement_kind = 'payment'
             ORDER BY m.created_at DESC, m.id DESC",
        )
        .map_err(|error| error.to_string())?;
    let credit_payment_rows = credit_payment_stmt
        .query_map(params![cash_session_id], |row| {
            Ok(CutCreditPaymentSummary {
                id: row.get(0)?,
                customer_name: row.get(1)?,
                payment_method: row.get(2)?,
                amount: row.get(3)?,
                reason: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|error| error.to_string())?;
    let credit_payments = credit_payment_rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let credit_payments_total = round_money(
        credit_payments
            .iter()
            .filter(|payment| payment_method_counts_as_cash(&payment.payment_method))
            .map(|payment| payment.amount)
            .sum(),
    );

    let mut department_stmt = conn
        .prepare(
            "SELECT COALESCE(p.category, '- Sin Departamento -'),
                    COALESCE(SUM(si.quantity), 0),
                    COALESCE(SUM(si.line_total), 0),
                    COALESCE(SUM(si.line_total - (p.cost * si.quantity)), 0)
             FROM sale_items si
             JOIN sales s ON s.id = si.sale_id
             JOIN products p ON p.id = si.product_id
             WHERE s.shift_id = ?1 AND s.status = 'paid'
             GROUP BY COALESCE(p.category, '- Sin Departamento -')
             ORDER BY SUM(si.line_total) DESC",
        )
        .map_err(|error| error.to_string())?;
    let department_rows = department_stmt
        .query_map(params![shift_id], |row| {
            Ok(CutDepartmentSummary {
                category: row.get(0)?,
                quantity: round_money(row.get(1)?),
                total_sales: round_money(row.get(2)?),
                gross_profit: round_money(row.get(3)?),
            })
        })
        .map_err(|error| error.to_string())?;
    let departments = department_rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let gross_profit = round_money(
        departments.iter().map(|department| department.gross_profit).sum::<f64>() - returns_profit,
    );

    let mut customer_stmt = conn
        .prepare(
            "SELECT s.id, c.id, c.name, s.total,
                    COALESCE((
                      SELECT SUM(si.line_total - (p.cost * si.quantity))
                      FROM sale_items si
                      JOIN products p ON p.id = si.product_id
                      WHERE si.sale_id = s.id
                    ), 0)
             FROM sales s
             JOIN customers c ON c.id = s.customer_id
             WHERE s.shift_id = ?1 AND s.status = 'paid' AND s.customer_id IS NOT NULL",
        )
        .map_err(|error| error.to_string())?;
    let customer_rows = customer_stmt
        .query_map(params![shift_id], |row| {
            Ok((
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, f64>(4)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut customer_totals: HashMap<i64, CutCustomerSummary> = HashMap::new();
    for row in customer_rows {
        let (customer_id, customer_name, total_sale, sale_profit) =
            row.map_err(|error| error.to_string())?;
        let entry = customer_totals
            .entry(customer_id)
            .or_insert(CutCustomerSummary {
                customer_id,
                customer_name,
                total_sales: 0.0,
                gross_profit: 0.0,
                ticket_count: 0,
            });
        entry.total_sales = round_money(entry.total_sales + total_sale);
        entry.gross_profit = round_money(entry.gross_profit + sale_profit);
        entry.ticket_count += 1;
    }
    let mut top_customers_by_sales = customer_totals.values().cloned().collect::<Vec<_>>();
    top_customers_by_sales.sort_by(|left, right| right.total_sales.total_cmp(&left.total_sales));
    top_customers_by_sales.truncate(5);
    let mut top_customers_by_profit = customer_totals.values().cloned().collect::<Vec<_>>();
    top_customers_by_profit
        .sort_by(|left, right| right.gross_profit.total_cmp(&left.gross_profit));
    top_customers_by_profit.truncate(5);

    let mut taxes = Vec::new();
    let mut tax_totals: HashMap<String, CutTaxSummary> = HashMap::new();
    let mut sale_item_stmt = conn
        .prepare(
            "SELECT si.product_id, si.quantity, si.unit_price, si.discount, si.tax_rate
             FROM sale_items si
             JOIN sales s ON s.id = si.sale_id
             WHERE s.shift_id = ?1 AND s.status = 'paid'",
        )
        .map_err(|error| error.to_string())?;
    let sale_item_rows = sale_item_stmt
        .query_map(params![shift_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, f64>(4)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    for row in sale_item_rows {
        let (product_id, quantity, unit_price, discount, fallback_rate) =
            row.map_err(|error| error.to_string())?;
        let mut tax_components = Vec::<(String, String, f64)>::new();
        let mut product_tax_stmt = conn
            .prepare(
                "SELECT t.name, t.type, t.rate
                 FROM product_taxes pt
                 JOIN taxes t ON t.id = pt.tax_id
                 WHERE pt.product_id = ?1 AND t.is_active = 1
                 ORDER BY t.id",
            )
            .map_err(|error| error.to_string())?;
        let product_tax_rows = product_tax_stmt
            .query_map(params![product_id], |tax_row| {
                Ok((
                    tax_row.get::<_, String>(0)?,
                    tax_row.get::<_, String>(1)?,
                    tax_row.get::<_, f64>(2)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        for tax_row in product_tax_rows {
            tax_components.push(tax_row.map_err(|error| error.to_string())?);
        }
        let effective_rate = if tax_components.is_empty() {
            fallback_rate
        } else {
            tax_components.iter().map(|(_, _, rate)| rate).sum()
        };
        if effective_rate <= 0.0 {
            continue;
        }
        let line_base = quantity * unit_price;
        let (line_subtotal, line_tax, _) =
            line_amounts(line_base, discount, effective_rate, prices_include_tax, true);
        if tax_components.is_empty() {
            let key = format!("fallback|{effective_rate:.6}");
            let entry = tax_totals.entry(key).or_insert(CutTaxSummary {
                tax_name: format!("Impuesto {:.2}%", effective_rate * 100.0),
                tax_type: "MIXTO".into(),
                rate: effective_rate,
                taxable_sales: 0.0,
                tax_collected: 0.0,
                gross_sales: 0.0,
            });
            entry.taxable_sales = round_money(entry.taxable_sales + line_subtotal);
            entry.tax_collected = round_money(entry.tax_collected + line_tax);
            entry.gross_sales = round_money(entry.gross_sales + line_subtotal + line_tax);
            continue;
        }
        for (name, tax_type, rate) in tax_components {
            let ratio = if effective_rate > 0.0 { rate / effective_rate } else { 0.0 };
            let allocated_tax = round_money(line_tax * ratio);
            let key = format!("{name}|{tax_type}|{rate:.6}");
            let entry = tax_totals.entry(key).or_insert(CutTaxSummary {
                tax_name: name,
                tax_type,
                rate,
                taxable_sales: 0.0,
                tax_collected: 0.0,
                gross_sales: 0.0,
            });
            entry.taxable_sales = round_money(entry.taxable_sales + line_subtotal);
            entry.tax_collected = round_money(entry.tax_collected + allocated_tax);
            entry.gross_sales = round_money(entry.gross_sales + line_subtotal + allocated_tax);
        }
    }
    taxes.extend(tax_totals.into_values());
    taxes.sort_by(|left, right| right.tax_collected.total_cmp(&left.tax_collected));

    let expected_cash = round_money(
        opening_cash + cash_paid + cash_entries_total - cash_out_total - cash_refunds_total
            + credit_payments_total,
    );
    let counted_income_total = round_money(
        cash_paid + cash_entries_total - cash_out_total - cash_refunds_total + credit_payments_total,
    );
    let net_sales = round_money(net_sales);
    let closing_cash = closing_cash.map(round_money);

    Ok(ShiftCutSnapshot {
        shift_id,
        cash_session_id,
        workstation_id,
        status,
        opened_at,
        closed_at,
        duration_minutes,
        opened_by_name,
        closed_by_name,
        total_tickets,
        canceled_tickets,
        gross_sales: round_money(net_sales + discount),
        net_sales,
        gross_profit,
        tax: round_money(tax),
        discount: round_money(discount),
        cash_paid,
        card_paid,
        transfer_paid,
        credit_sales,
        cash_entries_total,
        cash_out_total,
        cash_refunds_total,
        credit_payments_total,
        counted_income_total,
        average_ticket: average_ticket(net_sales, total_tickets),
        opening_cash: round_money(opening_cash),
        expected_cash,
        closing_cash,
        counted_cash: closing_cash,
        cash_difference: closing_cash.map(|value| round_money(value - expected_cash)),
        difference_reason: None,
        payment_breakdown,
        departments,
        cash_movements,
        refunds,
        credit_payments,
        taxes,
        top_customers_by_sales,
        top_customers_by_profit,
    })
}

