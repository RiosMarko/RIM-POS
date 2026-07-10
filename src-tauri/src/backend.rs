use crate::auth::{require_active_user, require_admin};
use crate::backup::{backup_create_with_conn, BackupResult};
use crate::cash::get_cash_session;
use crate::core::{format_quantity, now_iso, round_money, should_run_auto_backup};
#[cfg(test)]
use crate::core::{average_ticket, next_monthly_seq, period_key, visible_monthly_folio};
use crate::hardware::{
    device_list, read_serial_scale, run_print_file, temp_hardware_file, write_raw_device,
    HardwareDevice,
};
use crate::migrations::{configure_connection, init_db, migrate};
use crate::products::product_search_with_conn;
use crate::sales::held_ticket_list_with_conn;
#[cfg(test)]
use crate::security::legacy_hash_pin;
use crate::settings_access::{is_invoice_setting_key, is_public_setting_key};
#[cfg(test)]
use crate::validation;
use chrono::{DateTime, Duration, Local, Utc};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State};

pub(crate) struct AppState {
    pub(crate) db: Mutex<Connection>,
    pub(crate) db_path: PathBuf,
}

pub(crate) type CommandResult<T> = Result<T, String>;

const AUTO_BACKUP_LAST_SETTING: &str = "auto_backup_last_at";
const APP_RECOVERY_DIRTY_SETTING: &str = "app_recovery_dirty";
const APP_RECOVERY_LAST_MARKED_AT_SETTING: &str = "app_recovery_last_marked_at";

use crate::models::*;

pub(crate) fn setting_string(conn: &Connection, key: &str) -> CommandResult<Option<String>> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| error.to_string())
}

pub(crate) fn setting_bool(conn: &Connection, key: &str, default: bool) -> CommandResult<bool> {
    Ok(setting_string(conn, key)?
        .map(|value| value != "false")
        .unwrap_or(default))
}

pub(crate) fn current_workstation_id(conn: &Connection) -> CommandResult<String> {
    let fallback = env::var("COMPUTERNAME")
        .or_else(|_| env::var("HOSTNAME"))
        .unwrap_or_else(|_| "CAJA-1".into());
    Ok(setting_string(conn, "workstation_id")?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback)
        .trim()
        .chars()
        .take(40)
        .collect())
}


pub(crate) fn line_amounts(
    gross_or_net: f64,
    discount: f64,
    tax_rate: f64,
    prices_include_tax: bool,
    tax_enabled: bool,
) -> (f64, f64, f64) {
    let taxable = (gross_or_net - discount).max(0.0);
    if !tax_enabled || tax_rate <= 0.0 {
        return (round_money(taxable), 0.0, round_money(taxable));
    }
    if prices_include_tax {
        let subtotal = taxable / (1.0 + tax_rate);
        let tax = taxable - subtotal;
        return (
            round_money(subtotal),
            round_money(tax),
            round_money(taxable),
        );
    }
    let tax = taxable * tax_rate;
    (
        round_money(taxable),
        round_money(tax),
        round_money(taxable + tax),
    )
}

pub(crate) fn require_permission(conn: &Connection, actor_id: i64, permission: &str) -> CommandResult<()> {
    if has_permission(conn, actor_id, permission)? {
        Ok(())
    } else {
        Err("Permiso requerido".into())
    }
}

pub(crate) fn has_permission(conn: &Connection, actor_id: i64, permission: &str) -> CommandResult<bool> {
    let actor = require_active_user(conn, actor_id)?;
    if actor.role == "admin" {
        return Ok(true);
    }
    // The 'admin' permission elevates a cashier to full access, so it satisfies
    // any specific permission check too.
    let allowed: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM user_permissions WHERE user_id = ?1 AND permission_key IN (?2, 'admin')",
            params![actor_id, permission],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    Ok(allowed > 0)
}


fn today_utc_bounds() -> CommandResult<(String, String)> {
    let start = Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| "Fecha actual invalida".to_string())?
        .and_utc();
    let end = start + Duration::days(1);
    Ok((start.to_rfc3339(), end.to_rfc3339()))
}

