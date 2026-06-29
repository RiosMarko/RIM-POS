use crate::auth::{ensure_admin_remains, require_active_user, require_admin, UserSession};
use crate::backup::{backup_create_with_conn, BackupResult};
use crate::core::{
    average_ticket, next_monthly_seq, now_iso, period_key, round_money, should_run_auto_backup,
    visible_monthly_folio,
};
#[cfg(test)]
use crate::security::legacy_hash_pin;
use crate::security::{hash_pin, verify_pin};
use crate::settings_access::{is_invoice_setting_key, is_public_setting_key};
#[cfg(test)]
use crate::validation;
use crate::validation::{
    validate_non_negative, validate_optional_email, validate_optional_rfc, validate_pin,
    validate_positive, validate_required_text,
};
use chrono::Utc;
use rusqlite::{params, types::Type, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

struct AppState {
    db: Mutex<Connection>,
    db_path: PathBuf,
}

type CommandResult<T> = Result<T, String>;

const AUTO_BACKUP_LAST_SETTING: &str = "auto_backup_last_at";

#[derive(Debug, Serialize)]
struct Product {
    id: i64,
    sku: String,
    barcode: String,
    name: String,
    category: String,
    unit: String,
    price: f64,
    cost: f64,
    stock: f64,
    min_stock: f64,
    tax_rate: f64,
    tax_ids: Vec<i64>,
    active: bool,
}

#[derive(Debug, Deserialize)]
struct ProductInput {
    id: Option<i64>,
    sku: String,
    barcode: String,
    name: String,
    category: String,
    unit: String,
    price: f64,
    cost: f64,
    stock: f64,
    min_stock: f64,
    tax_rate: f64,
    #[serde(default)]
    tax_ids: Vec<i64>,
    active: bool,
}

#[derive(Debug, Deserialize)]
struct InventoryAdjustmentInput {
    product_id: i64,
    quantity: f64,
    reason: String,
}

#[derive(Debug, Serialize)]
struct InventoryMovement {
    id: i64,
    product_id: i64,
    product_name: String,
    movement_type: String,
    quantity: f64,
    reason: String,
    reference_id: Option<i64>,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct SaleItemInput {
    product_id: i64,
    quantity: f64,
    unit_price: f64,
    discount: f64,
}

#[derive(Debug, Deserialize)]
struct PaymentInput {
    method: String,
    amount: f64,
    reference: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SaleDraft {
    cashier_id: i64,
    customer_id: Option<i64>,
    items: Vec<SaleItemInput>,
    payments: Vec<PaymentInput>,
    notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HeldTicketItem {
    product_id: i64,
    quantity: f64,
    unit_price: f64,
    discount: f64,
    #[serde(default)]
    tax_rate: f64,
}

#[derive(Debug, Deserialize)]
struct HeldTicketInput {
    id: Option<i64>,
    name: String,
    cashier_id: i64,
    items: Vec<HeldTicketItem>,
}

#[derive(Debug, Serialize)]
struct HeldTicket {
    id: i64,
    name: String,
    cashier_id: i64,
    cashier_name: String,
    item_count: i64,
    total: f64,
    items: Vec<HeldTicketItem>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct ActiveSaleDraftInput {
    cashier_id: i64,
    cash_session_id: Option<i64>,
    items: Vec<HeldTicketItem>,
    cash_received: f64,
    card_received: f64,
    transfer_received: f64,
}

#[derive(Debug, Serialize)]
struct ActiveSaleDraft {
    cashier_id: i64,
    cash_session_id: Option<i64>,
    item_count: i64,
    total: f64,
    cash_received: f64,
    card_received: f64,
    transfer_received: f64,
    items: Vec<HeldTicketItem>,
    updated_at: String,
}

#[derive(Debug, Serialize)]
struct SaleReceipt {
    sale_id: i64,
    folio: String,
    subtotal: f64,
    tax: f64,
    discount: f64,
    total: f64,
    paid: f64,
    change_due: f64,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct ShiftCutSnapshot {
    shift_id: i64,
    cash_session_id: i64,
    status: String,
    opened_at: String,
    closed_at: Option<String>,
    total_tickets: i64,
    canceled_tickets: i64,
    gross_sales: f64,
    net_sales: f64,
    tax: f64,
    discount: f64,
    cash_paid: f64,
    card_paid: f64,
    transfer_paid: f64,
    average_ticket: f64,
    opening_cash: f64,
    expected_cash: f64,
    closing_cash: Option<f64>,
}

#[derive(Debug, Serialize)]
struct MonthlySalesReport {
    month: String,
    total_tickets: i64,
    total_amount: f64,
    average_ticket: f64,
    canceled_tickets: i64,
}

#[derive(Debug, Serialize)]
struct SaleListItem {
    id: i64,
    folio: String,
    cashier_name: String,
    total: f64,
    paid: f64,
    cash_paid: f64,
    card_paid: f64,
    transfer_paid: f64,
    status: String,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct CashSession {
    id: i64,
    opened_by: i64,
    opened_at: String,
    closed_at: Option<String>,
    opening_cash: f64,
    closing_cash: Option<f64>,
    expected_cash: f64,
    sales_total: f64,
    status: String,
}

#[derive(Debug, Deserialize)]
struct CashMovementInput {
    session_id: i64,
    movement_type: String,
    amount: f64,
    reason: String,
    actor_id: i64,
}

#[derive(Debug, Serialize)]
struct CashMovement {
    id: i64,
    session_id: i64,
    movement_type: String,
    amount: f64,
    reason: String,
    actor_name: String,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct HardwareResult {
    ok: bool,
    message: String,
}

#[derive(Debug, Serialize)]
struct HardwareDevice {
    id: String,
    name: String,
    device_type: String,
    connection: String,
    detail: String,
    is_default: bool,
}

#[derive(Debug, Serialize)]
struct ScaleReading {
    ok: bool,
    weight: f64,
    unit: String,
    source: String,
}

#[derive(Debug, Serialize)]
struct DashboardSummary {
    active_products: i64,
    low_stock_products: i64,
    today_sales: f64,
    today_tickets: i64,
    open_cash_session: Option<CashSession>,
}

#[derive(Debug, Serialize)]
struct ReportSummary {
    today_sales: f64,
    today_tickets: i64,
    average_ticket: f64,
    gross_profit: f64,
    cash_expected: f64,
    cash_sales: f64,
    card_sales: f64,
    transfer_sales: f64,
    low_stock_products: i64,
}

#[derive(Debug, Serialize)]
struct ProductSalesReport {
    product_id: i64,
    product_name: String,
    quantity: f64,
    total: f64,
}

#[derive(Debug, Serialize)]
struct ReportMovement {
    id: String,
    kind: String,
    title: String,
    detail: String,
    amount: f64,
    cash_paid: f64,
    card_paid: f64,
    transfer_paid: f64,
    actor_name: Option<String>,
    cash_session_id: Option<i64>,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct UserAccount {
    id: i64,
    name: String,
    role: String,
    active: bool,
    created_at: String,
    permissions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LoginInput {
    name: String,
    pin: String,
}

#[derive(Debug, Deserialize)]
struct InitialAdminInput {
    name: String,
    pin: String,
}

#[derive(Debug, Deserialize)]
struct UserCreateInput {
    name: String,
    pin: String,
    role: String,
    active: bool,
    permissions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct UserUpdateInput {
    id: i64,
    name: String,
    pin: Option<String>,
    role: String,
    active: bool,
    permissions: Vec<String>,
}

const USER_PERMISSION_KEYS: &[&str] = &[
    "products",
    "inventory",
    "customers",
    "reports",
    "purchases",
    "invoices",
];

#[derive(Debug, Deserialize)]
struct CustomerInput {
    id: Option<i64>,
    name: String,
    rfc: Option<String>,
    phone: Option<String>,
    email: Option<String>,
    credit_limit: f64,
}

#[derive(Debug, Deserialize)]
struct CustomerCreditInput {
    customer_id: i64,
    amount: f64,
    reason: String,
}

#[derive(Debug, Serialize)]
struct Customer {
    id: i64,
    name: String,
    rfc: Option<String>,
    phone: Option<String>,
    email: Option<String>,
    credit_limit: f64,
    balance: f64,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct Supplier {
    id: i64,
    name: String,
    phone: Option<String>,
    contact: Option<String>,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct SupplierInput {
    id: Option<i64>,
    name: String,
    phone: Option<String>,
    contact: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PurchaseInput {
    supplier_id: Option<i64>,
    product_id: i64,
    quantity: f64,
    unit_cost: f64,
    user_id: i64,
    note: Option<String>,
}

#[derive(Debug, Serialize)]
struct PurchaseReceipt {
    id: i64,
    supplier_name: Option<String>,
    product_name: String,
    quantity: f64,
    unit_cost: f64,
    total: f64,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct TaxOption {
    id: i64,
    name: String,
    #[serde(rename = "type")]
    tax_type: String,
    rate: f64,
    country: String,
    is_active: bool,
}

#[derive(Debug, Serialize)]
struct InvoiceDraft {
    id: i64,
    sale_id: Option<i64>,
    customer_id: Option<i64>,
    customer_name: Option<String>,
    folio: String,
    status: String,
    total: f64,
    pac_message: String,
    created_at: String,
}

fn setting_string(conn: &Connection, key: &str) -> CommandResult<Option<String>> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| error.to_string())
}

fn setting_bool(conn: &Connection, key: &str, default: bool) -> CommandResult<bool> {
    Ok(setting_string(conn, key)?
        .map(|value| value != "false")
        .unwrap_or(default))
}

fn line_amounts(
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

fn map_product(row: &rusqlite::Row<'_>) -> rusqlite::Result<Product> {
    Ok(Product {
        id: row.get(0)?,
        sku: row.get(1)?,
        barcode: row.get(2)?,
        name: row.get(3)?,
        category: row.get(4)?,
        unit: row.get(5)?,
        price: row.get(6)?,
        cost: row.get(7)?,
        stock: row.get(8)?,
        min_stock: row.get(9)?,
        tax_rate: row.get(10)?,
        tax_ids: Vec::new(),
        active: row.get::<_, i64>(11)? == 1,
    })
}

fn product_tax_ids(conn: &Connection, product_id: i64) -> CommandResult<Vec<i64>> {
    let mut stmt = conn
        .prepare("SELECT tax_id FROM product_taxes WHERE product_id = ?1 ORDER BY tax_id")
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![product_id], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn hydrate_product_taxes(conn: &Connection, product: &mut Product) -> CommandResult<()> {
    product.tax_ids = product_tax_ids(conn, product.id)?;
    Ok(())
}

fn tax_rate_for_ids(conn: &Connection, tax_ids: &[i64], fallback: f64) -> CommandResult<f64> {
    if tax_ids.is_empty() {
        return Ok(fallback.max(0.0));
    }
    let mut total = 0.0;
    for tax_id in tax_ids {
        let rate: f64 = conn
            .query_row(
                "SELECT rate FROM taxes WHERE id = ?1 AND is_active = 1",
                params![tax_id],
                |row| row.get(0),
            )
            .map_err(|_| format!("Impuesto no disponible: {tax_id}"))?;
        total += rate;
    }
    Ok(total)
}

fn save_product_taxes(conn: &Connection, product_id: i64, tax_ids: &[i64]) -> CommandResult<()> {
    conn.execute(
        "DELETE FROM product_taxes WHERE product_id = ?1",
        params![product_id],
    )
    .map_err(|error| error.to_string())?;
    let mut seen = HashSet::new();
    for tax_id in tax_ids {
        if seen.insert(*tax_id) {
            conn.execute(
                "INSERT INTO product_taxes (product_id, tax_id) VALUES (?1, ?2)",
                params![product_id, tax_id],
            )
            .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn init_db(app: &AppHandle) -> CommandResult<(Connection, PathBuf)> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("No se pudo localizar app data: {error}"))?;
    fs::create_dir_all(&data_dir).map_err(|error| format!("No se pudo crear app data: {error}"))?;
    let db_path = data_dir.join("pos-abarrotes.sqlite3");
    let conn = Connection::open(db_path).map_err(|error| error.to_string())?;
    migrate(&conn)?;
    seed_demo(&conn)?;
    let db_path = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("No se pudo localizar app data: {error}"))?
        .join("pos-abarrotes.sqlite3");
    Ok((conn, db_path))
}

fn migrate(conn: &Connection) -> CommandResult<()> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;

        CREATE TABLE IF NOT EXISTS users (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          name TEXT NOT NULL,
          pin_hash TEXT,
          role TEXT NOT NULL DEFAULT 'cashier',
          active INTEGER NOT NULL DEFAULT 1,
          created_at TEXT NOT NULL
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_users_name_unique ON users(lower(name));

        CREATE TABLE IF NOT EXISTS user_permissions (
          user_id INTEGER NOT NULL,
          permission_key TEXT NOT NULL,
          PRIMARY KEY(user_id, permission_key),
          FOREIGN KEY(user_id) REFERENCES users(id)
        );

        CREATE INDEX IF NOT EXISTS idx_user_permissions_permission_key
          ON user_permissions(permission_key);

        CREATE TABLE IF NOT EXISTS categories (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          name TEXT NOT NULL UNIQUE
        );

        CREATE TABLE IF NOT EXISTS products (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          sku TEXT NOT NULL UNIQUE,
          barcode TEXT NOT NULL UNIQUE,
          name TEXT NOT NULL,
          category TEXT NOT NULL,
          unit TEXT NOT NULL DEFAULT 'pieza',
          price REAL NOT NULL,
          cost REAL NOT NULL DEFAULT 0,
          stock REAL NOT NULL DEFAULT 0,
          min_stock REAL NOT NULL DEFAULT 0,
          tax_rate REAL NOT NULL DEFAULT 0,
          active INTEGER NOT NULL DEFAULT 1,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS customers (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          name TEXT NOT NULL,
          rfc TEXT,
          phone TEXT,
          email TEXT,
          credit_limit REAL NOT NULL DEFAULT 0,
          balance REAL NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS customer_credit_movements (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          customer_id INTEGER NOT NULL,
          amount REAL NOT NULL,
          reason TEXT NOT NULL,
          created_at TEXT NOT NULL,
          FOREIGN KEY(customer_id) REFERENCES customers(id)
        );

        CREATE TABLE IF NOT EXISTS suppliers (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          name TEXT NOT NULL,
          phone TEXT,
          contact TEXT,
          created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS taxes (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          name TEXT NOT NULL,
          type TEXT NOT NULL,
          rate REAL NOT NULL,
          country TEXT NOT NULL DEFAULT 'MX',
          parent_tax_id INTEGER,
          is_active INTEGER NOT NULL DEFAULT 1,
          FOREIGN KEY(parent_tax_id) REFERENCES taxes(id)
        );

        CREATE TABLE IF NOT EXISTS product_taxes (
          product_id INTEGER NOT NULL,
          tax_id INTEGER NOT NULL,
          PRIMARY KEY (product_id, tax_id),
          FOREIGN KEY(product_id) REFERENCES products(id),
          FOREIGN KEY(tax_id) REFERENCES taxes(id)
        );

        CREATE TABLE IF NOT EXISTS cash_sessions (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          opened_by INTEGER NOT NULL,
          opened_at TEXT NOT NULL,
          closed_at TEXT,
          opening_cash REAL NOT NULL,
          closing_cash REAL,
          expected_cash REAL NOT NULL DEFAULT 0,
          sales_total REAL NOT NULL DEFAULT 0,
          status TEXT NOT NULL DEFAULT 'open',
          FOREIGN KEY(opened_by) REFERENCES users(id)
        );

        CREATE TABLE IF NOT EXISTS shifts (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          cash_session_id INTEGER NOT NULL UNIQUE,
          opened_by INTEGER NOT NULL,
          opened_at TEXT NOT NULL,
          closed_by INTEGER,
          closed_at TEXT,
          status TEXT NOT NULL DEFAULT 'open',
          opening_cash REAL NOT NULL DEFAULT 0,
          closing_cash REAL,
          expected_cash REAL NOT NULL DEFAULT 0,
          total_tickets INTEGER NOT NULL DEFAULT 0,
          canceled_tickets INTEGER NOT NULL DEFAULT 0,
          gross_sales REAL NOT NULL DEFAULT 0,
          net_sales REAL NOT NULL DEFAULT 0,
          tax REAL NOT NULL DEFAULT 0,
          discount REAL NOT NULL DEFAULT 0,
          cash_paid REAL NOT NULL DEFAULT 0,
          card_paid REAL NOT NULL DEFAULT 0,
          transfer_paid REAL NOT NULL DEFAULT 0,
          average_ticket REAL NOT NULL DEFAULT 0,
          snapshot_json TEXT,
          FOREIGN KEY(cash_session_id) REFERENCES cash_sessions(id),
          FOREIGN KEY(opened_by) REFERENCES users(id),
          FOREIGN KEY(closed_by) REFERENCES users(id)
        );

        CREATE TABLE IF NOT EXISTS cash_movements (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          session_id INTEGER NOT NULL,
          movement_type TEXT NOT NULL,
          amount REAL NOT NULL,
          reason TEXT NOT NULL,
          actor_id INTEGER NOT NULL,
          created_at TEXT NOT NULL,
          FOREIGN KEY(session_id) REFERENCES cash_sessions(id),
          FOREIGN KEY(actor_id) REFERENCES users(id)
        );

        CREATE TABLE IF NOT EXISTS sales (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          folio TEXT NOT NULL UNIQUE,
          monthly_seq INTEGER NOT NULL DEFAULT 0,
          shift_id INTEGER,
          cashier_id INTEGER NOT NULL,
          customer_id INTEGER,
          cash_session_id INTEGER,
          subtotal REAL NOT NULL,
          tax REAL NOT NULL,
          discount REAL NOT NULL,
          total REAL NOT NULL,
          paid REAL NOT NULL,
          change_due REAL NOT NULL,
          status TEXT NOT NULL DEFAULT 'paid',
          notes TEXT,
          created_at TEXT NOT NULL,
          FOREIGN KEY(cashier_id) REFERENCES users(id),
          FOREIGN KEY(customer_id) REFERENCES customers(id),
          FOREIGN KEY(cash_session_id) REFERENCES cash_sessions(id),
          FOREIGN KEY(shift_id) REFERENCES shifts(id)
        );

        CREATE TABLE IF NOT EXISTS sale_items (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          sale_id INTEGER NOT NULL,
          product_id INTEGER NOT NULL,
          quantity REAL NOT NULL,
          unit_price REAL NOT NULL,
          discount REAL NOT NULL,
          tax_rate REAL NOT NULL,
          line_total REAL NOT NULL,
          FOREIGN KEY(sale_id) REFERENCES sales(id),
          FOREIGN KEY(product_id) REFERENCES products(id)
        );

        CREATE TABLE IF NOT EXISTS payments (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          sale_id INTEGER NOT NULL,
          method TEXT NOT NULL,
          amount REAL NOT NULL,
          reference TEXT,
          created_at TEXT NOT NULL,
          FOREIGN KEY(sale_id) REFERENCES sales(id)
        );

        CREATE TABLE IF NOT EXISTS held_tickets (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          name TEXT NOT NULL,
          cashier_id INTEGER NOT NULL,
          items_json TEXT NOT NULL,
          item_count INTEGER NOT NULL,
          total REAL NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          FOREIGN KEY(cashier_id) REFERENCES users(id)
        );

        CREATE TABLE IF NOT EXISTS active_sale_drafts (
          cashier_id INTEGER PRIMARY KEY,
          cash_session_id INTEGER,
          items_json TEXT NOT NULL,
          item_count INTEGER NOT NULL,
          total REAL NOT NULL,
          cash_received REAL NOT NULL DEFAULT 0,
          card_received REAL NOT NULL DEFAULT 0,
          transfer_received REAL NOT NULL DEFAULT 0,
          updated_at TEXT NOT NULL,
          FOREIGN KEY(cashier_id) REFERENCES users(id),
          FOREIGN KEY(cash_session_id) REFERENCES cash_sessions(id)
        );

        CREATE TABLE IF NOT EXISTS inventory_movements (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          product_id INTEGER NOT NULL,
          movement_type TEXT NOT NULL,
          quantity REAL NOT NULL,
          reason TEXT NOT NULL,
          reference_id INTEGER,
          created_at TEXT NOT NULL,
          FOREIGN KEY(product_id) REFERENCES products(id)
        );

        CREATE TABLE IF NOT EXISTS purchases (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          supplier_id INTEGER,
          total REAL NOT NULL,
          status TEXT NOT NULL,
          note TEXT,
          user_id INTEGER,
          created_at TEXT NOT NULL,
          FOREIGN KEY(supplier_id) REFERENCES suppliers(id),
          FOREIGN KEY(user_id) REFERENCES users(id)
        );

        CREATE TABLE IF NOT EXISTS purchase_items (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          purchase_id INTEGER NOT NULL,
          product_id INTEGER NOT NULL,
          quantity REAL NOT NULL,
          unit_cost REAL NOT NULL,
          line_total REAL NOT NULL,
          FOREIGN KEY(purchase_id) REFERENCES purchases(id),
          FOREIGN KEY(product_id) REFERENCES products(id)
        );

        CREATE TABLE IF NOT EXISTS supplier_products (
          supplier_id INTEGER NOT NULL,
          product_id INTEGER NOT NULL,
          supplier_price REAL,
          updated_at TEXT NOT NULL,
          PRIMARY KEY (supplier_id, product_id),
          FOREIGN KEY(supplier_id) REFERENCES suppliers(id),
          FOREIGN KEY(product_id) REFERENCES products(id)
        );

        CREATE TABLE IF NOT EXISTS price_history (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          product_id INTEGER NOT NULL,
          price REAL NOT NULL,
          recorded_at TEXT NOT NULL,
          FOREIGN KEY(product_id) REFERENCES products(id)
        );

        CREATE TABLE IF NOT EXISTS tax_profiles (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          name TEXT NOT NULL,
          rfc TEXT,
          regimen TEXT,
          postal_code TEXT,
          pac_provider TEXT,
          active INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS invoices_stub (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          sale_id INTEGER NOT NULL,
          customer_id INTEGER,
          status TEXT NOT NULL DEFAULT 'pending',
          cfdi_uuid TEXT,
          error_message TEXT,
          created_at TEXT NOT NULL,
          FOREIGN KEY(sale_id) REFERENCES sales(id),
          FOREIGN KEY(customer_id) REFERENCES customers(id)
        );

        CREATE TABLE IF NOT EXISTS company_settings (
          id INTEGER PRIMARY KEY CHECK (id = 1),
          rfc TEXT,
          fiscal_regime TEXT,
          fiscal_postal_code TEXT,
          csd_cert_path TEXT,
          csd_key_path TEXT,
          csd_password_encrypted TEXT,
          default_cfdi_use TEXT,
          invoice_series TEXT,
          logo_path TEXT,
          global_invoice_period TEXT,
          enforce_global_invoice_check INTEGER DEFAULT 1
        );

        CREATE TABLE IF NOT EXISTS sat_catalog_product_keys (
          key TEXT PRIMARY KEY,
          description TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sat_catalog_unit_keys (
          key TEXT PRIMARY KEY,
          description TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sat_catalog_cfdi_uses (
          key TEXT PRIMARY KEY,
          description TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sat_catalog_fiscal_regimes (
          key TEXT PRIMARY KEY,
          description TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS app_settings (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS audit_log (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          actor_id INTEGER,
          action TEXT NOT NULL,
          entity TEXT NOT NULL,
          entity_id INTEGER,
          details TEXT,
          created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS locked_periods (
          month TEXT PRIMARY KEY,
          locked_at TEXT NOT NULL,
          reason TEXT
        );
        ",
    )
    .map_err(|error| error.to_string())?;
    let _ = conn.execute(
        "ALTER TABLE customers ADD COLUMN credit_limit REAL NOT NULL DEFAULT 0",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE customers ADD COLUMN balance REAL NOT NULL DEFAULT 0",
        [],
    );
    let _ = conn.execute("ALTER TABLE products ADD COLUMN description TEXT", []);
    let _ = conn.execute("ALTER TABLE products ADD COLUMN sat_product_key TEXT", []);
    let _ = conn.execute("ALTER TABLE products ADD COLUMN sat_unit_key TEXT", []);
    let _ = conn.execute(
        "ALTER TABLE products ADD COLUMN track_inventory INTEGER NOT NULL DEFAULT 1",
        [],
    );
    let _ = conn.execute("ALTER TABLE suppliers ADD COLUMN contact TEXT", []);
    let _ = conn.execute("ALTER TABLE purchases ADD COLUMN note TEXT", []);
    let _ = conn.execute("ALTER TABLE purchases ADD COLUMN user_id INTEGER", []);
    let _ = conn.execute(
        "ALTER TABLE invoices_stub ADD COLUMN customer_id INTEGER",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE sales ADD COLUMN monthly_seq INTEGER NOT NULL DEFAULT 0",
        [],
    );
    let _ = conn.execute("ALTER TABLE sales ADD COLUMN shift_id INTEGER", []);
    let _ = conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_sales_monthly_seq
         ON sales(strftime('%Y-%m', created_at), monthly_seq)
         WHERE monthly_seq > 0",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sales_shift_id ON sales(shift_id)",
        [],
    );
    conn.execute_batch(
        "
        CREATE INDEX IF NOT EXISTS idx_products_active_name ON products(active, name);
        CREATE INDEX IF NOT EXISTS idx_products_active_category ON products(active, category);
        CREATE INDEX IF NOT EXISTS idx_products_active_stock ON products(active, stock);
        CREATE INDEX IF NOT EXISTS idx_product_taxes_tax_id ON product_taxes(tax_id);

        CREATE INDEX IF NOT EXISTS idx_customer_credit_movements_customer_created
          ON customer_credit_movements(customer_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_cash_sessions_status_id ON cash_sessions(status, id);
        CREATE INDEX IF NOT EXISTS idx_cash_sessions_opened_at ON cash_sessions(opened_at);
        CREATE INDEX IF NOT EXISTS idx_cash_sessions_closed_at ON cash_sessions(closed_at);
        CREATE INDEX IF NOT EXISTS idx_shifts_status_cash_session ON shifts(status, cash_session_id);

        CREATE INDEX IF NOT EXISTS idx_cash_movements_session_created
          ON cash_movements(session_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_cash_movements_actor_created
          ON cash_movements(actor_id, created_at);

        CREATE INDEX IF NOT EXISTS idx_sales_status_created_at ON sales(status, created_at);
        CREATE INDEX IF NOT EXISTS idx_sales_created_at ON sales(created_at);
        CREATE INDEX IF NOT EXISTS idx_sales_cash_session_id ON sales(cash_session_id);
        CREATE INDEX IF NOT EXISTS idx_sales_cashier_id ON sales(cashier_id);
        CREATE INDEX IF NOT EXISTS idx_sales_customer_id ON sales(customer_id);
        CREATE INDEX IF NOT EXISTS idx_sales_shift_status ON sales(shift_id, status);

        CREATE INDEX IF NOT EXISTS idx_sale_items_sale_id ON sale_items(sale_id);
        CREATE INDEX IF NOT EXISTS idx_sale_items_product_id ON sale_items(product_id);
        CREATE INDEX IF NOT EXISTS idx_payments_sale_method ON payments(sale_id, method);
        CREATE INDEX IF NOT EXISTS idx_payments_method_created ON payments(method, created_at);

        CREATE INDEX IF NOT EXISTS idx_held_tickets_cashier_updated
          ON held_tickets(cashier_id, updated_at);
        CREATE INDEX IF NOT EXISTS idx_active_sale_drafts_cash_session
          ON active_sale_drafts(cash_session_id);
        CREATE INDEX IF NOT EXISTS idx_inventory_movements_product_id_id
          ON inventory_movements(product_id, id);
        CREATE INDEX IF NOT EXISTS idx_inventory_movements_created_at
          ON inventory_movements(created_at);

        CREATE INDEX IF NOT EXISTS idx_purchases_created_at ON purchases(created_at);
        CREATE INDEX IF NOT EXISTS idx_purchases_supplier_id ON purchases(supplier_id);
        CREATE INDEX IF NOT EXISTS idx_purchases_user_id ON purchases(user_id);
        CREATE INDEX IF NOT EXISTS idx_purchase_items_purchase_id ON purchase_items(purchase_id);
        CREATE INDEX IF NOT EXISTS idx_purchase_items_product_id ON purchase_items(product_id);

        CREATE INDEX IF NOT EXISTS idx_invoices_stub_sale_id ON invoices_stub(sale_id);
        CREATE INDEX IF NOT EXISTS idx_invoices_stub_customer_id ON invoices_stub(customer_id);
        ",
    )
    .map_err(|error| error.to_string())?;
    let _ = conn.execute(
        "INSERT OR IGNORE INTO shifts (cash_session_id, opened_by, opened_at, closed_by, closed_at, status, opening_cash, closing_cash, expected_cash)
         SELECT id, opened_by, opened_at, opened_by, closed_at,
                CASE WHEN status = 'closed' THEN 'closed' ELSE 'open' END,
                opening_cash, closing_cash, expected_cash
         FROM cash_sessions",
        [],
    );
    let _ = conn.execute(
        "UPDATE sales
         SET shift_id = (
           SELECT sh.id FROM shifts sh WHERE sh.cash_session_id = sales.cash_session_id
         )
         WHERE shift_id IS NULL AND cash_session_id IS NOT NULL",
        [],
    );

    conn.execute(
        "INSERT OR IGNORE INTO taxes (id, name, type, rate, country, is_active)
         VALUES (1, 'IVA 16%', 'IVA', 0.16, 'MX', 1)",
        [],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO taxes (id, name, type, rate, country, is_active)
         VALUES (2, 'IVA 8%', 'IVA', 0.08, 'MX', 1)",
        [],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO taxes (id, name, type, rate, country, is_active)
         VALUES (3, 'Exento 0%', 'IVA', 0, 'MX', 1)",
        [],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO taxes (id, name, type, rate, country, is_active)
         VALUES (4, 'IEPS 8%', 'IEPS', 0.08, 'MX', 1)",
        [],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO taxes (id, name, type, rate, country, is_active)
         VALUES (5, 'IEPS 26.5%', 'IEPS', 0.265, 'MX', 1)",
        [],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO taxes (id, name, type, rate, country, is_active)
         VALUES (6, 'IEPS 30%', 'IEPS', 0.30, 'MX', 1)",
        [],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO company_settings (id, default_cfdi_use, invoice_series, global_invoice_period, enforce_global_invoice_check)
         VALUES (1, 'G03', 'A', 'monthly', 1)",
        [],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO sat_catalog_cfdi_uses (key, description) VALUES ('G03', 'Gastos en general')",
        [],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO sat_catalog_unit_keys (key, description) VALUES ('H87', 'Pieza')",
        [],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn seed_demo(conn: &Connection) -> CommandResult<()> {
    #[cfg(debug_assertions)]
    {
        let user_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
            .map_err(|error| error.to_string())?;
        if user_count == 0 {
            conn.execute(
            "INSERT INTO users (name, pin_hash, role, active, created_at) VALUES (?1, ?2, ?3, 1, ?4)",
            params!["Admin", hash_pin("1234")?, "admin", now_iso()],
        )
        .map_err(|error| error.to_string())?;
            conn.execute(
            "INSERT INTO users (name, pin_hash, role, active, created_at) VALUES (?1, ?2, ?3, 1, ?4)",
            params!["Cajera", hash_pin("1111")?, "cashier", now_iso()],
        )
        .map_err(|error| error.to_string())?;
        }
        let weak_demo: Option<i64> = conn
            .query_row(
                "SELECT id FROM users WHERE pin_hash = 'demo' LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if let Some(id) = weak_demo {
            conn.execute(
            "UPDATE users SET name = 'Admin', pin_hash = ?1, role = 'admin', active = 1 WHERE id = ?2",
            params![hash_pin("1234")?, id],
        )
        .map_err(|error| error.to_string())?;
        }
        let admin_exists: Option<i64> = conn
            .query_row(
                "SELECT id FROM users WHERE lower(name) = 'admin' LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if admin_exists.is_none() {
            conn.execute(
            "INSERT INTO users (name, pin_hash, role, active, created_at) VALUES (?1, ?2, ?3, 1, ?4)",
            params!["Admin", hash_pin("1234")?, "admin", now_iso()],
        )
        .map_err(|error| error.to_string())?;
        }
        let cashier_exists: Option<i64> = conn
            .query_row(
                "SELECT id FROM users WHERE lower(name) = 'cajera' LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if cashier_exists.is_none() {
            conn.execute(
            "INSERT INTO users (name, pin_hash, role, active, created_at) VALUES (?1, ?2, ?3, 1, ?4)",
            params!["Cajera", hash_pin("1111")?, "cashier", now_iso()],
        )
        .map_err(|error| error.to_string())?;
        }
    }

    let product_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM products", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if product_count == 0 {
        let products = [
            (
                "SKU-COCA-600",
                "7501055300075",
                "Refresco cola 600 ml",
                "Bebidas",
                "pieza",
                18.0,
                12.0,
                48.0,
                12.0,
                0.16,
            ),
            (
                "SKU-TORT-1K",
                "2000000000017",
                "Tortilla de maiz 1 kg",
                "Abarrotes",
                "kg",
                24.0,
                18.0,
                30.0,
                5.0,
                0.0,
            ),
            (
                "SKU-HUEVO-30",
                "2000000000024",
                "Huevo cartera 30 pzas",
                "Abarrotes",
                "pieza",
                82.0,
                68.0,
                16.0,
                4.0,
                0.0,
            ),
            (
                "SKU-SABR-45",
                "7501011131156",
                "Papas adobadas 45 g",
                "Botanas",
                "pieza",
                17.0,
                11.5,
                40.0,
                10.0,
                0.16,
            ),
            (
                "SKU-LECHE-1L",
                "7501020513318",
                "Leche entera 1 L",
                "Lacteos",
                "pieza",
                29.5,
                23.0,
                24.0,
                8.0,
                0.0,
            ),
            (
                "SKU-JABON-Z",
                "7509546041899",
                "Jabon zote rosa 400 g",
                "Limpieza",
                "pieza",
                21.0,
                15.0,
                20.0,
                6.0,
                0.16,
            ),
        ];
        for product in products {
            conn.execute(
                "INSERT INTO products
                (sku, barcode, name, category, unit, price, cost, stock, min_stock, tax_rate, active, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11, ?11)",
                params![
                    product.0,
                    product.1,
                    product.2,
                    product.3,
                    product.4,
                    product.5,
                    product.6,
                    product.7,
                    product.8,
                    product.9,
                    now_iso()
                ],
            )
            .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn map_user(row: &rusqlite::Row<'_>) -> rusqlite::Result<UserAccount> {
    Ok(UserAccount {
        id: row.get(0)?,
        name: row.get(1)?,
        role: row.get(2)?,
        active: row.get::<_, i64>(3)? == 1,
        created_at: row.get(4)?,
        permissions: Vec::new(),
    })
}

fn all_user_permissions() -> Vec<String> {
    USER_PERMISSION_KEYS
        .iter()
        .map(|permission| permission.to_string())
        .collect()
}

fn normalize_permissions(role: &str, permissions: &[String]) -> Vec<String> {
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

fn load_user_permissions(
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

fn hydrate_user_permissions(conn: &Connection, user: &mut UserAccount) -> CommandResult<()> {
    user.permissions = load_user_permissions(conn, user.id, &user.role)?;
    Ok(())
}

fn save_user_permissions(
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

fn require_permission(conn: &Connection, actor_id: i64, permission: &str) -> CommandResult<()> {
    let actor = require_active_user(conn, actor_id)?;
    if actor.role == "admin" {
        return Ok(());
    }
    let allowed: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM user_permissions WHERE user_id = ?1 AND permission_key = ?2",
            params![actor_id, permission],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if allowed == 0 {
        return Err("Permiso requerido".into());
    }
    Ok(())
}

fn map_customer(row: &rusqlite::Row<'_>) -> rusqlite::Result<Customer> {
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

fn map_inventory_movement(row: &rusqlite::Row<'_>) -> rusqlite::Result<InventoryMovement> {
    Ok(InventoryMovement {
        id: row.get(0)?,
        product_id: row.get(1)?,
        product_name: row.get(2)?,
        movement_type: row.get(3)?,
        quantity: row.get(4)?,
        reason: row.get(5)?,
        reference_id: row.get(6)?,
        created_at: row.get(7)?,
    })
}

#[tauri::command]
fn auth_needs_setup(state: State<'_, AppState>) -> CommandResult<bool> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    let active_users: i64 = conn
        .query_row("SELECT COUNT(*) FROM users WHERE active = 1", [], |row| {
            row.get(0)
        })
        .map_err(|error| error.to_string())?;
    Ok(active_users == 0)
}

#[tauri::command]
fn auth_create_initial_admin(
    state: State<'_, AppState>,
    input: InitialAdminInput,
) -> CommandResult<UserSession> {
    let name = input.name.trim();
    let pin = input.pin.trim();
    validate_required_text(name, 2, "Nombre muy corto")?;
    validate_pin(pin, 6, "PIN inicial")?;
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
fn auth_login(state: State<'_, AppState>, input: LoginInput) -> CommandResult<UserSession> {
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
        return Err("Usuario o PIN incorrecto".into());
    };
    if !verify_pin(&pin_hash, &input.pin) {
        return Err("Usuario o PIN incorrecto".into());
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
fn user_list(state: State<'_, AppState>, actor_id: i64) -> CommandResult<Vec<UserAccount>> {
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
fn user_create(
    state: State<'_, AppState>,
    actor_id: i64,
    input: UserCreateInput,
) -> CommandResult<UserAccount> {
    let name = input.name.trim();
    let pin = input.pin.trim();
    validate_required_text(name, 2, "Nombre muy corto")?;
    validate_pin(pin, 4, "PIN")?;
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
fn user_update(
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
        validate_pin(pin, 4, "PIN")?;
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
fn user_delete(state: State<'_, AppState>, actor_id: i64, id: i64) -> CommandResult<()> {
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

#[tauri::command]
fn product_search(
    state: State<'_, AppState>,
    actor_id: i64,
    query: String,
    limit: Option<i64>,
) -> CommandResult<Vec<Product>> {
    let limit = limit.unwrap_or(30).clamp(1, 100);
    let like = format!("%{}%", query.trim());
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, actor_id)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, sku, barcode, name, category, unit, price, cost, stock, min_stock, tax_rate, active
             FROM products
             WHERE active = 1
               AND (?1 = '%%' OR barcode = ?2 OR sku LIKE ?1 OR name LIKE ?1 OR category LIKE ?1)
             ORDER BY CASE WHEN barcode = ?2 THEN 0 ELSE 1 END, name
             LIMIT ?3",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![like, query.trim(), limit], map_product)
        .map_err(|error| error.to_string())?;
    let mut products = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    for product in &mut products {
        hydrate_product_taxes(&conn, product)?;
    }
    Ok(products)
}

#[tauri::command]
fn product_upsert(
    state: State<'_, AppState>,
    actor_id: i64,
    input: ProductInput,
) -> CommandResult<Product> {
    let sku = input.sku.trim();
    let barcode = input.barcode.trim();
    let name = input.name.trim();
    validate_required_text(sku, 2, "Producto incompleto")?;
    validate_required_text(barcode, 2, "Producto incompleto")?;
    validate_required_text(name, 2, "Producto incompleto")?;
    validate_non_negative(input.price, "Importe o existencia invalida")?;
    validate_non_negative(input.cost, "Importe o existencia invalida")?;
    validate_non_negative(input.stock, "Importe o existencia invalida")?;
    validate_non_negative(input.min_stock, "Importe o existencia invalida")?;
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "products")?;
    let now = now_iso();
    let active = if input.active { 1 } else { 0 };
    let tax_rate = tax_rate_for_ids(&conn, &input.tax_ids, input.tax_rate)?;
    let id = match input.id {
        Some(id) => {
            conn.execute(
                "UPDATE products
                 SET sku = ?1, barcode = ?2, name = ?3, category = ?4, unit = ?5, price = ?6,
                     cost = ?7, stock = ?8, min_stock = ?9, tax_rate = ?10, active = ?11, updated_at = ?12
                 WHERE id = ?13",
                params![
                    sku,
                    barcode,
                    name,
                    input.category.trim(),
                    input.unit.trim(),
                    input.price,
                    input.cost,
                    input.stock,
                    input.min_stock,
                    tax_rate,
                    active,
                    now,
                    id
                ],
            )
            .map_err(|error| error.to_string())?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO products
                 (sku, barcode, name, category, unit, price, cost, stock, min_stock, tax_rate, active, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
                params![
                    sku,
                    barcode,
                    name,
                    input.category.trim(),
                    input.unit.trim(),
                    input.price,
                    input.cost,
                    input.stock,
                    input.min_stock,
                    tax_rate,
                    active,
                    now
                ],
            )
            .map_err(|error| error.to_string())?;
            conn.last_insert_rowid()
        }
    };
    save_product_taxes(&conn, id, &input.tax_ids)?;
    get_product(&conn, id)
}

#[tauri::command]
fn product_delete(state: State<'_, AppState>, actor_id: i64, id: i64) -> CommandResult<()> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "products")?;
    conn.execute(
        "UPDATE products SET active = 0, updated_at = ?1 WHERE id = ?2",
        params![now_iso(), id],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn get_product(conn: &Connection, id: i64) -> CommandResult<Product> {
    let mut product = conn.query_row(
        "SELECT id, sku, barcode, name, category, unit, price, cost, stock, min_stock, tax_rate, active
         FROM products WHERE id = ?1",
        params![id],
        map_product,
    )
    .map_err(|error| error.to_string())?;
    hydrate_product_taxes(conn, &mut product)?;
    Ok(product)
}

#[tauri::command]
fn inventory_adjust(
    state: State<'_, AppState>,
    actor_id: i64,
    input: InventoryAdjustmentInput,
) -> CommandResult<Product> {
    if input.product_id <= 0 || !input.quantity.is_finite() || input.quantity == 0.0 {
        return Err("Ajuste invalido".into());
    }
    let reason = input.reason.trim();
    validate_required_text(reason, 2, "Motivo requerido")?;
    let mut conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "inventory")?;
    let tx = conn.transaction().map_err(|error| error.to_string())?;
    let current_stock: f64 = tx
        .query_row(
            "SELECT stock FROM products WHERE id = ?1 AND active = 1",
            params![input.product_id],
            |row| row.get(0),
        )
        .map_err(|_| "Producto no disponible".to_string())?;
    if current_stock + input.quantity < 0.0 {
        return Err("El ajuste deja stock negativo".into());
    }
    let now = now_iso();
    tx.execute(
        "UPDATE products SET stock = stock + ?1, updated_at = ?2 WHERE id = ?3",
        params![input.quantity, now, input.product_id],
    )
    .map_err(|error| error.to_string())?;
    tx.execute(
        "INSERT INTO inventory_movements (product_id, movement_type, quantity, reason, reference_id, created_at)
         VALUES (?1, 'adjustment', ?2, ?3, NULL, ?4)",
        params![input.product_id, input.quantity, reason, now],
    )
    .map_err(|error| error.to_string())?;
    tx.commit().map_err(|error| error.to_string())?;
    get_product(&conn, input.product_id)
}

#[tauri::command]
fn inventory_kardex(
    state: State<'_, AppState>,
    actor_id: i64,
    product_id: Option<i64>,
    limit: Option<i64>,
) -> CommandResult<Vec<InventoryMovement>> {
    let limit = limit.unwrap_or(80).clamp(1, 300);
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "inventory")?;
    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.product_id, p.name, m.movement_type, m.quantity, m.reason, m.reference_id, m.created_at
             FROM inventory_movements m
             JOIN products p ON p.id = m.product_id
             WHERE (?1 IS NULL OR m.product_id = ?1)
             ORDER BY m.id DESC
             LIMIT ?2",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![product_id, limit], map_inventory_movement)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn customer_list(state: State<'_, AppState>, actor_id: i64) -> CommandResult<Vec<Customer>> {
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
fn customer_upsert(
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
fn customer_credit_adjust(
    state: State<'_, AppState>,
    actor_id: i64,
    input: CustomerCreditInput,
) -> CommandResult<Customer> {
    if !input.amount.is_finite() || input.amount == 0.0 || input.reason.trim().len() < 2 {
        return Err("Movimiento de credito invalido".into());
    }
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "customers")?;
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE customers SET balance = balance + ?1 WHERE id = ?2",
        params![input.amount, input.customer_id],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT INTO customer_credit_movements (customer_id, amount, reason, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![input.customer_id, input.amount, input.reason.trim(), now],
    )
    .map_err(|error| error.to_string())?;
    conn.query_row(
        "SELECT id, name, rfc, phone, email, credit_limit, balance, created_at FROM customers WHERE id = ?1",
        params![input.customer_id],
        map_customer,
    )
    .map_err(|error| error.to_string())
}

fn map_supplier(row: &rusqlite::Row<'_>) -> rusqlite::Result<Supplier> {
    Ok(Supplier {
        id: row.get(0)?,
        name: row.get(1)?,
        phone: row.get(2)?,
        contact: row.get(3)?,
        created_at: row.get(4)?,
    })
}

#[tauri::command]
fn supplier_list(state: State<'_, AppState>, actor_id: i64) -> CommandResult<Vec<Supplier>> {
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
fn supplier_upsert(
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
fn purchase_create(
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
fn purchase_list(state: State<'_, AppState>, actor_id: i64) -> CommandResult<Vec<PurchaseReceipt>> {
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

#[tauri::command]
fn tax_list(state: State<'_, AppState>, actor_id: i64) -> CommandResult<Vec<TaxOption>> {
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

fn pac_message() -> String {
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
fn invoice_prepare(
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
fn invoice_list(state: State<'_, AppState>, actor_id: i64) -> CommandResult<Vec<InvoiceDraft>> {
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

fn validate_held_ticket_input(
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

fn validate_active_sale_draft_input(
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

fn map_held_ticket(row: &rusqlite::Row<'_>) -> rusqlite::Result<HeldTicket> {
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

fn map_active_sale_draft(row: &rusqlite::Row<'_>) -> rusqlite::Result<ActiveSaleDraft> {
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
fn held_ticket_list(state: State<'_, AppState>) -> CommandResult<Vec<HeldTicket>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
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
fn held_ticket_save(
    state: State<'_, AppState>,
    input: HeldTicketInput,
) -> CommandResult<HeldTicket> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    let tax_enabled = setting_bool(&conn, "tax_enabled", true)?;
    let prices_include_tax = true;
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
fn held_ticket_delete(state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    conn.execute("DELETE FROM held_tickets WHERE id = ?1", params![id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn active_sale_draft_get(
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
fn active_sale_draft_save(
    state: State<'_, AppState>,
    input: ActiveSaleDraftInput,
) -> CommandResult<ActiveSaleDraft> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    let tax_enabled = setting_bool(&conn, "tax_enabled", true)?;
    let prices_include_tax = true;
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
fn active_sale_draft_clear(state: State<'_, AppState>, cashier_id: i64) -> CommandResult<()> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    conn.execute(
        "DELETE FROM active_sale_drafts WHERE cashier_id = ?1",
        params![cashier_id],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn sale_create(state: State<'_, AppState>, draft: SaleDraft) -> CommandResult<SaleReceipt> {
    if draft.items.is_empty() {
        return Err("Venta sin articulos".into());
    }
    if draft.payments.is_empty() {
        return Err("Venta sin pagos".into());
    }

    let mut conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, draft.cashier_id)?;
    let tax_enabled = setting_bool(&conn, "tax_enabled", true)?;
    let prices_include_tax = true;
    let tx = conn.transaction().map_err(|error| error.to_string())?;
    let (shift_id, cash_session_id) = get_open_shift(&tx)?
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
    let change_due = round_money(paid - total);
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

    let cash_paid: f64 = draft
        .payments
        .iter()
        .filter(|payment| payment.method == "cash")
        .map(|payment| payment.amount)
        .sum();
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
fn sale_list(
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

#[tauri::command]
fn sale_cancel(
    state: State<'_, AppState>,
    sale_id: i64,
    actor_id: i64,
    reason: String,
) -> CommandResult<()> {
    if reason.trim().len() < 2 {
        return Err("Motivo requerido".into());
    }
    let mut conn = state.db.lock().map_err(|error| error.to_string())?;
    require_admin(&conn, actor_id)?;
    let tx = conn.transaction().map_err(|error| error.to_string())?;
    let (status, cash_session_id, total, created_at): (String, Option<i64>, f64, String) = tx
        .query_row(
            "SELECT status, cash_session_id, total, created_at FROM sales WHERE id = ?1",
            params![sale_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
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
        "UPDATE sales SET status = 'canceled', notes = COALESCE(notes, '') || ?1 WHERE id = ?2",
        params![format!(" | Cancelada: {}", reason.trim()), sale_id],
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
fn cash_session_open(
    state: State<'_, AppState>,
    opened_by: i64,
    opening_cash: f64,
) -> CommandResult<CashSession> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, opened_by)?;
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM cash_sessions WHERE status = 'open' ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if existing.is_some() {
        return Err("Ya hay caja abierta".into());
    }
    let opened_at = now_iso();
    conn.execute(
        "INSERT INTO cash_sessions (opened_by, opened_at, opening_cash, expected_cash, sales_total, status)
         VALUES (?1, ?2, ?3, ?3, 0, 'open')",
        params![opened_by, opened_at, opening_cash],
    )
    .map_err(|error| error.to_string())?;
    let cash_session_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO shifts (cash_session_id, opened_by, opened_at, status, opening_cash, expected_cash)
         VALUES (?1, ?2, ?3, 'open', ?4, ?4)",
        params![cash_session_id, opened_by, opened_at, opening_cash],
    )
    .map_err(|error| error.to_string())?;
    get_cash_session(&conn, cash_session_id)
}

fn map_cash_movement(row: &rusqlite::Row<'_>) -> rusqlite::Result<CashMovement> {
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
fn cash_movement_create(
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
fn cash_movement_list(
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

#[tauri::command]
fn cash_session_close(
    state: State<'_, AppState>,
    session_id: i64,
    closing_cash: f64,
) -> CommandResult<CashSession> {
    let _ = (state, session_id, closing_cash);
    Err("Use Corte Z para cerrar turno oficialmente".into())
}

#[tauri::command]
fn shift_cut_x(
    state: State<'_, AppState>,
    actor_id: i64,
    shift_id: Option<i64>,
) -> CommandResult<ShiftCutSnapshot> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, actor_id)?;
    let next_shift_id = match shift_id {
        Some(id) => id,
        None => get_open_shift(&conn)?
            .map(|(id, _)| id)
            .ok_or_else(|| "No hay turno abierto".to_string())?,
    };
    calculate_shift_cut(&conn, next_shift_id)
}

#[tauri::command]
fn shift_cut_z(
    state: State<'_, AppState>,
    shift_id: i64,
    closing_cash: f64,
    closed_by: i64,
) -> CommandResult<ShiftCutSnapshot> {
    if closing_cash < 0.0 {
        return Err("Efectivo contado invalido".into());
    }
    let mut conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, closed_by)?;
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
    let closed_at = now_iso();
    snapshot.status = "closed".into();
    snapshot.closed_at = Some(closed_at.clone());
    snapshot.closing_cash = Some(round_money(closing_cash));
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
fn monthly_sales_report(
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
fn period_lock(
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

fn get_cash_session(conn: &Connection, id: i64) -> CommandResult<CashSession> {
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

fn get_open_shift(conn: &Connection) -> CommandResult<Option<(i64, i64)>> {
    conn.query_row(
        "SELECT sh.id, sh.cash_session_id
         FROM shifts sh
         JOIN cash_sessions cs ON cs.id = sh.cash_session_id
         WHERE sh.status = 'open' AND cs.status = 'open'
         ORDER BY sh.id DESC
         LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
    .map_err(|error| error.to_string())
}

fn calculate_shift_cut(conn: &Connection, shift_id: i64) -> CommandResult<ShiftCutSnapshot> {
    let (
        cash_session_id,
        status,
        opened_at,
        closed_at,
        opening_cash,
        closing_cash,
        expected_cash,
    ): (i64, String, String, Option<String>, f64, Option<f64>, f64) = conn
        .query_row(
            "SELECT cash_session_id, status, opened_at, closed_at, opening_cash, closing_cash, expected_cash
             FROM shifts WHERE id = ?1",
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
                ))
            },
        )
        .map_err(|_| "Turno no encontrado".to_string())?;

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

    let (cash_received, card_paid, transfer_paid): (f64, f64, f64) = conn
        .query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN p.method = 'cash' THEN p.amount ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN p.method = 'card' THEN p.amount ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN p.method = 'transfer' THEN p.amount ELSE 0 END), 0)
             FROM payments p
             JOIN sales s ON s.id = p.sale_id
             WHERE s.shift_id = ?1 AND s.status = 'paid'",
            params![shift_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|error| error.to_string())?;
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
    let cash_paid = round_money((cash_received - cash_change).max(0.0));
    let net_sales = round_money(net_sales);
    Ok(ShiftCutSnapshot {
        shift_id,
        cash_session_id,
        status,
        opened_at,
        closed_at,
        total_tickets,
        canceled_tickets,
        gross_sales: round_money(net_sales + discount),
        net_sales,
        tax: round_money(tax),
        discount: round_money(discount),
        cash_paid,
        card_paid: round_money(card_paid),
        transfer_paid: round_money(transfer_paid),
        average_ticket: average_ticket(net_sales, total_tickets),
        opening_cash: round_money(opening_cash),
        expected_cash: round_money(expected_cash),
        closing_cash: closing_cash.map(round_money),
    })
}

#[tauri::command]
fn dashboard_summary(state: State<'_, AppState>, actor_id: i64) -> CommandResult<DashboardSummary> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, actor_id)?;
    let active_products = conn
        .query_row(
            "SELECT COUNT(*) FROM products WHERE active = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let low_stock_products = conn
        .query_row(
            "SELECT COUNT(*) FROM products WHERE active = 1 AND stock <= 0",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let today_sales = conn
        .query_row(
            "SELECT COALESCE(SUM(total), 0) FROM sales WHERE date(created_at) = date('now')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let today_tickets = conn
        .query_row(
            "SELECT COUNT(*) FROM sales WHERE date(created_at) = date('now')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let open_cash_session_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM cash_sessions WHERE status = 'open' ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let open_cash_session = match open_cash_session_id {
        Some(id) => Some(get_cash_session(&conn, id)?),
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
fn report_summary(state: State<'_, AppState>, actor_id: i64) -> CommandResult<ReportSummary> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "reports")?;
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
            "SELECT COALESCE(expected_cash, 0) FROM cash_sessions WHERE status = 'open' ORDER BY id DESC LIMIT 1",
            [],
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
    Ok(ReportSummary {
        today_sales,
        today_tickets,
        average_ticket: if today_tickets > 0 {
            round_money(today_sales / today_tickets as f64)
        } else {
            0.0
        },
        gross_profit: round_money(gross_profit),
        cash_expected,
        cash_sales: round_money(cash_sales),
        card_sales: round_money(card_sales),
        transfer_sales: round_money(transfer_sales),
        low_stock_products,
    })
}

#[tauri::command]
fn report_product_sales(
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
            "SELECT p.id, p.name, COALESCE(SUM(si.quantity), 0), COALESCE(SUM(si.line_total), 0)
             FROM sale_items si
             JOIN sales s ON s.id = si.sale_id
             JOIN products p ON p.id = si.product_id
             WHERE s.status = 'paid'
               AND (?2 IS NULL OR date(s.created_at) >= date(?2))
               AND (?3 IS NULL OR date(s.created_at) <= date(?3))
             GROUP BY p.id, p.name
             ORDER BY SUM(si.line_total) DESC
             LIMIT ?1",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![limit, from_date, to_date], |row| {
            Ok(ProductSalesReport {
                product_id: row.get(0)?,
                product_name: row.get(1)?,
                quantity: row.get(2)?,
                total: row.get(3)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn report_movement_history(
    state: State<'_, AppState>,
    actor_id: i64,
    limit: Option<i64>,
) -> CommandResult<Vec<ReportMovement>> {
    let limit = limit.unwrap_or(160).clamp(20, 500);
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "reports")?;
    let mut stmt = conn
        .prepare(
            "SELECT id, kind, title, detail, amount, cash_paid, card_paid, transfer_paid, actor_name, cash_session_id, created_at
             FROM (
               SELECT
                 'sale-' || s.id AS id,
                 'sale' AS kind,
                 CASE WHEN s.status = 'paid' THEN 'Venta ' || s.folio ELSE 'Cancelacion ' || s.folio END AS title,
                 'Efectivo ' || printf('%.2f', COALESCE(pay.cash_paid, 0)) ||
                   ' · Tarjeta ' || printf('%.2f', COALESCE(pay.card_paid, 0)) ||
                   ' · Crédito ' || printf('%.2f', COALESCE(pay.transfer_paid, 0)) AS detail,
                 CASE WHEN s.status = 'paid' THEN s.total ELSE -s.total END AS amount,
                 COALESCE(pay.cash_paid, 0) AS cash_paid,
                 COALESCE(pay.card_paid, 0) AS card_paid,
                 COALESCE(pay.transfer_paid, 0) AS transfer_paid,
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
                   SUM(CASE WHEN method = 'transfer' THEN amount ELSE 0 END) AS transfer_paid
                 FROM payments
                 GROUP BY sale_id
               ) pay ON pay.sale_id = s.id
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
                 u.name,
                 cs.id,
                 COALESCE(cs.closed_at, cs.opened_at)
               FROM cash_sessions cs
               JOIN users u ON u.id = cs.opened_by
               WHERE cs.closed_at IS NOT NULL
             )
             ORDER BY created_at DESC
             LIMIT ?1",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![limit], |row| {
            Ok(ReportMovement {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                detail: row.get(3)?,
                amount: row.get(4)?,
                cash_paid: row.get(5)?,
                card_paid: row.get(6)?,
                transfer_paid: row.get(7)?,
                actor_name: row.get(8)?,
                cash_session_id: row.get(9)?,
                created_at: row.get(10)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn clean_device_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(180)
        .collect()
}

fn command_lines(program: &str, args: &[&str]) -> Vec<String> {
    let Ok(output) = Command::new(program).args(args).output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(clean_device_text)
        .filter(|line| !line.is_empty())
        .collect()
}

fn add_device(
    devices: &mut Vec<HardwareDevice>,
    seen: &mut HashSet<String>,
    device: HardwareDevice,
) {
    if seen.insert(format!("{}:{}", device.device_type, device.id)) {
        devices.push(device);
    }
}

fn detect_unix_printers(devices: &mut Vec<HardwareDevice>, seen: &mut HashSet<String>) {
    let default_printer = command_lines("lpstat", &["-d"])
        .into_iter()
        .find_map(|line| {
            line.strip_prefix("system default destination: ")
                .map(str::to_string)
        });
    let uri_lines = command_lines("lpstat", &["-v"]);
    for line in command_lines("lpstat", &["-p"]) {
        let Some(rest) = line.strip_prefix("printer ") else {
            continue;
        };
        let name = rest.split_whitespace().next().unwrap_or("").trim();
        if name.is_empty() {
            continue;
        }
        let detail = uri_lines
            .iter()
            .find_map(|uri_line| {
                uri_line
                    .strip_prefix(&format!("device for {name}: "))
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "Impresora del sistema".into());
        add_device(
            devices,
            seen,
            HardwareDevice {
                id: name.into(),
                name: name.into(),
                device_type: "printer".into(),
                connection: "system".into(),
                detail,
                is_default: default_printer.as_deref() == Some(name),
            },
        );
    }
}

fn detect_windows_printers(devices: &mut Vec<HardwareDevice>, seen: &mut HashSet<String>) {
    let command = "Get-Printer | ForEach-Object { \"$($_.Name)|$($_.DriverName)|$($_.PortName)\" }";
    for line in command_lines("powershell", &["-NoProfile", "-Command", command]) {
        let parts: Vec<&str> = line.split('|').collect();
        let name = clean_device_text(parts.first().copied().unwrap_or(""));
        if name.is_empty() {
            continue;
        }
        let driver = clean_device_text(parts.get(1).copied().unwrap_or(""));
        let port = clean_device_text(parts.get(2).copied().unwrap_or(""));
        add_device(
            devices,
            seen,
            HardwareDevice {
                id: name.clone(),
                name,
                device_type: "printer".into(),
                connection: "windows-printer".into(),
                detail: format!("{driver} {port}").trim().into(),
                is_default: false,
            },
        );
    }
}

fn detect_serial_paths(devices: &mut Vec<HardwareDevice>, seen: &mut HashSet<String>) {
    #[cfg(windows)]
    {
        let command =
            "Get-CimInstance Win32_SerialPort | ForEach-Object { \"$($_.DeviceID)|$($_.Name)\" }";
        for line in command_lines("powershell", &["-NoProfile", "-Command", command]) {
            let parts: Vec<&str> = line.split('|').collect();
            let id = clean_device_text(parts.first().copied().unwrap_or(""));
            if id.is_empty() {
                continue;
            }
            let name = clean_device_text(parts.get(1).copied().unwrap_or(&id));
            add_device(
                devices,
                seen,
                HardwareDevice {
                    id: id.clone(),
                    name,
                    device_type: "serial".into(),
                    connection: "serial".into(),
                    detail: "Puerto serial local".into(),
                    is_default: false,
                },
            );
        }
    }

    #[cfg(not(windows))]
    {
        let prefixes = [
            "ttyUSB", "ttyACM", "tty.usb", "cu.usb", "ttyS", "cu.SLAB", "tty.SLAB",
        ];
        if let Ok(entries) = fs::read_dir("/dev") {
            for entry in entries.flatten() {
                let filename = entry.file_name().to_string_lossy().to_string();
                if !prefixes.iter().any(|prefix| filename.starts_with(prefix)) {
                    continue;
                }
                let path = format!("/dev/{filename}");
                add_device(
                    devices,
                    seen,
                    HardwareDevice {
                        id: path.clone(),
                        name: filename,
                        device_type: "serial".into(),
                        connection: "serial".into(),
                        detail: path,
                        is_default: false,
                    },
                );
            }
        }
        if let Ok(entries) = fs::read_dir("/dev/serial/by-id") {
            for entry in entries.flatten() {
                let path = entry.path().to_string_lossy().to_string();
                let name = entry.file_name().to_string_lossy().to_string();
                add_device(
                    devices,
                    seen,
                    HardwareDevice {
                        id: path.clone(),
                        name,
                        device_type: "serial".into(),
                        connection: "serial-by-id".into(),
                        detail: path,
                        is_default: false,
                    },
                );
            }
        }
    }
}

#[tauri::command]
fn hardware_device_list(
    state: State<'_, AppState>,
    actor_id: i64,
) -> CommandResult<Vec<HardwareDevice>> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_admin(&conn, actor_id)?;
    drop(conn);
    let mut devices = Vec::new();
    let mut seen = HashSet::new();
    let os = env::consts::OS;
    if os == "windows" {
        detect_windows_printers(&mut devices, &mut seen);
    } else {
        detect_unix_printers(&mut devices, &mut seen);
    }
    detect_serial_paths(&mut devices, &mut seen);

    if devices.is_empty() {
        add_device(
            &mut devices,
            &mut seen,
            HardwareDevice {
                id: "mock-printer-80mm".into(),
                name: "Mock 80mm".into(),
                device_type: "printer".into(),
                connection: "mock".into(),
                detail: "Dispositivo de prueba".into(),
                is_default: true,
            },
        );
    }
    Ok(devices)
}

fn temp_hardware_file(prefix: &str, extension: &str) -> PathBuf {
    let clean_prefix = prefix
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .collect::<String>();
    env::temp_dir().join(format!(
        "{clean_prefix}-{}.{}",
        Utc::now().timestamp_nanos_opt().unwrap_or(0),
        extension
    ))
}

#[cfg(windows)]
fn ps_single_quote(value: &str) -> String {
    value.replace('\'', "''")
}

fn run_print_file(printer: &str, file: &PathBuf, raw: bool) -> CommandResult<()> {
    let printer = clean_device_text(printer);
    if printer.is_empty() || printer.starts_with("mock-") {
        return Err("Configura una impresora real en Configuracion".into());
    }
    #[cfg(windows)]
    {
        if raw {
            return Err("Pulso raw de cajon no soportado por PowerShell. Usa impresora ESC/POS compartida por puerto raw.".into());
        }
        let path = ps_single_quote(&file.to_string_lossy());
        let printer = ps_single_quote(&printer);
        let command = format!("Get-Content -Raw '{path}' | Out-Printer -Name '{printer}'");
        let output = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(command)
            .output()
            .map_err(|error| format!("No se pudo imprimir: {error}"))?;
        if output.status.success() {
            return Ok(());
        }
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    #[cfg(not(windows))]
    {
        let file_path = file.to_string_lossy().to_string();
        let mut command = Command::new("lp");
        if raw {
            command.args(["-o", "raw"]);
        }
        let output = command
            .arg("-d")
            .arg(&printer)
            .arg(&file_path)
            .output()
            .map_err(|error| format!("No se pudo imprimir con lp: {error}"))?;
        if output.status.success() {
            return Ok(());
        }
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn ticket_setting(conn: &Connection, key: &str, default: &str) -> CommandResult<String> {
    Ok(setting_string(conn, key)?.unwrap_or_else(|| default.to_string()))
}

fn ticket_setting_i64(
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

fn ticket_separator(width: usize) -> String {
    "-".repeat(width.clamp(24, 48))
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
            demo.push_str(header.trim());
            demo.push('\n');
        }
        demo.push_str("Prueba de impresora\n");
        if show_date {
            demo.push_str("2026-06-20 08:20\n");
        }
        demo.push_str(&format!("{separator}\n*** OK ***\n"));
        if !footer.trim().is_empty() {
            demo.push('\n');
            demo.push_str(footer.trim());
            demo.push('\n');
        }
        demo.push_str(&"\n".repeat(extra_lines));
        return Ok(demo);
    }
    let (folio, subtotal, tax, total, paid, change_due, created_at, cashier_name): (
        String,
        f64,
        f64,
        f64,
        f64,
        f64,
        String,
        String,
    ) = conn
        .query_row(
            "SELECT s.folio, s.subtotal, s.tax, s.total, s.paid, s.change_due, s.created_at, u.name
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
        text.push_str(header.trim());
        text.push('\n');
    }
    text.push_str(&format!("Folio {folio}\n"));
    if show_date {
        text.push_str(&format!("{created_at}\n"));
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
        text.push_str(&format!("{name}\n  {quantity:.3} @ ${unit_price:.2}"));
        if discount > 0.0 {
            text.push_str(&format!(" desc ${discount:.2}"));
        }
        text.push_str(&format!("  ${line_total:.2}\n"));
        if show_barcode && !barcode.trim().is_empty() {
            text.push_str(&format!("  {barcode}\n"));
        }
    }
    text.push_str(&format!("{separator}\n"));
    if show_tax {
        text.push_str(&format!(
            "SUBTOTAL        ${subtotal:.2}\nIMPUESTOS       ${tax:.2}\n"
        ));
    }
    text.push_str(&format!("*** TOTAL       ${total:.2}\nPAGADO          ${paid:.2}\nCAMBIO          ${change_due:.2}\n"));
    if show_item_count {
        text.push_str(&format!("Articulos: {item_count:.3}\n"));
    }
    if !footer.trim().is_empty() {
        text.push('\n');
        text.push_str(footer.trim());
        text.push('\n');
    }
    text.push_str(&"\n".repeat(extra_lines));
    Ok(text)
}

#[tauri::command]
fn print_ticket(state: State<'_, AppState>, sale_id: i64) -> CommandResult<HardwareResult> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    let printer = setting_string(&conn, "printer")?.unwrap_or_default();
    let copies = ticket_setting_i64(&conn, "ticket_copies", 1, 1, 4)?;
    let text = receipt_text(&conn, sale_id)?;
    let text = if copies > 1 {
        (0..copies)
            .map(|_| text.clone())
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        text
    };
    let file = temp_hardware_file("rim-pos-ticket", "txt");
    fs::write(&file, text).map_err(|error| format!("No se pudo crear ticket temporal: {error}"))?;
    run_print_file(&printer, &file, false)?;
    let _ = fs::remove_file(file);
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
    let file = temp_hardware_file("rim-pos-drawer", "bin");
    fs::write(&file, [0x1B, 0x70, 0x00, 0x40, 0x50])
        .map_err(|error| format!("No se pudo crear pulso de cajon: {error}"))?;
    run_print_file(&drawer, &file, true)?;
    let _ = fs::remove_file(file);
    Ok(HardwareResult {
        ok: true,
        message: format!("Pulso de cajon enviado a {drawer}"),
    })
}

#[tauri::command]
fn read_scale(state: State<'_, AppState>) -> CommandResult<ScaleReading> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    let scale = setting_string(&conn, "scale")?.unwrap_or_default();
    if scale.is_empty() || scale.starts_with("mock-") {
        return Err("Configura una bascula serial real en Configuracion".into());
    }
    Err(format!(
        "Bascula {scale} detectada, pero lectura serial requiere protocolo del modelo. No se devuelve peso simulado."
    ))
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
fn backup_create(state: State<'_, AppState>, actor_id: i64) -> CommandResult<BackupResult> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_admin(&conn, actor_id)?;
    backup_create_with_conn(&conn, &state.db_path)
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
    Ok(Some(backup))
}

#[cfg(test)]
mod tests {
    use super::{
        average_ticket, hash_pin, legacy_hash_pin, line_amounts, next_monthly_seq, period_key,
        validation, verify_pin, visible_monthly_folio,
    };

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
    fn validation_rejects_weak_or_non_numeric_pin() {
        assert!(validation::validate_pin("1234", 4, "PIN").is_ok());
        assert!(validation::validate_pin("12a4", 4, "PIN").is_err());
        assert!(validation::validate_pin("123", 4, "PIN").is_err());
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
}

pub fn run() {
    tauri::Builder::default()
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
            product_search,
            product_upsert,
            product_delete,
            inventory_adjust,
            inventory_kardex,
            auth_needs_setup,
            auth_create_initial_admin,
            auth_login,
            user_list,
            user_create,
            user_update,
            user_delete,
            customer_list,
            customer_upsert,
            customer_credit_adjust,
            supplier_list,
            supplier_upsert,
            purchase_create,
            purchase_list,
            tax_list,
            invoice_prepare,
            invoice_list,
            held_ticket_list,
            held_ticket_save,
            held_ticket_delete,
            active_sale_draft_get,
            active_sale_draft_save,
            active_sale_draft_clear,
            sale_create,
            sale_list,
            sale_cancel,
            cash_session_open,
            cash_session_close,
            shift_cut_x,
            shift_cut_z,
            cash_movement_create,
            cash_movement_list,
            dashboard_summary,
            report_summary,
            report_product_sales,
            report_movement_history,
            monthly_sales_report,
            period_lock,
            hardware_device_list,
            print_ticket,
            open_cash_drawer,
            read_scale,
            settings_get,
            settings_set,
            backup_create,
            backup_auto_if_due
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
