use crate::backend::{current_workstation_id, has_permission, require_permission, AppState, CommandResult};
use crate::core::{average_ticket, round_money};
use crate::models::*;
use rusqlite::{params, OptionalExtension};
use tauri::State;

#[tauri::command]
pub(crate) fn monthly_sales_report(
    state: State<'_, AppState>,
    actor_id: i64,
    month: Option<String>,
) -> CommandResult<Vec<MonthlySalesReport>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "reports")?;
    let mut sql = String::from(
        "SELECT
            strftime('%Y-%m', s.created_at) AS month,
            SUM(CASE WHEN s.status = 'paid' THEN 1 ELSE 0 END) AS total_tickets,
            COALESCE(SUM(CASE WHEN s.status = 'paid' THEN s.total ELSE 0 END), 0) AS total_amount,
            SUM(CASE WHEN s.status = 'canceled' THEN 1 ELSE 0 END) AS canceled_tickets
         FROM sales s
         JOIN shifts sh ON sh.id = s.shift_id
         WHERE sh.status = 'closed'",
    );
    if month.is_some() {
        sql.push_str(" AND strftime('%Y-%m', s.created_at) = ?1");
    }
    sql.push_str(" GROUP BY month ORDER BY month DESC");
    let mut stmt = conn.prepare(&sql).map_err(|error| error.to_string())?;
    let map_row = |row: &rusqlite::Row<'_>| {
        let month: String = row.get(0)?;
        let total_tickets: i64 = row.get::<_, Option<i64>>(1)?.unwrap_or(0);
        let total_amount: f64 = row.get(2)?;
        let canceled_tickets: i64 = row.get::<_, Option<i64>>(3)?.unwrap_or(0);
        Ok(MonthlySalesReport {
            month,
            total_tickets,
            total_amount: round_money(total_amount),
            average_ticket: average_ticket(total_amount, total_tickets),
            canceled_tickets,
        })
    };
    let rows = match month {
        Some(value) => stmt
            .query_map(params![value], map_row)
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>(),
        None => stmt
            .query_map([], map_row)
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>(),
    };
    rows.map_err(|error| error.to_string())
}
#[tauri::command]
pub(crate) fn report_summary(state: State<'_, AppState>, actor_id: i64) -> CommandResult<ReportSummary> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "reports")?;
    let workstation_id = current_workstation_id(&conn)?;
    let (today_sales, today_tickets): (f64, i64) = conn
        .query_row(
            "SELECT COALESCE(SUM(total), 0), COUNT(*)
             FROM sales
             WHERE date(created_at) = date('now') AND status = 'paid'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| error.to_string())?;
    let gross_profit = conn
        .query_row(
            "SELECT COALESCE(SUM((si.unit_price - p.cost) * si.quantity - si.discount), 0)
             FROM sale_items si
             JOIN sales s ON s.id = si.sale_id
             JOIN products p ON p.id = si.product_id
             WHERE date(s.created_at) = date('now') AND s.status = 'paid'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let cash_expected = conn
        .query_row(
            "SELECT COALESCE(expected_cash, 0) FROM cash_sessions WHERE status = 'open' AND workstation_id = ?1 ORDER BY id DESC LIMIT 1",
            params![workstation_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .unwrap_or(0.0);
    let (cash_sales, card_sales, transfer_sales): (f64, f64, f64) = conn
        .query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN p.method = 'cash' THEN p.amount ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN p.method = 'card' THEN p.amount ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN p.method = 'transfer' THEN p.amount ELSE 0 END), 0)
             FROM payments p
             JOIN sales s ON s.id = p.sale_id
             WHERE date(s.created_at) = date('now') AND s.status = 'paid'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|error| error.to_string())?;
    let low_stock_products = conn
        .query_row(
            "SELECT COUNT(*) FROM products WHERE active = 1 AND stock <= 0",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let can_view_profit = has_permission(&conn, actor_id, "view_profit")?;
    Ok(ReportSummary {
        today_sales,
        today_tickets,
        average_ticket: if today_tickets > 0 {
            round_money(today_sales / today_tickets as f64)
        } else {
            0.0
        },
        gross_profit: if can_view_profit { round_money(gross_profit) } else { 0.0 },
        cash_expected,
        cash_sales: round_money(cash_sales),
        card_sales: round_money(card_sales),
        transfer_sales: round_money(transfer_sales),
        low_stock_products,
    })
}

#[tauri::command]
pub(crate) fn report_product_sales(
    state: State<'_, AppState>,
    actor_id: i64,
    limit: Option<i64>,
    from_date: Option<String>,
    to_date: Option<String>,
) -> CommandResult<Vec<ProductSalesReport>> {
    let limit = limit.unwrap_or(20).clamp(1, 100);
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "reports")?;
    let mut stmt = conn
        .prepare(
            "SELECT
               p.id,
               p.name,
               p.category,
               COALESCE(SUM(si.quantity), 0),
               COALESCE(SUM(si.line_total), 0),
               COALESCE(SUM(si.line_total - (p.cost * si.quantity)), 0)
             FROM sale_items si
             JOIN sales s ON s.id = si.sale_id
             JOIN products p ON p.id = si.product_id
             WHERE s.status = 'paid'
               AND (?2 IS NULL OR date(s.created_at) >= date(?2))
               AND (?3 IS NULL OR date(s.created_at) <= date(?3))
             GROUP BY p.id, p.name, p.category
             ORDER BY SUM(si.line_total) DESC
             LIMIT ?1",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![limit, from_date, to_date], |row| {
            Ok(ProductSalesReport {
                product_id: row.get(0)?,
                product_name: row.get(1)?,
                category: row.get(2)?,
                quantity: row.get(3)?,
                total: row.get(4)?,
                gross_profit: row.get(5)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn report_unsold_products(
    state: State<'_, AppState>,
    actor_id: i64,
    from_date: Option<String>,
    to_date: Option<String>,
    limit: Option<i64>,
) -> CommandResult<Vec<ProductSalesReport>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "reports")?;
    let limit = limit.unwrap_or(100).clamp(1, 500);
    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.name, p.category
             FROM products p
             WHERE p.active = 1
               AND NOT EXISTS (
                 SELECT 1
                 FROM sale_items si
                 JOIN sales s ON s.id = si.sale_id
                 WHERE si.product_id = p.id
                   AND s.status = 'paid'
                   AND (?1 IS NULL OR date(s.created_at) >= date(?1))
                   AND (?2 IS NULL OR date(s.created_at) <= date(?2))
               )
             ORDER BY p.name
             LIMIT ?3",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![from_date, to_date, limit], |row| {
            Ok(ProductSalesReport {
                product_id: row.get(0)?,
                product_name: row.get(1)?,
                category: row.get(2)?,
                quantity: 0.0,
                total: 0.0,
                gross_profit: 0.0,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn report_tax_breakdown(
    state: State<'_, AppState>,
    actor_id: i64,
    from_date: Option<String>,
    to_date: Option<String>,
) -> CommandResult<Vec<TaxBreakdown>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "reports")?;
    let mut stmt = conn
        .prepare(
            "SELECT
               si.tax_rate,
               COALESCE(SUM(CASE
                 WHEN si.tax_rate > 0 THEN si.line_total / (1 + si.tax_rate)
                 ELSE si.line_total
               END), 0) AS taxable_sales,
               COALESCE(SUM(CASE
                 WHEN si.tax_rate > 0 THEN si.line_total - (si.line_total / (1 + si.tax_rate))
                 ELSE 0
               END), 0) AS tax_collected,
               COALESCE(SUM(si.line_total), 0) AS gross_sales
             FROM sale_items si
             JOIN sales s ON s.id = si.sale_id
             WHERE s.status = 'paid'
               AND (?1 IS NULL OR date(s.created_at) >= date(?1))
               AND (?2 IS NULL OR date(s.created_at) <= date(?2))
             GROUP BY si.tax_rate
             ORDER BY si.tax_rate DESC",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![from_date, to_date], |row| {
            Ok(TaxBreakdown {
                tax_rate: row.get(0)?,
                taxable_sales: round_money(row.get(1)?),
                tax_collected: round_money(row.get(2)?),
                gross_sales: round_money(row.get(3)?),
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn report_movement_history(
    state: State<'_, AppState>,
    actor_id: i64,
    limit: Option<i64>,
    from_date: Option<String>,
    to_date: Option<String>,
) -> CommandResult<Vec<ReportMovement>> {
    let limit = limit.unwrap_or(160).clamp(20, 500);
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "reports")?;
    let mut stmt = conn
        .prepare(
            "SELECT id, kind, title, detail, amount, gross_profit, cash_paid, card_paid, transfer_paid, tax_total, card_terminal, actor_name, cash_session_id, created_at
             FROM (
               SELECT
                 'sale-' || s.id AS id,
                 'sale' AS kind,
                 CASE WHEN s.status = 'paid' THEN 'Venta ' || s.folio ELSE 'Cancelacion ' || s.folio END AS title,
                 'Efectivo ' || printf('%.2f', COALESCE(pay.cash_paid, 0)) ||
                   ' · Tarjeta ' || printf('%.2f', COALESCE(pay.card_paid, 0)) ||
                   COALESCE(' · Terminal ' || pay.card_terminal, '') ||
                   ' · Transferencia ' || printf('%.2f', COALESCE(pay.transfer_paid, 0)) AS detail,
                 CASE WHEN s.status = 'paid' THEN s.total ELSE -s.total END AS amount,
                 CASE WHEN s.status = 'paid' THEN COALESCE(profit.gross_profit, 0) ELSE -COALESCE(profit.gross_profit, 0) END AS gross_profit,
                 COALESCE(pay.cash_paid, 0) AS cash_paid,
                 COALESCE(pay.card_paid, 0) AS card_paid,
                 COALESCE(pay.transfer_paid, 0) AS transfer_paid,
                 s.tax AS tax_total,
                 pay.card_terminal AS card_terminal,
                 u.name AS actor_name,
                 s.cash_session_id AS cash_session_id,
                 s.created_at AS created_at
               FROM sales s
               JOIN users u ON u.id = s.cashier_id
               LEFT JOIN (
                 SELECT
                   sale_id,
                   SUM(CASE WHEN method = 'cash' THEN amount ELSE 0 END) AS cash_paid,
                   SUM(CASE WHEN method = 'card' THEN amount ELSE 0 END) AS card_paid,
                   SUM(CASE WHEN method = 'transfer' THEN amount ELSE 0 END) AS transfer_paid,
                   MAX(CASE WHEN method = 'card' THEN reference ELSE NULL END) AS card_terminal
                 FROM payments
                 GROUP BY sale_id
               ) pay ON pay.sale_id = s.id
               LEFT JOIN (
                 SELECT
                   si.sale_id,
                   SUM(si.line_total - (p.cost * si.quantity)) AS gross_profit
                 FROM sale_items si
                 JOIN products p ON p.id = si.product_id
                 GROUP BY si.sale_id
               ) profit ON profit.sale_id = s.id
               UNION ALL
               SELECT
                 'purchase-' || pu.id,
                 'purchase',
                 'Compra ' || pu.id,
                 p.name || COALESCE(' · ' || s.name, ''),
                 -pu.total,
                 0,
                 0,
                 0,
                 0,
                 0,
                 NULL,
                 u.name,
                 NULL,
                 pu.created_at
               FROM purchases pu
               JOIN purchase_items pi ON pi.purchase_id = pu.id
               JOIN products p ON p.id = pi.product_id
               LEFT JOIN suppliers s ON s.id = pu.supplier_id
               LEFT JOIN users u ON u.id = pu.user_id
               UNION ALL
               SELECT
                 'cash-' || m.id,
                 'cash',
                 CASE
                   WHEN m.movement_type = 'in' THEN 'Entrada caja'
                   WHEN m.movement_type = 'out' THEN 'Retiro caja'
                   ELSE 'Cajon abierto'
                 END,
                 m.reason,
                 CASE
                   WHEN m.movement_type = 'in' THEN m.amount
                   WHEN m.movement_type = 'out' THEN -m.amount
                   ELSE 0
                 END,
                 0,
                 0,
                 0,
                 0,
                 0,
                 NULL,
                 u.name,
                 m.session_id,
                 m.created_at
               FROM cash_movements m
               JOIN users u ON u.id = m.actor_id
               UNION ALL
               SELECT
                 'inventory-' || im.id,
                 'inventory',
                 'Inventario ' || im.movement_type,
                 p.name || ' · ' || im.reason || ' · ' || printf('%.3f', im.quantity),
                 0,
                 0,
                 0,
                 0,
                 0,
                 0,
                 NULL,
                 NULL,
                 NULL,
                 im.created_at
               FROM inventory_movements im
               JOIN products p ON p.id = im.product_id
               UNION ALL
               SELECT
                 'credit-' || ccm.id,
                 'credit',
                 CASE WHEN ccm.amount > 0 THEN 'Cargo cliente' ELSE 'Abono cliente' END,
                 c.name || ' · ' || ccm.reason,
                 ccm.amount,
                 0,
                 0,
                 0,
                 0,
                 0,
                 NULL,
                 NULL,
                 NULL,
                 ccm.created_at
               FROM customer_credit_movements ccm
               JOIN customers c ON c.id = ccm.customer_id
               UNION ALL
               SELECT
                 'cut-open-' || cs.id,
                 'cut',
                 'Apertura caja ' || cs.id,
                 'Fondo inicial ' || printf('%.2f', cs.opening_cash),
                 cs.opening_cash,
                 0,
                 0,
                 0,
                 0,
                 0,
                 NULL,
                 u.name,
                 cs.id,
                 cs.opened_at
               FROM cash_sessions cs
               JOIN users u ON u.id = cs.opened_by
               UNION ALL
               SELECT
                 'cut-close-' || cs.id,
                 'cut',
                 'Corte caja ' || cs.id,
                 'Contado ' || printf('%.2f', COALESCE(cs.closing_cash, 0)) || ' · esperado ' || printf('%.2f', cs.expected_cash),
                 COALESCE(cs.closing_cash, cs.expected_cash),
                 0,
                 0,
                 0,
                 0,
                 0,
                 NULL,
                 u.name,
                 cs.id,
                 COALESCE(cs.closed_at, cs.opened_at)
               FROM cash_sessions cs
               JOIN users u ON u.id = cs.opened_by
               WHERE cs.closed_at IS NOT NULL
             )
             WHERE (?2 IS NULL OR date(created_at) >= date(?2))
               AND (?3 IS NULL OR date(created_at) <= date(?3))
             ORDER BY created_at DESC
             LIMIT ?1",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![limit, from_date, to_date], |row| {
            Ok(ReportMovement {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                detail: row.get(3)?,
                amount: row.get(4)?,
                gross_profit: row.get(5)?,
                cash_paid: row.get(6)?,
                card_paid: row.get(7)?,
                transfer_paid: row.get(8)?,
                tax_total: row.get(9)?,
                card_terminal: row.get(10)?,
                actor_name: row.get(11)?,
                cash_session_id: row.get(12)?,
                created_at: row.get(13)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}