fn dashboard_summary_with_conn(
    conn: &Connection,
    actor_id: i64,
) -> CommandResult<DashboardSummary> {
    require_active_user(conn, actor_id)?;
    let workstation_id = current_workstation_id(conn)?;
    let (active_products, low_stock_products): (i64, i64) = conn
        .query_row(
            "SELECT
                COUNT(*),
                COALESCE(SUM(CASE WHEN stock <= 0 THEN 1 ELSE 0 END), 0)
             FROM products
             WHERE active = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| error.to_string())?;
    let (start, end) = today_utc_bounds()?;
    let (today_sales, today_tickets): (f64, i64) = conn
        .query_row(
            "SELECT COALESCE(SUM(total), 0), COUNT(*)
             FROM sales
             WHERE created_at >= ?1 AND created_at < ?2",
            params![start, end],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| error.to_string())?;
    let open_cash_session_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM cash_sessions WHERE status = 'open' AND workstation_id = ?1 ORDER BY id DESC LIMIT 1",
            params![workstation_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let open_cash_session = match open_cash_session_id {
        Some(id) => Some(get_cash_session(conn, id)?),
        None => None,
    };
    Ok(DashboardSummary {
        active_products,
        low_stock_products,
        today_sales,
        today_tickets,
        open_cash_session,
    })
}

#[tauri::command]
fn dashboard_summary(state: State<'_, AppState>, actor_id: i64) -> CommandResult<DashboardSummary> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    dashboard_summary_with_conn(&conn, actor_id)
}

#[tauri::command]
fn app_bootstrap(state: State<'_, AppState>, actor_id: i64) -> CommandResult<AppBootstrap> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    let summary = dashboard_summary_with_conn(&conn, actor_id)?;
    let products = product_search_with_conn(&conn, "", 40, 0)?;
    let held_tickets = held_ticket_list_with_conn(&conn)?;
    let tax_enabled = setting_bool(&conn, "tax_enabled", true)?;
    let tax_prices_include_tax = setting_bool(&conn, "tax_prices_include_tax", true)?;
    let total_round_up = setting_bool(&conn, "total_round_up", true)?;
    let unclean_shutdown = setting_bool(&conn, APP_RECOVERY_DIRTY_SETTING, false)?;
    let now = now_iso();
    conn.execute(
        "INSERT INTO app_settings (key, value, updated_at)
         VALUES (?1, 'true', ?2)
         ON CONFLICT(key) DO UPDATE SET value = 'true', updated_at = excluded.updated_at",
        params![APP_RECOVERY_DIRTY_SETTING, now],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT INTO app_settings (key, value, updated_at)
         VALUES (?1, ?2, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![APP_RECOVERY_LAST_MARKED_AT_SETTING, now],
    )
    .map_err(|error| error.to_string())?;
    Ok(AppBootstrap {
        summary,
        products,
        held_tickets,
        tax_enabled,
        tax_prices_include_tax,
        total_round_up,
        unclean_shutdown,
    })
}

#[tauri::command]
fn app_recovery_mark_clean(state: State<'_, AppState>, actor_id: i64) -> CommandResult<()> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, actor_id)?;
    let now = now_iso();
    conn.execute(
        "INSERT INTO app_settings (key, value, updated_at)
         VALUES (?1, 'false', ?2)
         ON CONFLICT(key) DO UPDATE SET value = 'false', updated_at = excluded.updated_at",
        params![APP_RECOVERY_DIRTY_SETTING, now],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT INTO app_settings (key, value, updated_at)
         VALUES (?1, ?2, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![APP_RECOVERY_LAST_MARKED_AT_SETTING, now],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}


#[tauri::command]
fn hardware_device_list(
    state: State<'_, AppState>,
    actor_id: i64,
    include_network: Option<bool>,
) -> CommandResult<Vec<HardwareDevice>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_admin(&conn, actor_id)?;
    drop(conn);
    Ok(device_list(include_network.unwrap_or(false)))
}

pub(crate) fn ticket_setting(conn: &Connection, key: &str, default: &str) -> CommandResult<String> {
    Ok(setting_string(conn, key)?.unwrap_or_else(|| default.to_string()))
}

pub(crate) fn cut_printer_setting(conn: &Connection) -> CommandResult<String> {
    let dedicated = ticket_setting(conn, "cut_printer", "")?;
    if !dedicated.trim().is_empty() {
        return Ok(dedicated);
    }
    ticket_setting(conn, "printer", "")
}

pub(crate) fn ticket_setting_i64(
    conn: &Connection,
    key: &str,
    default: i64,
    min: i64,
    max: i64,
) -> CommandResult<i64> {
    let value = setting_string(conn, key)?
        .and_then(|raw| raw.parse::<i64>().ok())
        .unwrap_or(default);
    Ok(value.clamp(min, max))
}

pub(crate) fn ticket_separator(width: usize) -> String {
    "-".repeat(width.clamp(24, 48))
}

/// Left-pads a line with spaces so it sits centered within the paper width.
/// Lines already as wide as (or wider than) the paper are left untouched.
fn center_line(text: &str, width: usize) -> String {
    let len = text.chars().count();
    if len >= width {
        return text.to_string();
    }
    format!("{}{}", " ".repeat((width - len) / 2), text)
}

fn receipt_text(conn: &Connection, sale_id: i64) -> CommandResult<String> {
    let width = ticket_setting_i64(conn, "ticket_width", 32, 24, 48)? as usize;
    let separator = ticket_separator(width);
    let store_name = ticket_setting(conn, "ticket_store_name", "RIM-POS")?;
    let header = ticket_setting(conn, "ticket_header", "Abarrotes y miscelanea")?;
    let footer = ticket_setting(conn, "ticket_footer", "Gracias por su compra")?;
    let show_logo = setting_bool(conn, "ticket_show_logo", true)?;
    let show_date = setting_bool(conn, "ticket_show_date", true)?;
    let show_cashier = setting_bool(conn, "ticket_show_cashier", true)?;
    let show_barcode = setting_bool(conn, "ticket_show_barcode", false)?;
    let show_item_count = setting_bool(conn, "ticket_show_item_count", true)?;
    let start_lines = ticket_setting_i64(conn, "ticket_start_lines", 0, 0, 8)? as usize;
    let extra_lines = ticket_setting_i64(conn, "ticket_extra_lines", 3, 0, 8)? as usize;
    let show_tax = setting_bool(conn, "tax_show_breakdown", true)?;

    if sale_id <= 0 {
        let mut demo = String::new();
        demo.push_str(&"\n".repeat(start_lines));
        if show_logo {
            demo.push_str(&format!("{store_name}\n"));
        }
        if !header.trim().is_empty() {
            demo.push_str(&header);
            demo.push('\n');
        }
        demo.push_str("Prueba de impresora\n");
        if show_date {
            demo.push_str("2026-06-20 08:20\n");
        }
        demo.push_str(&format!("{separator}\n*** OK ***\n"));
        if !footer.trim().is_empty() {
            demo.push('\n');
            demo.push_str(&footer);
            demo.push('\n');
        }
        demo.push_str(&"\n".repeat(extra_lines));
        return Ok(demo);
    }
    let (folio, subtotal, tax, rounding, total, paid, change_due, created_at, cashier_name): (
        String,
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
        String,
        String,
    ) = conn
        .query_row(
            "SELECT s.folio, s.subtotal, s.tax, s.rounding, s.total, s.paid, s.change_due, s.created_at, u.name
             FROM sales s
             JOIN users u ON u.id = s.cashier_id
             WHERE s.id = ?1",
            params![sale_id],
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
        .map_err(|_| format!("Venta no encontrada: {sale_id}"))?;
    let mut text = String::new();
    text.push_str(&"\n".repeat(start_lines));
    if show_logo {
        text.push_str(&format!("{store_name}\n"));
    }
    if !header.trim().is_empty() {
        // Push the header as typed, not trimmed: a user indenting a line with
        // leading spaces (e.g. to hand-center a title on 32-col paper) had
        // that indentation silently eaten here even though it displays fine
        // in the settings preview, which never trims. Only the emptiness
        // check needs .trim(); the printed content should not.
        text.push_str(&header);
        text.push('\n');
    }
    text.push_str(&format!("Folio {folio}\n"));
    if show_date {
        match DateTime::parse_from_rfc3339(&created_at) {
            Ok(parsed) => {
                let local = parsed.with_timezone(&Local);
                text.push_str(&format!(
                    "{}   {}\n",
                    local.format("%Y-%m-%d"),
                    local.format("%-I:%M %P")
                ));
            }
            Err(_) => text.push_str(&format!("{created_at}\n")),
        }
    }
    if show_cashier {
        text.push_str(&format!("Cajero: {cashier_name}\n"));
    }
    text.push_str(&format!("{separator}\n"));
    let mut stmt = conn
        .prepare(
            "SELECT p.name, p.barcode, si.quantity, si.unit_price, si.discount, si.line_total
             FROM sale_items si
             JOIN products p ON p.id = si.product_id
             WHERE si.sale_id = ?1
             ORDER BY si.id",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![sale_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, f64>(5)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut item_count = 0.0;
    for row in rows {
        let (name, barcode, quantity, unit_price, discount, line_total) =
            row.map_err(|error| error.to_string())?;
        item_count += quantity;
        text.push_str(&format!(
            "{name}\n  {} @ ${unit_price:.2}",
            format_quantity(quantity)
        ));
        if discount > 0.0 {
            text.push_str(&format!(" desc ${discount:.2}"));
        }
        text.push_str(&format!("  ${line_total:.2}\n"));
        if show_barcode && !barcode.trim().is_empty() {
            text.push_str(&format!("  {barcode}\n"));
        }
        text.push('\n');
    }
    text.push_str(&format!("{separator}\n"));
    if show_tax {
        text.push_str(&format!(
            "SUBTOTAL        ${subtotal:.2}\nIMPUESTOS       ${tax:.2}\n"
        ));
    }
    if rounding.abs() >= 0.005 {
        text.push_str(&format!("REDONDEO        ${rounding:.2}\n"));
    }
    text.push_str(&format!("*** TOTAL       ${total:.2}\nPAGADO          ${paid:.2}\nCAMBIO          ${change_due:.2}\n"));
    if show_item_count {
        text.push_str(&format!("Articulos: {}\n", format_quantity(item_count)));
    }
    if !footer.trim().is_empty() {
        text.push('\n');
        // Same reasoning as the header above: keep the user's own spacing.
        text.push_str(&footer);
        text.push('\n');
    }

    // Card sales get a credit voucher at the bottom: label, signature space and
    // the terminal used (stored in the card payment's reference).
    let (card_count, card_terminal): (i64, Option<String>) = conn
        .query_row(
            "SELECT COUNT(*),
                    MAX(CASE WHEN TRIM(COALESCE(reference, '')) <> '' THEN reference END)
             FROM payments WHERE sale_id = ?1 AND method = 'card'",
            params![sale_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| error.to_string())?;
    if card_count > 0 {
        // One blank line of breathing room before the credit-voucher block,
        // and every line in it centered on the paper width.
        text.push('\n');
        text.push_str(&format!("{separator}\n"));
        text.push_str(&format!("{}\n", center_line("VENTA A CREDITO", width)));
        text.push_str(&format!("{}\n", center_line("FIRMA DEL CLIENTE", width)));
        text.push_str("\n\n\n");
        text.push_str(&format!("{}\n", "_".repeat(width)));
        if let Some(terminal) = card_terminal {
            let terminal = terminal.trim();
            if !terminal.is_empty() {
                text.push_str(&format!("{}\n", center_line(&format!("TERMINAL {terminal}"), width)));
            }
        }
    }

    text.push_str(&"\n".repeat(extra_lines));
    Ok(text)
}

/// Wraps the receipt text in ESC/POS control codes: initialize (ESC @), then the
/// content, then a feed + partial cut (GS V B). Sent raw to the printer so CUPS
/// does not re-paginate a 58mm ticket into a full letter-size page (the "tira
/// larga" bug). Cut bytes are ignored by printers without an auto-cutter.
/// Thermal/ESC-POS clones default to codepage 437 or 850, not UTF-8. Sending
/// Rust's native UTF-8 bytes straight through makes every accented Spanish
/// letter print as garbage (each UTF-8 continuation byte gets interpreted on
/// its own). This maps the accented characters that actually show up in
/// tickets (store name, product names, "Codigo", footer, etc.) to their
/// single-byte CP850 codepoints, and falls back to the plain unaccented
/// letter for anything else non-ASCII -- readable-without-accent beats a
/// wrong symbol if a character isn't in the table.
fn encode_cp850(text: &str) -> Vec<u8> {
    text.chars()
        .map(|character| match character {
            'á' => 0xA0, 'é' => 0x82, 'í' => 0xA1, 'ó' => 0xA2, 'ú' => 0xA3,
            'Á' => 0xB5, 'É' => 0x90, 'Í' => 0xD6, 'Ó' => 0xE0, 'Ú' => 0xE9,
            'ñ' => 0xA4, 'Ñ' => 0xA5,
            'ü' => 0x81, 'Ü' => 0x9A,
            '¿' => 0xA8, '¡' => 0xAD,
            _ if (character as u32) < 128 => character as u8,
            'à' | 'â' | 'ä' => b'a',
            'è' | 'ê' | 'ë' => b'e',
            'ì' | 'î' | 'ï' => b'i',
            'ò' | 'ô' | 'ö' => b'o',
            'ù' | 'û' => b'u',
            _ => b'?',
        })
        .collect()
}

pub(crate) fn escpos_ticket_bytes(text: &str, copies: i64) -> Vec<u8> {
    let mut bytes = Vec::new();
    for _ in 0..copies.max(1) {
        bytes.extend_from_slice(&[0x1B, 0x40]); // ESC @  -> reset printer
        bytes.extend_from_slice(&[0x1B, 0x74, 0x02]); // ESC t 2 -> codepage CP850 (matches encode_cp850)
        bytes.extend_from_slice(&[0x1B, 0x45, 0x01]); // ESC E 1 -> emphasized (bold)
        bytes.extend_from_slice(&[0x1B, 0x47, 0x01]); // ESC G 1 -> double-strike (darker)
        bytes.extend_from_slice(&encode_cp850(text));
        bytes.extend_from_slice(b"\n\n\n"); // feed before cut
        bytes.extend_from_slice(&[0x1D, 0x56, 0x42, 0x00]); // GS V B 0 -> feed + partial cut
    }
    bytes
}

// Prints the receipt as plain text through the printer's driver. Correct for a
// normal printer (Brother, laser, inkjet): sending it ESC/POS raw would jam the
// queue because the driver can't interpret raw bytes.
fn print_ticket_text(printer: &str, text: &str, copies: i64) -> CommandResult<()> {
    let text = if copies > 1 {
        (0..copies).map(|_| text.to_string()).collect::<Vec<_>>().join("\n")
    } else {
        text.to_string()
    };
    let file = temp_hardware_file("rim-pos-ticket", "txt");
    fs::write(&file, text).map_err(|error| format!("No se pudo crear ticket temporal: {error}"))?;
    let result = run_print_file(printer, &file, false);
    let _ = fs::remove_file(file);
    result
}

#[tauri::command]
fn print_ticket(state: State<'_, AppState>, sale_id: i64) -> CommandResult<HardwareResult> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    let printer = setting_string(&conn, "printer")?.unwrap_or_default();
    let copies = ticket_setting_i64(&conn, "ticket_copies", 1, 1, 4)?;
    // Thermal ESC/POS (POS-58) by default; switch off for a normal driver
    // printer (Brother etc.) so its queue doesn't jam with raw bytes.
    let escpos = setting_bool(&conn, "ticket_escpos", true)?;
    let text = receipt_text(&conn, sale_id)?;
    drop(conn);

    if !escpos {
        print_ticket_text(&printer, &text, copies)?;
        return Ok(HardwareResult {
            ok: true,
            message: format!("Ticket enviado a {printer}"),
        });
    }

    // Send ESC/POS raw so CUPS (macOS/Linux) and raw/network/COM ports (any OS)
    // print the ticket at 58mm and cut, instead of stretching it to a full page.
    let bytes = escpos_ticket_bytes(&text, copies);
    let raw_file = temp_hardware_file("rim-pos-ticket", "bin");
    fs::write(&raw_file, bytes).map_err(|error| format!("No se pudo crear ticket temporal: {error}"))?;
    let raw_result = run_print_file(&printer, &raw_file, true);
    let _ = fs::remove_file(&raw_file);

    // POS-58 mode must fail loudly if RAW ESC/POS cannot reach queue. A text
    // fallback reports success but skips thermal formatting and paper cut.
    raw_result?;

    Ok(HardwareResult {
        ok: true,
        message: format!("Ticket enviado a {printer}"),
    })
}

#[tauri::command]
fn open_cash_drawer(state: State<'_, AppState>) -> CommandResult<HardwareResult> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    let drawer = setting_string(&conn, "drawer")?
        .or_else(|| setting_string(&conn, "printer").ok().flatten())
        .unwrap_or_default();
    drop(conn);
    let pulse = [0x1B, 0x70, 0x00, 0x40, 0x50];
    if write_raw_device(&drawer, &pulse)? {
        return Ok(HardwareResult {
            ok: true,
            message: format!("Pulso de cajon enviado directo a {drawer}"),
        });
    }
    let file = temp_hardware_file("rim-pos-drawer", "bin");
    fs::write(&file, pulse).map_err(|error| format!("No se pudo crear pulso de cajon: {error}"))?;
    run_print_file(&drawer, &file, true)?;
    let _ = fs::remove_file(file);
    Ok(HardwareResult {
        ok: true,
        message: format!("Pulso de cajon enviado a {drawer}"),
    })
}

#[tauri::command]
fn read_scale(state: State<'_, AppState>) -> CommandResult<ScaleReading> {
    let (scale, baud_rate) = {
        let conn = state.db.lock().map_err(|error| error.to_string())?;
        (
            setting_string(&conn, "scale")?.unwrap_or_default(),
            ticket_setting_i64(&conn, "scale_baud_rate", 9600, 1200, 115200)? as u32,
        )
    };
    let mut candidates = vec![baud_rate, 9600, 4800, 2400, 19200, 38400];
    candidates.dedup();
    let mut last_error = None;
    let mut result = None;
    for candidate in candidates {
        match read_serial_scale(&scale, candidate, 1200) {
            Ok((weight, raw)) => {
                result = Some((weight, raw, candidate));
                break;
            }
            Err(error) => last_error = Some(error),
        }
    }
    let (weight, raw, detected_baud_rate) =
        result.ok_or_else(|| last_error.unwrap_or_else(|| "No se pudo leer bascula".into()))?;
    if detected_baud_rate != baud_rate {
        let conn = state.db.lock().map_err(|error| error.to_string())?;
        conn.execute(
            "INSERT INTO app_settings (key, value, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params!["scale_baud_rate", detected_baud_rate.to_string(), now_iso()],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(ScaleReading {
        ok: true,
        weight,
        unit: "kg".into(),
        source: raw,
        baud_rate: detected_baud_rate,
    })
}

#[tauri::command]
fn settings_get(
    state: State<'_, AppState>,
    actor_id: i64,
    key: String,
) -> CommandResult<Option<String>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    if is_public_setting_key(&key) {
        require_active_user(&conn, actor_id)?;
    } else if is_invoice_setting_key(&key) {
        require_permission(&conn, actor_id, "invoices")?;
    } else {
        require_admin(&conn, actor_id)?;
    }
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn settings_get_many(
    state: State<'_, AppState>,
    actor_id: i64,
    keys: Vec<String>,
) -> CommandResult<HashMap<String, Option<String>>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    let mut values = HashMap::with_capacity(keys.len());
    for key in keys {
        if is_public_setting_key(&key) {
            require_active_user(&conn, actor_id)?;
        } else if is_invoice_setting_key(&key) {
            require_permission(&conn, actor_id, "invoices")?;
        } else {
            require_admin(&conn, actor_id)?;
        }
        let value = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        values.insert(key, value);
    }
    Ok(values)
}

#[tauri::command]
fn settings_set(
    state: State<'_, AppState>,
    actor_id: i64,
    key: String,
    value: String,
) -> CommandResult<()> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    if is_invoice_setting_key(&key) {
        require_permission(&conn, actor_id, "invoices")?;
    } else {
        require_admin(&conn, actor_id)?;
    }
    conn.execute(
        "INSERT INTO app_settings (key, value, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, now_iso()],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn settings_set_many(
    state: State<'_, AppState>,
    actor_id: i64,
    entries: HashMap<String, String>,
) -> CommandResult<()> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    let updated_at = now_iso();
    for (key, value) in entries {
        if is_invoice_setting_key(&key) {
            require_permission(&conn, actor_id, "invoices")?;
        } else {
            require_admin(&conn, actor_id)?;
        }
        conn.execute(
            "INSERT INTO app_settings (key, value, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, updated_at],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn backup_create(state: State<'_, AppState>, actor_id: i64) -> CommandResult<BackupResult> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_admin(&conn, actor_id)?;
    let backup = backup_create_with_conn(&conn, &state.db_path)?;
    conn.execute(
        "INSERT INTO audit_log (actor_id, action, entity, entity_id, details, created_at)
         VALUES (?1, 'backup_create', 'backup', NULL, ?2, ?3)",
        params![actor_id, backup.path, backup.created_at],
    )
    .map_err(|error| error.to_string())?;
    Ok(backup)
}

#[tauri::command]
fn backup_export_desktop(state: State<'_, AppState>, actor_id: i64) -> CommandResult<BackupResult> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_admin(&conn, actor_id)?;
    let backup = backup_create_with_conn(&conn, &state.db_path)?;
    let home = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .map_err(|_| "No se pudo localizar Escritorio".to_string())?;
    let export_dir = PathBuf::from(home).join("Desktop").join("RIM-POS-backups");
    fs::create_dir_all(&export_dir).map_err(|error| error.to_string())?;
    let file_name = PathBuf::from(&backup.path)
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .ok_or_else(|| "Backup sin nombre de archivo".to_string())?;
    let export_path = export_dir.join(file_name);
    fs::copy(&backup.path, &export_path).map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT INTO audit_log (actor_id, action, entity, entity_id, details, created_at)
         VALUES (?1, 'backup_export', 'backup', NULL, ?2, ?3)",
        params![actor_id, export_path.to_string_lossy().to_string(), backup.created_at],
    )
    .map_err(|error| error.to_string())?;
    Ok(BackupResult {
        path: export_path.to_string_lossy().to_string(),
        created_at: backup.created_at,
    })
}

#[tauri::command]
fn export_file(state: State<'_, AppState>, actor_id: i64, path: String, contents: Vec<u8>) -> CommandResult<()> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, actor_id)?;
    fs::write(&path, contents).map_err(|error| error.to_string())?;
    Ok(())
}

fn backup_dir_for(db_path: &PathBuf) -> CommandResult<PathBuf> {
    db_path
        .parent()
        .ok_or_else(|| "Ruta DB invalida".to_string())
        .map(|path| path.join("backups"))
}

fn sidecar_path(db_path: &PathBuf, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}{}", db_path.to_string_lossy(), suffix))
}

fn validate_restore_backup(db_path: &PathBuf, path: &str) -> CommandResult<PathBuf> {
    let backup_dir = backup_dir_for(db_path)?;
    let backup_dir = backup_dir
        .canonicalize()
        .map_err(|_| "Carpeta de backups no disponible".to_string())?;
    let requested = PathBuf::from(path)
        .canonicalize()
        .map_err(|_| "Backup no encontrado".to_string())?;
    if !requested.starts_with(&backup_dir) {
        return Err("Backup fuera de carpeta segura".into());
    }
    let name = requested
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();
    if !name.starts_with("pos-backup-") || !name.ends_with(".sqlite3") {
        return Err("Archivo backup invalido".into());
    }
    let validation = Connection::open_with_flags(&requested, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("Backup no abre: {error}"))?;
    let integrity: String = validation
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| format!("Backup no se pudo validar: {error}"))?;
    if integrity != "ok" {
        return Err(format!("Backup dañado: {integrity}"));
    }
    for table in ["users", "products", "sales", "app_settings"] {
        let exists: Option<String> = validation
            .query_row(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1",
                params![table],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if exists.is_none() {
            return Err(format!("Backup no parece ser de RIM-POS: falta {table}"));
        }
    }
    Ok(requested)
}

#[tauri::command]
fn backup_restore(
    state: State<'_, AppState>,
    actor_id: i64,
    path: String,
) -> CommandResult<BackupRestoreResult> {
    let requested = validate_restore_backup(&state.db_path, &path)?;
    let restored_at = now_iso();
    let temp_restore = state.db_path.with_extension("restore-tmp");
    fs::copy(&requested, &temp_restore)
        .map_err(|error| format!("No se pudo preparar restauracion: {error}"))?;

    let mut conn = state.db.lock().map_err(|error| error.to_string())?;
    require_admin(&conn, actor_id)?;
    let safety_backup = backup_create_with_conn(&conn, &state.db_path)?;
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|error| error.to_string())?;

    let memory_conn = Connection::open_in_memory().map_err(|error| error.to_string())?;
    let old_conn = std::mem::replace(&mut *conn, memory_conn);
    drop(old_conn);

    let wal_path = sidecar_path(&state.db_path, "-wal");
    let shm_path = sidecar_path(&state.db_path, "-shm");
    let _ = fs::remove_file(&wal_path);
    let _ = fs::remove_file(&shm_path);

    let restore_result = fs::copy(&temp_restore, &state.db_path);
    let _ = fs::remove_file(&temp_restore);
    if let Err(error) = restore_result {
        let _ = fs::copy(&safety_backup.path, &state.db_path);
        let reopened = Connection::open(&state.db_path).map_err(|open_error| {
            format!("Restore fallo: {error}. Reabrir backup de seguridad fallo: {open_error}")
        })?;
        configure_connection(&reopened)?;
        migrate(&reopened)?;
        *conn = reopened;
        return Err(format!("No se pudo restaurar backup: {error}"));
    }

    let reopened = Connection::open(&state.db_path).map_err(|error| error.to_string())?;
    configure_connection(&reopened)?;
    migrate(&reopened)?;
    let _ = reopened.execute(
        "INSERT INTO audit_log (actor_id, action, entity, entity_id, details, created_at)
         VALUES (?1, 'backup_restore', 'backup', NULL, ?2, ?3)",
        params![actor_id, requested.to_string_lossy().to_string(), restored_at],
    );
    *conn = reopened;

    Ok(BackupRestoreResult {
        restored_path: requested.to_string_lossy().to_string(),
        safety_backup_path: safety_backup.path,
        restored_at,
    })
}

#[tauri::command]
fn backup_list(state: State<'_, AppState>, actor_id: i64) -> CommandResult<Vec<BackupFile>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_admin(&conn, actor_id)?;
    let backup_dir = backup_dir_for(&state.db_path)?;
    let mut files = Vec::new();
    if !backup_dir.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(backup_dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("pos-backup-") || !name.ends_with(".sqlite3") {
            continue;
        }
        let metadata = entry.metadata().map_err(|error| error.to_string())?;
        files.push(BackupFile {
            path: path.to_string_lossy().to_string(),
            name,
            size_bytes: metadata.len(),
            created_at: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| {
                    chrono::DateTime::<Utc>::from(std::time::UNIX_EPOCH + duration).to_rfc3339()
                })
                .unwrap_or_else(now_iso),
        });
    }
    files.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(files)
}

#[tauri::command]
fn backup_auto_if_due(
    state: State<'_, AppState>,
    actor_id: i64,
) -> CommandResult<Option<BackupResult>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, actor_id)?;
    let last_backup_at = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            params![AUTO_BACKUP_LAST_SETTING],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if !should_run_auto_backup(last_backup_at) {
        return Ok(None);
    }
    let backup = backup_create_with_conn(&conn, &state.db_path)?;
    conn.execute(
        "INSERT INTO app_settings (key, value, updated_at)
         VALUES (?1, ?2, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![AUTO_BACKUP_LAST_SETTING, backup.created_at],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT INTO audit_log (actor_id, action, entity, entity_id, details, created_at)
         VALUES (?1, 'backup_auto', 'backup', NULL, ?2, ?3)",
        params![actor_id, backup.path, backup.created_at],
    )
    .map_err(|error| error.to_string())?;
    Ok(Some(backup))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cash::{
        calculate_shift_cut, close_shift_cut_z_with_conn, daily_cut_summary_with_conn,
        open_cash_session_with_conn, redact_shift_cut_profit,
    };
    use crate::customers::customer_credit_adjust_with_conn;
    use crate::products::{get_product, import_products_with_conn};
    use crate::sales::{
        cancel_sale_with_conn, create_sale_with_conn, return_sale_item_with_conn,
        sale_detail_with_conn, validate_held_ticket_input,
    };
    use crate::security::{hash_pin, verify_pin};
    use rusqlite::{params, Connection};

    fn flow_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute(
            "INSERT INTO users (id, name, pin_hash, role, active, created_at)
             VALUES (?1, ?2, ?3, 'admin', 1, ?4)",
            params![1, "Admin", hash_pin("123456").unwrap(), now_iso()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO users (id, name, pin_hash, role, active, created_at)
             VALUES (?1, ?2, ?3, 'cashier', 1, ?4)",
            params![2, "Cajera", hash_pin("111111").unwrap(), now_iso()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO products
             (id, sku, barcode, name, category, unit, price, wholesale_price, cost, stock, min_stock, tax_rate, active, search_text, created_at, updated_at)
             VALUES (1, 'SKU-TEST', '750000000001', 'Producto test', 'Abarrotes', 'pieza', 20, NULL, 10, 5, 1, 0, 1, 'producto test', ?1, ?1)",
            params![now_iso()],
        )
        .unwrap();
        conn
    }

    #[test]
    fn monthly_sequence_is_max_plus_one() {
        assert_eq!(next_monthly_seq(0), 1);
        assert_eq!(next_monthly_seq(41), 42);
    }

    #[test]
    fn visible_folio_uses_period_and_three_digits() {
        assert_eq!(visible_monthly_folio("2026-06", 1), "2026-06-001");
        assert_eq!(visible_monthly_folio("2026-06", 128), "2026-06-128");
    }

    #[test]
    fn period_key_uses_calendar_month() {
        assert_eq!(period_key("2026-06-21T10:00:00Z").unwrap(), "2026-06");
    }

    #[test]
    fn average_ticket_handles_zero() {
        assert_eq!(average_ticket(300.0, 3), 100.0);
        assert_eq!(average_ticket(300.0, 0), 0.0);
    }

    #[test]
    fn argon2_pin_hash_verifies_and_rejects_wrong_pin() {
        let hash = hash_pin("493827").unwrap();
        assert!(hash.starts_with("$argon2"));
        assert!(verify_pin(&hash, "493827"));
        assert!(!verify_pin(&hash, "000000"));
    }

    #[test]
    fn legacy_pin_hash_still_verifies_for_migration() {
        let hash = legacy_hash_pin("1234");
        assert!(verify_pin(&hash, "1234"));
        assert!(!verify_pin(&hash, "1111"));
    }

    #[test]
    fn validation_accepts_secure_passwords_and_rejects_weak_ones() {
        assert!(validation::validate_pin("Abc12345", 4, "Contraseña").is_ok());
        assert!(validation::validate_pin("1234", 4, "Contraseña").is_ok());
        assert!(validation::validate_pin("abcd", 4, "Contraseña").is_ok());
        assert!(validation::validate_pin("Ab1", 4, "Contraseña").is_err());
    }

    #[test]
    fn validation_rejects_bad_rfc_and_email() {
        assert!(validation::validate_optional_rfc(Some("XAXX010101000")).is_ok());
        assert!(validation::validate_optional_rfc(Some("BAD")).is_err());
        assert!(validation::validate_optional_email(Some("cliente@example.com")).is_ok());
        assert!(validation::validate_optional_email(Some("cliente@local")).is_err());
    }

    #[test]
    fn line_amounts_handles_included_and_added_tax() {
        assert_eq!(
            line_amounts(116.0, 0.0, 0.16, true, true),
            (100.0, 16.0, 116.0)
        );
        assert_eq!(
            line_amounts(100.0, 0.0, 0.16, false, true),
            (100.0, 16.0, 116.0)
        );
        assert_eq!(
            line_amounts(50.0, 100.0, 0.16, false, true),
            (0.0, 0.0, 0.0)
        );
    }

    #[test]
    fn product_bulk_import_is_all_or_nothing() {
        let mut conn = flow_conn();
        let result = import_products_with_conn(
            &mut conn,
            vec![
                ProductImportRow {
                    row_number: 2,
                    sku: "SKU-NEW".into(),
                    barcode: "750000000002".into(),
                    name: "Nuevo".into(),
                    category: "Abarrotes".into(),
                    unit: "pieza".into(),
                    price: 12.0,
                    wholesale_price: None,
                    cost: 8.0,
                    stock: 3.0,
                    min_stock: 1.0,
                    tax_rate: 0.0,
                    tax_ids: vec![],
                    active: true,
                },
                ProductImportRow {
                    row_number: 3,
                    sku: "SKU-BAD".into(),
                    barcode: "750000000003".into(),
                    name: "Malo".into(),
                    category: "Abarrotes".into(),
                    unit: "pieza".into(),
                    price: -1.0,
                    wholesale_price: None,
                    cost: 8.0,
                    stock: 3.0,
                    min_stock: 1.0,
                    tax_rate: 0.0,
                    tax_ids: vec![],
                    active: true,
                },
            ],
        )
        .unwrap();
        assert!(!result.committed);
        assert_eq!(result.imported, 0);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM products WHERE sku = 'SKU-NEW'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn product_bulk_import_updates_and_reactivates_existing_product() {
        let mut conn = flow_conn();
        conn.execute(
            "UPDATE products SET active = 0, stock = 5, price = 20 WHERE id = 1",
            [],
        )
        .unwrap();

        let result = import_products_with_conn(
            &mut conn,
            vec![ProductImportRow {
                row_number: 2,
                sku: "IGNORED".into(),
                barcode: "750000000001".into(),
                name: "Producto actualizado".into(),
                category: "Bebidas".into(),
                unit: "pieza".into(),
                price: 25.0,
                wholesale_price: None,
                cost: 12.0,
                stock: 30.0,
                min_stock: 2.0,
                tax_rate: 0.0,
                tax_ids: vec![],
                active: false,
            }],
        )
        .unwrap();

        assert!(result.committed);
        assert_eq!(result.created, 0);
        assert_eq!(result.updated, 1);
        let product = get_product(&conn, 1).unwrap();
        assert_eq!(product.name, "Producto actualizado");
        assert_eq!(product.stock, 30.0);
        assert_eq!(product.price, 25.0);
        assert!(product.active);
    }

    #[test]
    fn mixed_payment_change_comes_from_cash_only() {
        let mut conn = flow_conn();
        let session = open_cash_session_with_conn(&conn, 2, 100.0).unwrap();

        let receipt = create_sale_with_conn(
            &mut conn,
            SaleDraft {
                cashier_id: 2,
                customer_id: None,
                items: vec![SaleItemInput {
                    product_id: 1,
                    quantity: 2.0,
                    unit_price: 20.0,
                    discount: 0.0,
                }],
                payments: vec![
                    PaymentInput {
                        method: "card".into(),
                        amount: 30.0,
                        reference: Some("Terminal 1".into()),
                    },
                    PaymentInput {
                        method: "cash".into(),
                        amount: 20.0,
                        reference: None,
                    },
                ],
                notes: None,
            },
        )
        .unwrap();
        assert_eq!(receipt.total, 40.0);
        assert_eq!(receipt.paid, 50.0);
        assert_eq!(receipt.change_due, 10.0);
        let expected_cash: f64 = conn
            .query_row(
                "SELECT expected_cash FROM cash_sessions WHERE id = ?1",
                params![session.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(expected_cash, 110.0);

        let err = create_sale_with_conn(
            &mut conn,
            SaleDraft {
                cashier_id: 2,
                customer_id: None,
                items: vec![SaleItemInput {
                    product_id: 1,
                    quantity: 1.0,
                    unit_price: 20.0,
                    discount: 0.0,
                }],
                payments: vec![PaymentInput {
                    method: "card".into(),
                    amount: 25.0,
                    reference: Some("Terminal 1".into()),
                }],
                notes: None,
            },
        )
        .unwrap_err();
        assert_eq!(err, "Tarjeta/credito excede total");
    }

    #[test]
    fn sale_cancel_and_shift_cut_restore_stock_and_cash() {
        let mut conn = flow_conn();
        let session = open_cash_session_with_conn(&conn, 2, 100.0).unwrap();
        let shift_id: i64 = conn
            .query_row(
                "SELECT id FROM shifts WHERE cash_session_id = ?1",
                params![session.id],
                |row| row.get(0),
            )
            .unwrap();

        let receipt = create_sale_with_conn(
            &mut conn,
            SaleDraft {
                cashier_id: 2,
                customer_id: None,
                items: vec![SaleItemInput {
                    product_id: 1,
                    quantity: 2.0,
                    unit_price: 20.0,
                    discount: 0.0,
                }],
                payments: vec![PaymentInput {
                    method: "cash".into(),
                    amount: 50.0,
                    reference: None,
                }],
                notes: None,
            },
        )
        .unwrap();
        assert_eq!(receipt.total, 40.0);
        let stock_after_sale: f64 = conn
            .query_row("SELECT stock FROM products WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(stock_after_sale, 3.0);
        let cash_after_sale: f64 = conn
            .query_row(
                "SELECT expected_cash FROM cash_sessions WHERE id = ?1",
                params![session.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(cash_after_sale, 140.0);

        cancel_sale_with_conn(&mut conn, receipt.sale_id, 1, "Error de captura".into()).unwrap();
        let stock_after_cancel: f64 = conn
            .query_row("SELECT stock FROM products WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(stock_after_cancel, 5.0);
        let cash_after_cancel: f64 = conn
            .query_row(
                "SELECT expected_cash FROM cash_sessions WHERE id = ?1",
                params![session.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(cash_after_cancel, 100.0);

        let second = create_sale_with_conn(
            &mut conn,
            SaleDraft {
                cashier_id: 2,
                customer_id: None,
                items: vec![SaleItemInput {
                    product_id: 1,
                    quantity: 1.0,
                    unit_price: 20.0,
                    discount: 0.0,
                }],
                payments: vec![PaymentInput {
                    method: "cash".into(),
                    amount: 20.0,
                    reference: None,
                }],
                notes: None,
            },
        )
        .unwrap();
        assert_eq!(second.total, 20.0);
        // Drawer physically holds 120: opening 100, +50 -10 change (sale 1),
        // -40 refund on cancel, +20 (sale 2). The cut must expect 120, not
        // double-subtract the canceled sale's cash.
        let snapshot =
            close_shift_cut_z_with_conn(&mut conn, shift_id, 120.0, 2, Some("[]".into()), None)
                .unwrap();
        assert_eq!(snapshot.status, "closed");
        assert_eq!(snapshot.total_tickets, 1);
        assert_eq!(snapshot.canceled_tickets, 1);
        assert_eq!(snapshot.expected_cash, 120.0);
        assert_eq!(snapshot.cash_difference, Some(0.0));

        let err = cancel_sale_with_conn(&mut conn, second.sale_id, 1, "Ya no quiero".into())
            .unwrap_err();
        assert_eq!(
            err,
            "Venta de corte cerrado: registra devolucion en el turno actual"
        );
    }

    #[test]
    fn partial_item_return_restores_stock_and_nets_shift_cut() {
        let mut conn = flow_conn();
        let session = open_cash_session_with_conn(&conn, 2, 100.0).unwrap();
        let shift_id: i64 = conn
            .query_row(
                "SELECT id FROM shifts WHERE cash_session_id = ?1",
                params![session.id],
                |row| row.get(0),
            )
            .unwrap();

        let receipt = create_sale_with_conn(
            &mut conn,
            SaleDraft {
                cashier_id: 2,
                customer_id: None,
                items: vec![SaleItemInput {
                    product_id: 1,
                    quantity: 2.0,
                    unit_price: 20.0,
                    discount: 0.0,
                }],
                payments: vec![PaymentInput {
                    method: "cash".into(),
                    amount: 40.0,
                    reference: None,
                }],
                notes: None,
            },
        )
        .unwrap();
        assert_eq!(receipt.total, 40.0);

        let sale_item_id: i64 = conn
            .query_row(
                "SELECT id FROM sale_items WHERE sale_id = ?1",
                params![receipt.sale_id],
                |row| row.get(0),
            )
            .unwrap();

        // Return 1 of the 2 units.
        return_sale_item_with_conn(&mut conn, 1, sale_item_id, 1.0, "Producto danado".into()).unwrap();

        let stock_after_return: f64 = conn
            .query_row("SELECT stock FROM products WHERE id = 1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(stock_after_return, 4.0);
        let cash_after_return: f64 = conn
            .query_row(
                "SELECT expected_cash FROM cash_sessions WHERE id = ?1",
                params![session.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(cash_after_return, 120.0);

        // Cannot return more than what is left on the line.
        let err = return_sale_item_with_conn(&mut conn, 1, sale_item_id, 2.0, "De mas".into())
            .unwrap_err();
        assert!(err.starts_with("Solo quedan"), "unexpected error: {err}");

        let snapshot =
            close_shift_cut_z_with_conn(&mut conn, shift_id, 120.0, 2, Some("[]".into()), None)
                .unwrap();
        assert_eq!(snapshot.total_tickets, 1);
        assert_eq!(snapshot.canceled_tickets, 0);
        assert_eq!(snapshot.net_sales, 20.0);
        assert_eq!(snapshot.gross_profit, 10.0);
        assert_eq!(snapshot.expected_cash, 120.0);
        assert_eq!(snapshot.cash_difference, Some(0.0));
        assert_eq!(snapshot.cash_refunds_total, 20.0);
        assert_eq!(snapshot.refunds.len(), 1);
        assert_eq!(snapshot.refunds[0].cash_amount, 20.0);
    }

    #[cfg(not(windows))]
    #[test]
    fn escpos_ticket_wraps_init_and_cut_per_copy() {
        let bytes = escpos_ticket_bytes("HOLA", 2);
        // Starts with ESC @ (init).
        assert_eq!(&bytes[0..2], &[0x1B, 0x40]);
        // Ends with GS V B 0 (feed + partial cut).
        assert_eq!(&bytes[bytes.len() - 4..], &[0x1D, 0x56, 0x42, 0x00]);
        // Two copies -> two init sequences and two cut sequences.
        assert_eq!(bytes.windows(2).filter(|w| *w == [0x1B, 0x40]).count(), 2);
        assert_eq!(bytes.windows(4).filter(|w| *w == [0x1D, 0x56, 0x42, 0x00]).count(), 2);
    }

    #[test]
    fn receipt_text_preserves_header_footer_indentation() {
        let conn = flow_conn();
        let now = now_iso();
        for (key, value) in [
            ("ticket_header", "   Centrado a mano"),
            ("ticket_footer", "  Vuelve pronto  "),
        ] {
            conn.execute(
                "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
                params![key, value, now],
            )
            .unwrap();
        }
        // sale_id <= 0 uses the same "Probar impresora" demo path the Settings
        // screen's test-print button calls, so this exercises exactly what a
        // real test print sends.
        let text = receipt_text(&conn, 0).unwrap();
        assert!(
            text.contains("   Centrado a mano"),
            "leading spaces the user typed in the header must survive to print: {text:?}"
        );
        assert!(
            text.contains("  Vuelve pronto  "),
            "leading/trailing spaces in the footer must survive to print: {text:?}"
        );
    }

    #[test]
    fn escpos_ticket_selects_cp850_codepage() {
        let bytes = escpos_ticket_bytes("x", 1);
        // ESC t 2 must appear right after the ESC @ reset, before any text.
        assert_eq!(&bytes[2..5], &[0x1B, 0x74, 0x02]);
    }

    #[test]
    fn encode_cp850_maps_common_spanish_accents() {
        // Real strings this ticket generator actually sends: header/footer,
        // "Codigo", product names. Wrong bytes here print as garbage symbols
        // on real thermal hardware instead of the intended letter.
        assert_eq!(encode_cp850("á"), vec![0xA0]);
        assert_eq!(encode_cp850("é"), vec![0x82]);
        assert_eq!(encode_cp850("í"), vec![0xA1]);
        assert_eq!(encode_cp850("ó"), vec![0xA2]);
        assert_eq!(encode_cp850("ú"), vec![0xA3]);
        assert_eq!(encode_cp850("ñ"), vec![0xA4]);
        assert_eq!(encode_cp850("Ñ"), vec![0xA5]);
        assert_eq!(encode_cp850("¿Cómo estás?"), b"\xA8C\xA2mo est\xA0s?".to_vec());
        assert_eq!(encode_cp850("Cajón"), b"Caj\xA2n".to_vec());
        assert_eq!(encode_cp850("Gracias por su compra"), b"Gracias por su compra".to_vec());
    }

    #[test]
    fn encode_cp850_falls_back_to_unaccented_letter_for_unmapped_chars() {
        // Worst case must be "readable without the accent", never a wrong
        // symbol (e.g. a box-drawing character or currency sign).
        assert_eq!(encode_cp850("à"), vec![b'a']);
        assert_eq!(encode_cp850("ê"), vec![b'e']);
    }

    #[test]
    fn cleanup_removes_only_old_unsold_hidden_products() {
        use crate::migrations::cleanup_orphan_quick_products;
        let mut conn = flow_conn();
        let _session = open_cash_session_with_conn(&conn, 2, 100.0).unwrap();
        let insert_hidden = |conn: &Connection, token: &str, created: &str| {
            conn.execute(
                "INSERT INTO products (sku, barcode, name, category, unit, price, cost, stock, min_stock, tax_rate, active, catalog_hidden, search_text, created_at, updated_at)
                 VALUES (?1, ?1, ?2, 'Venta rapida', 'pieza', 10, 0, 1000, 0, 0, 1, 1, '', ?3, ?3)",
                params![token, token, created],
            )
            .unwrap();
            conn.last_insert_rowid()
        };
        let old_unsold = insert_hidden(&conn, "OLD", "2020-01-01T00:00:00+00:00");
        let recent_unsold = insert_hidden(&conn, "NEW", &now_iso());
        let old_sold = insert_hidden(&conn, "SOLD", "2020-01-01T00:00:00+00:00");
        // Sell the old_sold one so it is not orphan.
        create_sale_with_conn(
            &mut conn,
            SaleDraft {
                cashier_id: 2,
                customer_id: None,
                items: vec![SaleItemInput { product_id: old_sold, quantity: 1.0, unit_price: 10.0, discount: 0.0 }],
                payments: vec![PaymentInput { method: "cash".into(), amount: 10.0, reference: None }],
                notes: None,
            },
        )
        .unwrap();

        cleanup_orphan_quick_products(&conn);

        let exists = |conn: &Connection, id: i64| -> bool {
            conn.query_row("SELECT COUNT(*) FROM products WHERE id = ?1", params![id], |row| row.get::<_, i64>(0)).unwrap() > 0
        };
        assert!(!exists(&conn, old_unsold), "old unsold hidden product should be removed");
        assert!(exists(&conn, recent_unsold), "recent hidden product should stay");
        assert!(exists(&conn, old_sold), "sold hidden product should stay");
        assert!(exists(&conn, 1), "normal catalog product must never be touched");
    }

    #[test]
    fn sale_detail_returns_items_payments_and_partial_return_status() {
        let mut conn = flow_conn();
        let _session = open_cash_session_with_conn(&conn, 2, 100.0).unwrap();
        let receipt = create_sale_with_conn(
            &mut conn,
            SaleDraft {
                cashier_id: 2,
                customer_id: None,
                items: vec![SaleItemInput { product_id: 1, quantity: 2.0, unit_price: 20.0, discount: 0.0 }],
                payments: vec![PaymentInput { method: "cash".into(), amount: 40.0, reference: None }],
                notes: None,
            },
        )
        .unwrap();
        let sale_item_id: i64 = conn
            .query_row("SELECT id FROM sale_items WHERE sale_id = ?1", params![receipt.sale_id], |row| row.get(0))
            .unwrap();
        return_sale_item_with_conn(&mut conn, 1, sale_item_id, 1.0, "Prueba".into()).unwrap();

        let detail = sale_detail_with_conn(&conn, receipt.sale_id).unwrap();
        assert_eq!(detail.folio, receipt.folio);
        assert_eq!(detail.status, "paid");
        assert_eq!(detail.cashier_name, "Cajera");
        assert_eq!(detail.items.len(), 1);
        assert_eq!(detail.items[0].quantity, 2.0);
        assert_eq!(detail.items[0].returned_quantity, 1.0);
        assert_eq!(detail.payments.len(), 1);
        assert_eq!(detail.payments[0].method, "cash");
        assert_eq!(detail.payments[0].amount, 40.0);
    }

    #[test]
    fn folio_is_global_consecutive_and_continues_after_legacy() {
        let mut conn = flow_conn();
        let _session = open_cash_session_with_conn(&conn, 2, 100.0).unwrap();
        // Simulate 3 legacy sales with the old monthly folio format.
        for seq in 1..=3 {
            conn.execute(
                "INSERT INTO sales (folio, monthly_seq, cashier_id, subtotal, tax, discount, total, paid, change_due, status, created_at)
                 VALUES (?1, ?2, 2, 10, 0, 0, 10, 10, 0, 'paid', ?3)",
                params![format!("2026-06-{seq:03}"), seq, now_iso()],
            )
            .unwrap();
        }
        let make_sale = |conn: &mut Connection| {
            create_sale_with_conn(
                conn,
                SaleDraft {
                    cashier_id: 2,
                    customer_id: None,
                    items: vec![SaleItemInput { product_id: 1, quantity: 1.0, unit_price: 20.0, discount: 0.0 }],
                    payments: vec![PaymentInput { method: "cash".into(), amount: 20.0, reference: None }],
                    notes: None,
                },
            )
            .unwrap()
        };
        // 3 legacy sales exist -> next visible folio continues at 4, then 5.
        let first = make_sale(&mut conn);
        assert_eq!(first.folio, "4");
        let second = make_sale(&mut conn);
        assert_eq!(second.folio, "5");
    }

    #[test]
    fn sale_total_rounds_up_to_whole_pesos() {
        let mut conn = flow_conn();
        let _session = open_cash_session_with_conn(&conn, 2, 100.0).unwrap();
        let receipt = create_sale_with_conn(
            &mut conn,
            SaleDraft {
                cashier_id: 2,
                customer_id: None,
                items: vec![SaleItemInput { product_id: 1, quantity: 1.0, unit_price: 58.90, discount: 0.0 }],
                payments: vec![PaymentInput { method: "cash".into(), amount: 59.0, reference: None }],
                notes: None,
            },
        )
        .unwrap();
        // 58.90 charged as 59; no change; 0.10 recorded as rounding.
        assert_eq!(receipt.total, 59.0);
        let (total, rounding, change): (f64, f64, f64) = conn
            .query_row(
                "SELECT total, rounding, change_due FROM sales WHERE id = ?1",
                params![receipt.sale_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(total, 59.0);
        assert_eq!(rounding, 0.10);
        assert_eq!(change, 0.0);
    }

    #[test]
    fn card_sale_return_does_not_touch_expected_cash() {
        let mut conn = flow_conn();
        let session = open_cash_session_with_conn(&conn, 2, 100.0).unwrap();
        let sale = create_sale_with_conn(
            &mut conn,
            SaleDraft {
                cashier_id: 2,
                customer_id: None,
                items: vec![SaleItemInput { product_id: 1, quantity: 1.0, unit_price: 20.0, discount: 0.0 }],
                payments: vec![PaymentInput { method: "card".into(), amount: 20.0, reference: Some("TERM-1".into()) }],
                notes: None,
            },
        )
        .unwrap();
        // Card sale adds no cash to the drawer.
        let cash_after_sale: f64 = conn
            .query_row("SELECT expected_cash FROM cash_sessions WHERE id = ?1", params![session.id], |row| row.get(0))
            .unwrap();
        assert_eq!(cash_after_sale, 100.0);

        let sale_item_id: i64 = conn
            .query_row("SELECT id FROM sale_items WHERE sale_id = ?1", params![sale.sale_id], |row| row.get(0))
            .unwrap();
        return_sale_item_with_conn(&mut conn, 1, sale_item_id, 1.0, "Devolucion tarjeta".into()).unwrap();

        // Refund goes back to the card: no cash leaves the drawer.
        let cash_refund: f64 = conn
            .query_row("SELECT cash_refund FROM sale_returns WHERE sale_item_id = ?1", params![sale_item_id], |row| row.get(0))
            .unwrap();
        assert_eq!(cash_refund, 0.0);
        let cash_after_return: f64 = conn
            .query_row("SELECT expected_cash FROM cash_sessions WHERE id = ?1", params![session.id], |row| row.get(0))
            .unwrap();
        assert_eq!(cash_after_return, 100.0);
    }

    #[test]
    fn card_sale_receipt_adds_credit_voucher_with_terminal() {
        let mut conn = flow_conn();
        let _session = open_cash_session_with_conn(&conn, 2, 100.0).unwrap();
        let card = create_sale_with_conn(
            &mut conn,
            SaleDraft {
                cashier_id: 2,
                customer_id: None,
                items: vec![SaleItemInput { product_id: 1, quantity: 1.0, unit_price: 20.0, discount: 0.0 }],
                payments: vec![PaymentInput { method: "card".into(), amount: 20.0, reference: Some("BANORTE-1".into()) }],
                notes: None,
            },
        )
        .unwrap();
        let cash = create_sale_with_conn(
            &mut conn,
            SaleDraft {
                cashier_id: 2,
                customer_id: None,
                items: vec![SaleItemInput { product_id: 1, quantity: 1.0, unit_price: 20.0, discount: 0.0 }],
                payments: vec![PaymentInput { method: "cash".into(), amount: 20.0, reference: None }],
                notes: None,
            },
        )
        .unwrap();

        let card_ticket = receipt_text(&conn, card.sale_id).unwrap();
        assert!(card_ticket.contains("VENTA A CREDITO"), "{card_ticket}");
        assert!(card_ticket.contains("FIRMA DEL CLIENTE"));
        assert!(card_ticket.contains("TERMINAL BANORTE-1"));
        // Centered on the default 32-col width: (32-15)/2 = 8 leading spaces.
        assert!(card_ticket.contains("        VENTA A CREDITO"), "{card_ticket}");
        // Exactly one blank line separates it from what's above (products/totals).
        assert!(card_ticket.contains("\n\n----"), "expected one blank line before the separator: {card_ticket}");

        // Cash sale must NOT get the credit voucher.
        let cash_ticket = receipt_text(&conn, cash.sale_id).unwrap();
        assert!(!cash_ticket.contains("VENTA A CREDITO"), "{cash_ticket}");
    }

    #[test]
    fn center_line_pads_short_lines_and_leaves_wide_lines_alone() {
        assert_eq!(center_line("VENTA A CREDITO", 32), format!("{}VENTA A CREDITO", " ".repeat(8)));
        assert_eq!(center_line("this-line-is-already-32-chars!!", 32), "this-line-is-already-32-chars!!");
        assert_eq!(center_line("this line is longer than the paper width", 32), "this line is longer than the paper width");
    }

    #[test]
    fn inventory_adjustment_carries_actor_name_for_reports() {
        let conn = flow_conn();
        conn.execute(
            "INSERT INTO inventory_movements (product_id, movement_type, quantity, reason, reference_id, actor_id, created_at)
             VALUES (1, 'adjustment', 5.0, 'Merma', NULL, 2, ?1)",
            params![now_iso()],
        )
        .unwrap();
        let actor_name: String = conn
            .query_row(
                "SELECT u.name FROM inventory_movements im JOIN users u ON u.id = im.actor_id WHERE im.product_id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(actor_name, "Cajera");
    }

    #[test]
    fn customer_credit_cash_payment_increases_running_expected_cash() {
        let conn = flow_conn();
        conn.execute(
            "INSERT INTO customers (id, name, rfc, phone, email, credit_limit, balance, created_at)
             VALUES (1, 'Cliente Uno', NULL, NULL, NULL, 500, 100, ?1)",
            params![now_iso()],
        )
        .unwrap();
        let session = open_cash_session_with_conn(&conn, 2, 100.0).unwrap();
        let shift_id: i64 = conn
            .query_row(
                "SELECT id FROM shifts WHERE cash_session_id = ?1",
                params![session.id],
                |row| row.get(0),
            )
            .unwrap();

        // Customer pays 15 in cash toward their balance: the drawer gains that
        // cash, so the running expected_cash used by Arqueo/close-shift must move too.
        customer_credit_adjust_with_conn(
            &conn,
            1,
            CustomerCreditInput {
                customer_id: 1,
                amount: -15.0,
                reason: "Abono".into(),
                payment_method: Some("cash".into()),
            },
        )
        .unwrap();

        let session_expected: f64 = conn
            .query_row(
                "SELECT expected_cash FROM cash_sessions WHERE id = ?1",
                params![session.id],
                |row| row.get(0),
            )
            .unwrap();
        let shift_expected: f64 = conn
            .query_row(
                "SELECT expected_cash FROM shifts WHERE id = ?1",
                params![shift_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(session_expected, 115.0);
        assert_eq!(shift_expected, 115.0);

        // A card abono must NOT touch the cash drawer total.
        customer_credit_adjust_with_conn(
            &conn,
            1,
            CustomerCreditInput {
                customer_id: 1,
                amount: -10.0,
                reason: "Abono tarjeta".into(),
                payment_method: Some("card".into()),
            },
        )
        .unwrap();
        let session_expected_after_card: f64 = conn
            .query_row(
                "SELECT expected_cash FROM cash_sessions WHERE id = ?1",
                params![session.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(session_expected_after_card, 115.0);
    }

    #[test]
    fn shift_cut_profit_redaction_hides_margin_without_view_profit_permission() {
        let mut conn = flow_conn();
        conn.execute(
            "INSERT INTO customers (id, name, rfc, phone, email, credit_limit, balance, created_at)
             VALUES (1, 'Cliente Uno', NULL, NULL, NULL, 500, 0, ?1)",
            params![now_iso()],
        )
        .unwrap();
        let session = open_cash_session_with_conn(&conn, 2, 100.0).unwrap();
        let shift_id: i64 = conn
            .query_row(
                "SELECT id FROM shifts WHERE cash_session_id = ?1",
                params![session.id],
                |row| row.get(0),
            )
            .unwrap();
        create_sale_with_conn(
            &mut conn,
            SaleDraft {
                cashier_id: 2,
                customer_id: Some(1),
                items: vec![SaleItemInput {
                    product_id: 1,
                    quantity: 2.0,
                    unit_price: 20.0,
                    discount: 0.0,
                }],
                payments: vec![PaymentInput {
                    method: "cash".into(),
                    amount: 40.0,
                    reference: None,
                }],
                notes: None,
            },
        )
        .unwrap();

        let snapshot = calculate_shift_cut(&conn, shift_id).unwrap();
        assert_eq!(snapshot.gross_profit, 20.0);
        assert!(!snapshot.departments.is_empty());
        assert!(snapshot.departments.iter().any(|department| department.gross_profit > 0.0));
        assert!(!snapshot.top_customers_by_profit.is_empty());

        // Cashier (user 2) has no view_profit permission; admin (user 1) always does.
        assert!(!has_permission(&conn, 2, "view_profit").unwrap());
        assert!(has_permission(&conn, 1, "view_profit").unwrap());

        let mut redacted = snapshot;
        redact_shift_cut_profit(&mut redacted);
        assert_eq!(redacted.gross_profit, 0.0);
        assert!(redacted.departments.iter().all(|department| department.gross_profit == 0.0));
        assert!(redacted.top_customers_by_profit.is_empty());
        assert!(redacted.top_customers_by_sales.iter().all(|customer| customer.gross_profit == 0.0));
        // Sales figures (not profit) must survive redaction untouched.
        assert!(redacted.departments.iter().any(|department| department.total_sales > 0.0));
    }

    #[test]
    fn shift_cut_expected_cash_uses_sales_movements_refunds_and_credit_payments() {
        let mut conn = flow_conn();
        conn.execute(
            "INSERT INTO customers (id, name, rfc, phone, email, credit_limit, balance, created_at)
             VALUES (1, 'Cliente Uno', NULL, NULL, NULL, 500, 0, ?1)",
            params![now_iso()],
        )
        .unwrap();
        let session = open_cash_session_with_conn(&conn, 2, 100.0).unwrap();
        let shift_id: i64 = conn
            .query_row(
                "SELECT id FROM shifts WHERE cash_session_id = ?1",
                params![session.id],
                |row| row.get(0),
            )
            .unwrap();

        create_sale_with_conn(
            &mut conn,
            SaleDraft {
                cashier_id: 2,
                customer_id: Some(1),
                items: vec![SaleItemInput {
                    product_id: 1,
                    quantity: 2.0,
                    unit_price: 20.0,
                    discount: 0.0,
                }],
                payments: vec![PaymentInput {
                    method: "cash".into(),
                    amount: 40.0,
                    reference: None,
                }],
                notes: None,
            },
        )
        .unwrap();

        let sale_to_cancel = create_sale_with_conn(
            &mut conn,
            SaleDraft {
                cashier_id: 2,
                customer_id: Some(1),
                items: vec![SaleItemInput {
                    product_id: 1,
                    quantity: 1.0,
                    unit_price: 20.0,
                    discount: 0.0,
                }],
                payments: vec![PaymentInput {
                    method: "cash".into(),
                    amount: 20.0,
                    reference: None,
                }],
                notes: None,
            },
        )
        .unwrap();

        conn.execute(
            "INSERT INTO cash_movements (session_id, movement_type, amount, reason, actor_id, created_at)
             VALUES (?1, 'in', 10, 'Recarga de caja', 2, ?2)",
            params![session.id, now_iso()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO cash_movements (session_id, movement_type, amount, reason, actor_id, created_at)
             VALUES (?1, 'out', 5, 'Compra menor', 2, ?2)",
            params![session.id, now_iso()],
        )
        .unwrap();

        conn.execute(
            "UPDATE customers SET balance = balance + 30 WHERE id = 1",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO customer_credit_movements
             (customer_id, amount, reason, created_at, movement_kind, payment_method, actor_id, cash_session_id)
             VALUES (1, 30, 'Cargo', ?1, 'charge', NULL, 1, NULL)",
            params![now_iso()],
        )
        .unwrap();
        conn.execute(
            "UPDATE customers SET balance = balance - 15 WHERE id = 1",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO customer_credit_movements
             (customer_id, amount, reason, created_at, movement_kind, payment_method, actor_id, cash_session_id)
             VALUES (1, -15, 'Abono parcial', ?1, 'payment', 'cash', 1, ?2)",
            params![now_iso(), session.id],
        )
        .unwrap();

        cancel_sale_with_conn(&mut conn, sale_to_cancel.sale_id, 1, "Devolucion".into()).unwrap();
        let snapshot = calculate_shift_cut(&conn, shift_id).unwrap();
        assert_eq!(snapshot.total_tickets, 1);
        assert_eq!(snapshot.canceled_tickets, 1);
        assert_eq!(snapshot.cash_paid, 40.0);
        assert_eq!(snapshot.cash_entries_total, 10.0);
        assert_eq!(snapshot.cash_out_total, 5.0);
        assert_eq!(snapshot.cash_refunds_total, 20.0);
        assert_eq!(snapshot.credit_payments_total, 15.0);
        // Drawer really holds 160: opening 100 + 40 + 20 sales cash - 20 refund
        // + 10 in - 5 out + 15 abono. The canceled sale nets to zero, it is not
        // subtracted twice.
        assert_eq!(snapshot.expected_cash, 160.0);
        assert_eq!(snapshot.refunds.len(), 1);
        assert_eq!(snapshot.credit_payments.len(), 1);
        assert_eq!(snapshot.payment_breakdown.iter().find(|payment| payment.method == "cash").map(|payment| payment.amount), Some(40.0));
    }

    #[test]
    fn daily_cut_summary_aggregates_closed_shifts() {
        let mut conn = flow_conn();
        let first_session = open_cash_session_with_conn(&conn, 2, 100.0).unwrap();
        let first_shift_id: i64 = conn
            .query_row(
                "SELECT id FROM shifts WHERE cash_session_id = ?1",
                params![first_session.id],
                |row| row.get(0),
            )
            .unwrap();
        create_sale_with_conn(
            &mut conn,
            SaleDraft {
                cashier_id: 2,
                customer_id: None,
                items: vec![SaleItemInput {
                    product_id: 1,
                    quantity: 1.0,
                    unit_price: 20.0,
                    discount: 0.0,
                }],
                payments: vec![PaymentInput {
                    method: "cash".into(),
                    amount: 20.0,
                    reference: None,
                }],
                notes: None,
            },
        )
        .unwrap();
        let first_cut =
            close_shift_cut_z_with_conn(&mut conn, first_shift_id, 120.0, 2, Some("[]".into()), None)
                .unwrap();

        let second_session = open_cash_session_with_conn(&conn, 2, 50.0).unwrap();
        let second_shift_id: i64 = conn
            .query_row(
                "SELECT id FROM shifts WHERE cash_session_id = ?1",
                params![second_session.id],
                |row| row.get(0),
            )
            .unwrap();
        create_sale_with_conn(
            &mut conn,
            SaleDraft {
                cashier_id: 2,
                customer_id: None,
                items: vec![SaleItemInput {
                    product_id: 1,
                    quantity: 1.0,
                    unit_price: 20.0,
                    discount: 0.0,
                }],
                payments: vec![PaymentInput {
                    method: "card".into(),
                    amount: 20.0,
                    reference: Some("Terminal".into()),
                }],
                notes: None,
            },
        )
        .unwrap();
        close_shift_cut_z_with_conn(&mut conn, second_shift_id, 50.0, 2, Some("[]".into()), None)
            .unwrap();

        // El corte diario filtra por dia LOCAL (date(closed_at,'localtime')).
        // Derivar la fecha esperada igual, no del slice UTC, para no fallar cerca de medianoche UTC.
        let date = DateTime::parse_from_rfc3339(first_cut.closed_at.as_deref().unwrap())
            .unwrap()
            .with_timezone(&Local)
            .format("%Y-%m-%d")
            .to_string();
        let summary = daily_cut_summary_with_conn(&conn, Some(date)).unwrap();
        assert_eq!(summary.cut_count, 2);
        assert_eq!(summary.total_tickets, 2);
        assert_eq!(summary.net_sales, 40.0);
        assert_eq!(summary.cash_paid, 20.0);
        assert_eq!(summary.card_paid, 20.0);
        assert_eq!(summary.expected_cash, 170.0);
        assert_eq!(summary.payment_breakdown.iter().find(|payment| payment.method == "cash").map(|payment| payment.amount), Some(20.0));
        assert_eq!(summary.payment_breakdown.iter().find(|payment| payment.method == "card").map(|payment| payment.amount), Some(20.0));
        assert_eq!(summary.cuts.len(), 2);
    }

    #[test]
    fn held_ticket_validation_reports_bad_rows_before_save() {
        let invalid = HeldTicketInput {
            id: None,
            name: "A".into(),
            cashier_id: 2,
            items: vec![HeldTicketItem {
                product_id: 1,
                quantity: 1.0,
                unit_price: 20.0,
                discount: 0.0,
                tax_rate: 0.0,
            }],
        };
        assert_eq!(
            validate_held_ticket_input(&invalid, true, true).unwrap_err(),
            "Nombre de ticket muy corto"
        );

        let valid = HeldTicketInput {
            id: None,
            name: "Cliente mostrador".into(),
            cashier_id: 2,
            items: vec![HeldTicketItem {
                product_id: 1,
                quantity: 2.0,
                unit_price: 20.0,
                discount: 5.0,
                tax_rate: 0.0,
            }],
        };
        let (_, item_count, total) = validate_held_ticket_input(&valid, true, true).unwrap();
        assert_eq!(item_count, 1);
        assert_eq!(total, 35.0);
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let (conn, db_path) = init_db(&app.handle())
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
            app.manage(AppState {
                db: Mutex::new(conn),
                db_path,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crate::products::product_search,
            crate::products::quick_product_create,
            crate::products::product_get_many,
            crate::products::product_upsert,
            crate::products::product_bulk_validate,
            crate::products::product_bulk_import,
            crate::products::product_delete,
            crate::products::inventory_adjust,
            crate::products::inventory_kardex,
            crate::users::auth_needs_setup,
            crate::users::auth_create_initial_admin,
            crate::users::auth_login,
            crate::users::user_list,
            crate::users::cashier_options,
            crate::users::user_create,
            crate::users::user_update,
            crate::users::user_delete,
            crate::customers::customer_list,
            crate::customers::customer_upsert,
            crate::customers::customer_credit_adjust,
            crate::purchases::supplier_list,
            crate::purchases::supplier_upsert,
            crate::purchases::supplier_delete,
            crate::invoices::tax_list,
            crate::invoices::invoice_prepare,
            crate::invoices::invoice_list,
            crate::sales::held_ticket_list,
            crate::sales::held_ticket_save,
            crate::sales::held_ticket_delete,
            crate::sales::active_sale_draft_get,
            crate::sales::active_sale_draft_save,
            crate::sales::active_sale_draft_clear,
            crate::sales::sale_create,
            crate::sales::sale_list,
            crate::sales::sale_line_history,
            crate::sales::sale_detail,
            crate::sales::sale_item_return,
            crate::sales::sale_cancel,
            crate::cash::cash_session_open,
            crate::cash::cash_session_close,
            crate::cash::shift_cut_x,
            crate::cash::shift_cut_z,
            crate::cash::shift_cut_history,
            crate::cash::print_shift_cut,
            crate::cash::daily_cut_summary,
            crate::cash::print_daily_cut,
            crate::cash::cash_movement_create,
            crate::cash::cash_movement_list,
            crate::cash::cash_count_create,
            crate::cash::cash_count_list,
            dashboard_summary,
            app_bootstrap,
            app_recovery_mark_clean,
            crate::reports::report_summary,
            crate::reports::report_product_sales,
            crate::reports::report_unsold_products,
            crate::reports::report_tax_breakdown,
            crate::reports::report_movement_history,
            crate::reports::monthly_sales_report,
            crate::cash::period_lock,
            crate::cash::audit_log_list,
            hardware_device_list,
            print_ticket,
            open_cash_drawer,
            read_scale,
            settings_get,
            settings_get_many,
            settings_set,
            settings_set_many,
            backup_create,
            backup_export_desktop,
            export_file,
            backup_list,
            backup_restore,
            backup_auto_if_due
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
