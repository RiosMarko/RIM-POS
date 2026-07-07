use crate::backend::CommandResult;
use crate::core::now_iso;
use crate::products::product_search_text;
use crate::security::hash_pin;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub(crate) fn init_db(app: &AppHandle) -> CommandResult<(Connection, PathBuf)> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("No se pudo localizar app data: {error}"))?;
    fs::create_dir_all(&data_dir).map_err(|error| format!("No se pudo crear app data: {error}"))?;
    let db_path = data_dir.join("pos-abarrotes.sqlite3");
    let conn = Connection::open_with_flags(
        &db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )
    .map_err(|error| error.to_string())?;
    configure_connection(&conn)?;
    migrate(&conn)?;
    seed_demo(&conn)?;
    let db_path = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("No se pudo localizar app data: {error}"))?
        .join("pos-abarrotes.sqlite3");
    Ok((conn, db_path))
}

pub(crate) fn configure_connection(conn: &Connection) -> CommandResult<()> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        PRAGMA busy_timeout = 5000;
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA temp_store = MEMORY;
        PRAGMA cache_size = -20000;
        ",
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn migrate(conn: &Connection) -> CommandResult<()> {
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
          wholesale_price REAL,
          cost REAL NOT NULL DEFAULT 0,
          stock REAL NOT NULL DEFAULT 0,
          min_stock REAL NOT NULL DEFAULT 0,
          tax_rate REAL NOT NULL DEFAULT 0,
          active INTEGER NOT NULL DEFAULT 1,
          search_text TEXT NOT NULL DEFAULT '',
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
          workstation_id TEXT NOT NULL DEFAULT 'CAJA-1',
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

        CREATE TABLE IF NOT EXISTS cash_counts (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          session_id INTEGER NOT NULL,
          shift_id INTEGER,
          count_type TEXT NOT NULL,
          expected_cash REAL NOT NULL,
          counted_cash REAL NOT NULL,
          difference REAL NOT NULL,
          denominations_json TEXT NOT NULL,
          difference_reason TEXT,
          actor_id INTEGER NOT NULL,
          created_at TEXT NOT NULL,
          FOREIGN KEY(session_id) REFERENCES cash_sessions(id),
          FOREIGN KEY(shift_id) REFERENCES shifts(id),
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

        CREATE TABLE IF NOT EXISTS sale_returns (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          sale_id INTEGER NOT NULL,
          sale_item_id INTEGER NOT NULL,
          product_id INTEGER NOT NULL,
          quantity REAL NOT NULL,
          refund_total REAL NOT NULL,
          cash_refund REAL NOT NULL,
          reason TEXT NOT NULL,
          actor_id INTEGER NOT NULL,
          cash_session_id INTEGER,
          shift_id INTEGER,
          created_at TEXT NOT NULL,
          FOREIGN KEY(sale_id) REFERENCES sales(id),
          FOREIGN KEY(sale_item_id) REFERENCES sale_items(id),
          FOREIGN KEY(product_id) REFERENCES products(id),
          FOREIGN KEY(actor_id) REFERENCES users(id)
        );

        CREATE INDEX IF NOT EXISTS idx_sale_returns_shift ON sale_returns(shift_id);
        CREATE INDEX IF NOT EXISTS idx_sale_returns_item ON sale_returns(sale_item_id);

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

        CREATE TABLE IF NOT EXISTS schema_migrations (
          version INTEGER PRIMARY KEY,
          name TEXT NOT NULL,
          applied_at TEXT NOT NULL
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
    let _ = conn.execute("ALTER TABLE products ADD COLUMN wholesale_price REAL", []);
    let _ = conn.execute(
        "ALTER TABLE products ADD COLUMN search_text TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = conn.execute("ALTER TABLE suppliers ADD COLUMN contact TEXT", []);
    let _ = conn.execute(
        "ALTER TABLE suppliers ADD COLUMN active INTEGER NOT NULL DEFAULT 1",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE inventory_movements ADD COLUMN actor_id INTEGER",
        [],
    );
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
    let _ = conn.execute(
        "ALTER TABLE cash_sessions ADD COLUMN workstation_id TEXT NOT NULL DEFAULT 'CAJA-1'",
        [],
    );
    let _ = conn.execute("ALTER TABLE sales ADD COLUMN shift_id INTEGER", []);
    let _ = conn.execute("ALTER TABLE sales ADD COLUMN canceled_at TEXT", []);
    let _ = conn.execute("ALTER TABLE sales ADD COLUMN canceled_by INTEGER", []);
    let _ = conn.execute("ALTER TABLE sales ADD COLUMN cancel_reason TEXT", []);
    let _ = conn.execute(
        "ALTER TABLE customer_credit_movements ADD COLUMN movement_kind TEXT NOT NULL DEFAULT 'adjust'",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE customer_credit_movements ADD COLUMN payment_method TEXT",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE customer_credit_movements ADD COLUMN actor_id INTEGER",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE customer_credit_movements ADD COLUMN cash_session_id INTEGER",
        [],
    );
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
        CREATE INDEX IF NOT EXISTS idx_products_active_barcode ON products(active, barcode);
        CREATE INDEX IF NOT EXISTS idx_products_active_stock ON products(active, stock);
        CREATE INDEX IF NOT EXISTS idx_products_active_search ON products(active, search_text);
        CREATE INDEX IF NOT EXISTS idx_products_lower_sku ON products(lower(sku));
        CREATE INDEX IF NOT EXISTS idx_products_active_updated ON products(active, updated_at);
        CREATE INDEX IF NOT EXISTS idx_product_taxes_tax_id ON product_taxes(tax_id);

        CREATE INDEX IF NOT EXISTS idx_customer_credit_movements_customer_created
          ON customer_credit_movements(customer_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_customer_credit_movements_session_created
          ON customer_credit_movements(cash_session_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_cash_sessions_status_id ON cash_sessions(status, id);
        CREATE INDEX IF NOT EXISTS idx_cash_sessions_workstation_status ON cash_sessions(workstation_id, status);
        CREATE INDEX IF NOT EXISTS idx_cash_sessions_opened_at ON cash_sessions(opened_at);
        CREATE INDEX IF NOT EXISTS idx_cash_sessions_closed_at ON cash_sessions(closed_at);
        CREATE INDEX IF NOT EXISTS idx_shifts_status_cash_session ON shifts(status, cash_session_id);

        CREATE INDEX IF NOT EXISTS idx_cash_movements_session_created
          ON cash_movements(session_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_cash_movements_actor_created
          ON cash_movements(actor_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_cash_counts_session_created
          ON cash_counts(session_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_cash_counts_shift_created
          ON cash_counts(shift_id, created_at);

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
    conn.execute(
        "UPDATE products
         SET search_text = lower(sku || ' ' || barcode || ' ' || name || ' ' || category || ' ' || unit)
         WHERE search_text = ''",
        [],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations (version, name, applied_at)
         VALUES (1, 'base_local_pos_schema', ?1)",
        params![now_iso()],
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

pub(crate) fn seed_demo(conn: &Connection) -> CommandResult<()> {
    #[cfg(not(debug_assertions))]
    let _ = conn;

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

    #[cfg(debug_assertions)]
    {
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
                (
                    "SKU-ARROZ-1K",
                    "7501000000011",
                    "Arroz super extra 1 kg",
                    "Abarrotes",
                    "pieza",
                    32.0,
                    24.0,
                    20.0,
                    0.0,
                    0.0,
                ),
                (
                    "SKU-FRIJOL-1K",
                    "7501000000028",
                    "Frijol pinto 1 kg",
                    "Abarrotes",
                    "pieza",
                    42.0,
                    33.0,
                    18.0,
                    0.0,
                    0.0,
                ),
                (
                    "SKU-AZUCAR-1K",
                    "7501000000035",
                    "Azucar estandar 1 kg",
                    "Abarrotes",
                    "pieza",
                    30.0,
                    23.0,
                    24.0,
                    0.0,
                    0.0,
                ),
                (
                    "SKU-SAL-1K",
                    "7501000000042",
                    "Sal refinada 1 kg",
                    "Abarrotes",
                    "pieza",
                    18.0,
                    12.5,
                    18.0,
                    0.0,
                    0.0,
                ),
                (
                    "SKU-ACEITE-850",
                    "7501000000059",
                    "Aceite vegetal 850 ml",
                    "Abarrotes",
                    "pieza",
                    48.0,
                    39.0,
                    18.0,
                    0.0,
                    0.0,
                ),
                (
                    "SKU-ATUN-140",
                    "7501000000066",
                    "Atun en agua 140 g",
                    "Abarrotes",
                    "pieza",
                    24.0,
                    18.0,
                    30.0,
                    0.0,
                    0.0,
                ),
                (
                    "SKU-SARDINA-425",
                    "7501000000073",
                    "Sardina en tomate 425 g",
                    "Abarrotes",
                    "pieza",
                    36.0,
                    28.0,
                    14.0,
                    0.0,
                    0.0,
                ),
                (
                    "SKU-MAYONESA-390",
                    "7501000000080",
                    "Mayonesa 390 g",
                    "Abarrotes",
                    "pieza",
                    54.0,
                    42.0,
                    10.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-CATSUP-370",
                    "7501000000097",
                    "Catsup 370 g",
                    "Abarrotes",
                    "pieza",
                    34.0,
                    25.0,
                    12.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-PASTA-200",
                    "7501000000103",
                    "Pasta sopa codito 200 g",
                    "Abarrotes",
                    "pieza",
                    13.0,
                    9.0,
                    36.0,
                    0.0,
                    0.0,
                ),
                (
                    "SKU-GALLETA-MARIA",
                    "7501000000110",
                    "Galleta Maria 170 g",
                    "Galletas",
                    "pieza",
                    18.0,
                    13.0,
                    24.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-GALLETA-SAL",
                    "7501000000127",
                    "Galleta salada 186 g",
                    "Galletas",
                    "pieza",
                    20.0,
                    15.0,
                    24.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-PAN-CAJA",
                    "7501000000134",
                    "Pan blanco de caja",
                    "Panaderia",
                    "pieza",
                    48.0,
                    38.0,
                    8.0,
                    0.0,
                    0.0,
                ),
                (
                    "SKU-BIMB-ROLES",
                    "7501000000141",
                    "Roles glaseados paquete",
                    "Panaderia",
                    "pieza",
                    25.0,
                    18.0,
                    12.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-AGUA-600",
                    "7501000000158",
                    "Agua natural 600 ml",
                    "Bebidas",
                    "pieza",
                    12.0,
                    7.0,
                    48.0,
                    0.0,
                    0.0,
                ),
                (
                    "SKU-AGUA-1L",
                    "7501000000165",
                    "Agua natural 1 L",
                    "Bebidas",
                    "pieza",
                    18.0,
                    11.0,
                    36.0,
                    0.0,
                    0.0,
                ),
                (
                    "SKU-JUGO-1L",
                    "7501000000172",
                    "Jugo naranja 1 L",
                    "Bebidas",
                    "pieza",
                    32.0,
                    24.0,
                    16.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-SUERO-625",
                    "7501000000189",
                    "Suero oral 625 ml",
                    "Bebidas",
                    "pieza",
                    24.0,
                    17.0,
                    18.0,
                    0.0,
                    0.0,
                ),
                (
                    "SKU-CAFE-100",
                    "7501000000196",
                    "Cafe soluble 100 g",
                    "Abarrotes",
                    "pieza",
                    76.0,
                    60.0,
                    8.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-CHOCOLATE",
                    "7501000000202",
                    "Chocolate en polvo 400 g",
                    "Abarrotes",
                    "pieza",
                    48.0,
                    36.0,
                    12.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-LECHE-EVAP",
                    "7501000000219",
                    "Leche evaporada lata",
                    "Lacteos",
                    "pieza",
                    24.0,
                    18.0,
                    24.0,
                    0.0,
                    0.0,
                ),
                (
                    "SKU-CREMA-450",
                    "7501000000226",
                    "Crema acida 450 g",
                    "Lacteos",
                    "pieza",
                    42.0,
                    33.0,
                    10.0,
                    0.0,
                    0.0,
                ),
                (
                    "SKU-QUESO-200",
                    "7501000000233",
                    "Queso fresco 200 g",
                    "Lacteos",
                    "pieza",
                    44.0,
                    35.0,
                    10.0,
                    0.0,
                    0.0,
                ),
                (
                    "SKU-JAMON-250",
                    "7501000000240",
                    "Jamon de pavo 250 g",
                    "Carnes frias",
                    "pieza",
                    52.0,
                    42.0,
                    8.0,
                    0.0,
                    0.0,
                ),
                (
                    "SKU-SALCHICHA",
                    "7501000000257",
                    "Salchicha paquete 500 g",
                    "Carnes frias",
                    "pieza",
                    46.0,
                    36.0,
                    10.0,
                    0.0,
                    0.0,
                ),
                (
                    "SKU-PAPEL-4",
                    "7501000000264",
                    "Papel higienico 4 rollos",
                    "Higiene",
                    "pieza",
                    44.0,
                    34.0,
                    18.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-SERVILLETAS",
                    "7501000000271",
                    "Servilletas paquete",
                    "Higiene",
                    "pieza",
                    22.0,
                    15.0,
                    20.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-PASTA-DENTAL",
                    "7501000000288",
                    "Pasta dental 100 ml",
                    "Higiene",
                    "pieza",
                    36.0,
                    27.0,
                    14.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-SHAMPOO",
                    "7501000000295",
                    "Shampoo familiar 750 ml",
                    "Higiene",
                    "pieza",
                    68.0,
                    52.0,
                    8.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-DETERGENTE-1K",
                    "7501000000301",
                    "Detergente polvo 1 kg",
                    "Limpieza",
                    "pieza",
                    46.0,
                    34.0,
                    18.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-CLORO-1L",
                    "7501000000318",
                    "Cloro 1 L",
                    "Limpieza",
                    "pieza",
                    18.0,
                    11.0,
                    24.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-SUAVITEL-850",
                    "7501000000325",
                    "Suavizante 850 ml",
                    "Limpieza",
                    "pieza",
                    28.0,
                    20.0,
                    18.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-FIBRA",
                    "7501000000332",
                    "Fibra esponja multiusos",
                    "Limpieza",
                    "pieza",
                    14.0,
                    8.0,
                    24.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-PAPAS-45",
                    "7501000000349",
                    "Papas sal 45 g",
                    "Botanas",
                    "pieza",
                    17.0,
                    11.0,
                    30.0,
                    0.0,
                    0.24,
                ),
                (
                    "SKU-CACAHUATE",
                    "7501000000356",
                    "Cacahuate japones 100 g",
                    "Botanas",
                    "pieza",
                    18.0,
                    12.0,
                    24.0,
                    0.0,
                    0.24,
                ),
                (
                    "SKU-CHICHARRON",
                    "7501000000363",
                    "Chicharron harina bolsa",
                    "Botanas",
                    "pieza",
                    15.0,
                    9.0,
                    30.0,
                    0.0,
                    0.24,
                ),
                (
                    "SKU-CHICLE",
                    "7501000000370",
                    "Chicle paquete",
                    "Dulces",
                    "pieza",
                    12.0,
                    8.0,
                    30.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-CHOCOLATE-BARRA",
                    "7501000000387",
                    "Chocolate barra",
                    "Dulces",
                    "pieza",
                    16.0,
                    11.0,
                    24.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-PALETA",
                    "7501000000394",
                    "Paleta caramelo",
                    "Dulces",
                    "pieza",
                    5.0,
                    3.0,
                    60.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-PILA-AA",
                    "7501000000400",
                    "Pilas AA paquete",
                    "Varios",
                    "pieza",
                    38.0,
                    27.0,
                    12.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-VELA",
                    "7501000000417",
                    "Vela blanca pieza",
                    "Varios",
                    "pieza",
                    12.0,
                    7.0,
                    18.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-FOCO",
                    "7501000000424",
                    "Foco led 9w",
                    "Varios",
                    "pieza",
                    32.0,
                    22.0,
                    12.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-CIGARRO-SUELTO",
                    "7501000000431",
                    "Cigarro suelto",
                    "Tabaco",
                    "pieza",
                    8.0,
                    6.0,
                    100.0,
                    0.0,
                    0.16,
                ),
                (
                    "SKU-ENCENDEDOR",
                    "7501000000448",
                    "Encendedor",
                    "Tabaco",
                    "pieza",
                    18.0,
                    10.0,
                    18.0,
                    0.0,
                    0.16,
                ),
            ];
            for product in products {
                conn.execute(
                "INSERT INTO products
                (sku, barcode, name, category, unit, price, wholesale_price, cost, stock, min_stock, tax_rate, active, search_text, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, ?9, ?10, 1, ?11, ?12, ?12)",
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
                    product_search_text(product.0, product.1, product.2, product.3, product.4),
                    now_iso()
                ],
            )
            .map_err(|error| error.to_string())?;
            }
        }
    }
    Ok(())
}

