use crate::auth::require_active_user;
use crate::backend::{require_permission, AppState, CommandResult};
use crate::core::now_iso;
use crate::models::*;
use crate::validation::{validate_non_negative, validate_required_text};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};
use tauri::State;

pub(crate) fn normalize_catalog_text(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .map(|character| match character {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            other => other,
        })
        .filter(|character| character.is_ascii_alphanumeric() || character.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn normalize_catalog_code(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

pub(crate) fn product_search_text(sku: &str, barcode: &str, name: &str, category: &str, unit: &str) -> String {
    normalize_catalog_text(&format!("{sku} {barcode} {name} {category} {unit}"))
}
pub(crate) fn map_product(row: &rusqlite::Row<'_>) -> rusqlite::Result<Product> {
    Ok(Product {
        id: row.get(0)?,
        sku: row.get(1)?,
        barcode: row.get(2)?,
        name: row.get(3)?,
        category: row.get(4)?,
        unit: row.get(5)?,
        price: row.get(6)?,
        wholesale_price: row.get(7)?,
        cost: row.get(8)?,
        stock: row.get(9)?,
        min_stock: row.get(10)?,
        tax_rate: row.get(11)?,
        tax_ids: Vec::new(),
        active: row.get::<_, i64>(12)? == 1,
    })
}

pub(crate) fn product_tax_ids(conn: &Connection, product_id: i64) -> CommandResult<Vec<i64>> {
    let mut stmt = conn
        .prepare("SELECT tax_id FROM product_taxes WHERE product_id = ?1 ORDER BY tax_id")
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![product_id], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub(crate) fn hydrate_product_taxes(conn: &Connection, product: &mut Product) -> CommandResult<()> {
    product.tax_ids = product_tax_ids(conn, product.id)?;
    Ok(())
}

pub(crate) fn hydrate_products_taxes(conn: &Connection, products: &mut [Product]) -> CommandResult<()> {
    if products.is_empty() {
        return Ok(());
    }
    let mut tax_ids_by_product: HashMap<i64, Vec<i64>> =
        HashMap::with_capacity(products.len());
    for chunk in products.chunks(900) {
        let placeholders = std::iter::repeat("?")
            .take(chunk.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT product_id, tax_id
             FROM product_taxes
             WHERE product_id IN ({placeholders})
             ORDER BY product_id, tax_id"
        );
        let mut stmt = conn.prepare(&sql).map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map(params_from_iter(chunk.iter().map(|product| product.id)), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(|error| error.to_string())?;
        for row in rows {
            let (product_id, tax_id) = row.map_err(|error| error.to_string())?;
            tax_ids_by_product.entry(product_id).or_default().push(tax_id);
        }
    }
    for product in products {
        product.tax_ids = tax_ids_by_product.remove(&product.id).unwrap_or_default();
    }
    Ok(())
}

pub(crate) fn tax_rate_for_ids(conn: &Connection, tax_ids: &[i64], fallback: f64) -> CommandResult<f64> {
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

pub(crate) fn save_product_taxes(conn: &Connection, product_id: i64, tax_ids: &[i64]) -> CommandResult<()> {
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

pub(crate) fn product_import_issue(row: &ProductImportRow, message: impl Into<String>) -> ProductImportIssue {
    ProductImportIssue {
        row_number: row.row_number,
        sku: row.sku.trim().to_string(),
        barcode: row.barcode.trim().to_string(),
        message: message.into(),
    }
}

pub(crate) fn existing_product_id_for_import(
    conn: &Connection,
    sku: &str,
    barcode: &str,
) -> CommandResult<Option<i64>> {
    let sku_id = conn
        .query_row(
            "SELECT id FROM products WHERE lower(sku) = lower(?1)",
            params![sku],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let barcode_id = conn
        .query_row(
            "SELECT id FROM products WHERE barcode = ?1",
            params![barcode],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    match (sku_id, barcode_id) {
        (Some(left), Some(right)) if left != right => {
            Err("SKU y codigo pertenecen a productos distintos".to_string())
        }
        (Some(id), _) | (_, Some(id)) => Ok(Some(id)),
        (None, None) => Ok(None),
    }
}

pub(crate) fn import_products_with_conn(
    conn: &mut Connection,
    rows: Vec<ProductImportRow>,
) -> CommandResult<ProductImportResult> {
    if rows.is_empty() {
        return Ok(ProductImportResult {
            imported: 0,
            created: 0,
            updated: 0,
            failed: 1,
            committed: false,
            issues: vec![ProductImportIssue {
                row_number: 0,
                sku: String::new(),
                barcode: String::new(),
                message: "CSV sin productos".into(),
            }],
        });
    }

    let mut issues = Vec::new();
    let mut seen_barcodes = HashSet::new();
    let mut prepared = Vec::new();

    for row in rows {
        let barcode = row.barcode.trim();
        let sku = barcode;
        let name = row.name.trim();
        let category = row.category.trim();
        let unit = row.unit.trim();
        let _requested_active = row.active;

        if let Err(message) = validate_required_text(barcode, 1, "Codigo requerido") {
            issues.push(product_import_issue(&row, message));
            continue;
        }
        if let Err(message) = validate_required_text(name, 2, "Nombre requerido") {
            issues.push(product_import_issue(&row, message));
            continue;
        }
        if let Err(message) = validate_required_text(category, 2, "Departamento requerido") {
            issues.push(product_import_issue(&row, message));
            continue;
        }
        if let Err(message) = validate_required_text(unit, 1, "Unidad requerida") {
            issues.push(product_import_issue(&row, message));
            continue;
        }
        let mut numeric_error = None;
        for (value, label) in [
            (row.price, "Precio invalido"),
            (row.wholesale_price.unwrap_or(0.0), "Mayoreo invalido"),
            (row.cost, "Costo invalido"),
            (row.stock, "Stock invalido"),
            (row.min_stock, "Minimo invalido"),
            (row.tax_rate, "Impuesto invalido"),
        ] {
            if let Err(message) = validate_non_negative(value, label) {
                numeric_error = Some(message);
                break;
            }
        }
        if let Some(message) = numeric_error {
            issues.push(product_import_issue(&row, message));
            continue;
        }

        if !seen_barcodes.insert(barcode.to_string()) {
            issues.push(product_import_issue(&row, "Codigo duplicado en archivo"));
            continue;
        }

        match existing_product_id_for_import(conn, sku, barcode) {
            Ok(id) => prepared.push(ProductInput {
                id,
                sku: sku.to_string(),
                barcode: barcode.to_string(),
                name: name.to_string(),
                category: category.to_string(),
                unit: unit.to_string(),
                price: row.price,
                wholesale_price: row.wholesale_price,
                cost: row.cost,
                stock: row.stock,
                min_stock: row.min_stock,
                tax_rate: row.tax_rate,
                tax_ids: row.tax_ids,
                active: true,
            }),
            Err(message) => issues.push(product_import_issue(&row, message)),
        }
    }

    if !issues.is_empty() {
        return Ok(ProductImportResult {
            imported: 0,
            created: 0,
            updated: 0,
            failed: issues.len() as i64,
            committed: false,
            issues,
        });
    }

    let tx = conn.transaction().map_err(|error| error.to_string())?;
    let now = now_iso();
    let mut created = 0_i64;
    let mut updated = 0_i64;
    for input in prepared {
        let active = if input.active { 1 } else { 0 };
        let tax_rate = tax_rate_for_ids(&tx, &input.tax_ids, input.tax_rate)?;
        let id = match input.id {
            Some(id) => {
                tx.execute(
                    "UPDATE products
                     SET sku = ?1, barcode = ?2, name = ?3, category = ?4, unit = ?5, price = ?6,
                         wholesale_price = ?7, cost = ?8, stock = ?9, min_stock = ?10, tax_rate = ?11,
                         active = ?12, search_text = ?13, updated_at = ?14
                     WHERE id = ?15",
                    params![
                        input.sku,
                        input.barcode,
                        input.name,
                        input.category,
                        input.unit,
                        input.price,
                        input.wholesale_price,
                        input.cost,
                        input.stock,
                        input.min_stock,
                        tax_rate,
                        active,
                        product_search_text(&input.sku, &input.barcode, &input.name, &input.category, &input.unit),
                        now,
                        id
                    ],
                )
                .map_err(|error| error.to_string())?;
                updated += 1;
                id
            }
            None => {
                tx.execute(
                    "INSERT INTO products
                     (sku, barcode, name, category, unit, price, wholesale_price, cost, stock, min_stock, tax_rate, active, search_text, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)",
                    params![
                        input.sku,
                        input.barcode,
                        input.name,
                        input.category,
                        input.unit,
                        input.price,
                        input.wholesale_price,
                        input.cost,
                        input.stock,
                        input.min_stock,
                        tax_rate,
                        active,
                        product_search_text(&input.sku, &input.barcode, &input.name, &input.category, &input.unit),
                        now
                    ],
                )
                .map_err(|error| error.to_string())?;
                created += 1;
                tx.last_insert_rowid()
            }
        };
        save_product_taxes(&tx, id, &input.tax_ids)?;
    }
    tx.commit().map_err(|error| error.to_string())?;

    Ok(ProductImportResult {
        imported: created + updated,
        created,
        updated,
        failed: 0,
        committed: true,
        issues: Vec::new(),
    })
}

pub(crate) fn map_inventory_movement(row: &rusqlite::Row<'_>) -> rusqlite::Result<InventoryMovement> {
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
pub(crate) fn product_search(
    state: State<'_, AppState>,
    actor_id: i64,
    query: String,
    limit: Option<i64>,
    offset: Option<i64>,
) -> CommandResult<Vec<Product>> {
    let limit = limit.unwrap_or(30).clamp(1, 50_000);
    let offset = offset.unwrap_or(0).max(0);
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, actor_id)?;
    product_search_with_conn(&conn, &query, limit, offset)
}

pub(crate) fn product_search_with_conn(
    conn: &Connection,
    query: &str,
    limit: i64,
    offset: i64,
) -> CommandResult<Vec<Product>> {
    let trimmed = query.trim();
    let normalized = normalize_catalog_text(trimmed);
    let normalized_code = normalize_catalog_code(trimmed);
    if normalized.is_empty() {
        let mut stmt = conn
            .prepare(
                "SELECT id, sku, barcode, name, category, unit, price, wholesale_price, cost, stock, min_stock, tax_rate, active
                 FROM products
                 WHERE active = 1
                 ORDER BY lower(barcode), name
                 LIMIT ?1 OFFSET ?2",
            )
            .map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map(params![limit, offset], map_product)
            .map_err(|error| error.to_string())?;
        let mut products = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        hydrate_products_taxes(conn, &mut products)?;
        return Ok(products);
    }
    let mut exact_stmt = conn
        .prepare(
            "SELECT id, sku, barcode, name, category, unit, price, wholesale_price, cost, stock, min_stock, tax_rate, active
             FROM products
             WHERE active = 1
               AND (barcode = ?1 OR lower(sku) = ?2 OR replace(lower(barcode), ' ', '') = ?2)
             ORDER BY lower(barcode), name
             LIMIT ?3",
        )
        .map_err(|error| error.to_string())?;
    let exact_rows = exact_stmt
        .query_map(params![trimmed, normalized_code, limit], map_product)
        .map_err(|error| error.to_string())?;
    let mut exact_products = exact_rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    if !exact_products.is_empty() {
        hydrate_products_taxes(conn, &mut exact_products)?;
        return Ok(exact_products);
    }
    let like = format!("%{}%", normalized);
    let raw_like = format!("%{}%", trimmed.to_lowercase());
    let mut stmt = conn
        .prepare(
            "SELECT id, sku, barcode, name, category, unit, price, wholesale_price, cost, stock, min_stock, tax_rate, active
             FROM products
             WHERE active = 1
               AND (search_text LIKE ?1
                    OR lower(name) LIKE ?2
                    OR lower(category) LIKE ?2
                    OR lower(sku) LIKE ?2)
             ORDER BY
               CASE
                 WHEN search_text LIKE ?1 THEN 1
                 ELSE 2
               END,
               lower(barcode),
               name
             LIMIT ?3 OFFSET ?4",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(
            params![like, raw_like, limit, offset],
            map_product,
        )
        .map_err(|error| error.to_string())?;
    let mut products = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    hydrate_products_taxes(conn, &mut products)?;
    Ok(products)
}

#[tauri::command]
pub(crate) fn product_get_many(
    state: State<'_, AppState>,
    actor_id: i64,
    ids: Vec<i64>,
) -> CommandResult<Vec<Product>> {
    let mut ids = ids.into_iter().filter(|id| *id > 0).collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_active_user(&conn, actor_id)?;
    let placeholders = std::iter::repeat("?")
        .take(ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT id, sku, barcode, name, category, unit, price, wholesale_price, cost, stock, min_stock, tax_rate, active
         FROM products
         WHERE active = 1 AND id IN ({placeholders})
         ORDER BY name"
    );
    let mut stmt = conn.prepare(&sql).map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params_from_iter(ids.iter()), map_product)
        .map_err(|error| error.to_string())?;
    let mut products = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    hydrate_products_taxes(&conn, &mut products)?;
    Ok(products)
}

#[tauri::command]
pub(crate) fn product_upsert(
    state: State<'_, AppState>,
    actor_id: i64,
    input: ProductInput,
) -> CommandResult<Product> {
    let sku = input.sku.trim();
    let barcode = input.barcode.trim();
    let name = input.name.trim();
    let category = input.category.trim();
    let unit = input.unit.trim();
    validate_required_text(sku, 1, "Producto incompleto")?;
    validate_required_text(barcode, 1, "Producto incompleto")?;
    validate_required_text(name, 2, "Producto incompleto")?;
    validate_non_negative(input.price, "Importe o existencia invalida")?;
    validate_non_negative(input.wholesale_price.unwrap_or(0.0), "Importe o existencia invalida")?;
    validate_non_negative(input.cost, "Importe o existencia invalida")?;
    validate_non_negative(input.stock, "Importe o existencia invalida")?;
    validate_non_negative(input.min_stock, "Importe o existencia invalida")?;
    let mut conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "products")?;
    let now = now_iso();
    let active = if input.active { 1 } else { 0 };
    let tx = conn.transaction().map_err(|error| error.to_string())?;
    let tax_rate = tax_rate_for_ids(&tx, &input.tax_ids, input.tax_rate)?;
    let id = match input.id {
        Some(id) => {
            let before: Option<(f64, f64)> = tx
                .query_row(
                    "SELECT price, stock FROM products WHERE id = ?1",
                    params![id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(|error| error.to_string())?;
            tx.execute(
                "UPDATE products
                 SET sku = ?1, barcode = ?2, name = ?3, category = ?4, unit = ?5, price = ?6,
                     wholesale_price = ?7, cost = ?8, stock = ?9, min_stock = ?10, tax_rate = ?11,
                     active = ?12, search_text = ?13, updated_at = ?14
                 WHERE id = ?15",
                params![
                    sku,
                    barcode,
                    name,
                    category,
                    unit,
                    input.price,
                    input.wholesale_price,
                    input.cost,
                    input.stock,
                    input.min_stock,
                    tax_rate,
                    active,
                    product_search_text(sku, barcode, name, category, unit),
                    now,
                    id
                ],
            )
            .map_err(|error| error.to_string())?;
            if let Some((old_price, old_stock)) = before {
                let mut details = Vec::new();
                if (old_price - input.price).abs() > 0.0001 {
                    details.push(format!("precio {:.2} -> {:.2}", old_price, input.price));
                }
                let stock_delta = input.stock - old_stock;
                if stock_delta.abs() > 0.0001 {
                    details.push(format!("stock {:.3} -> {:.3}", old_stock, input.stock));
                    tx.execute(
                        "INSERT INTO inventory_movements (product_id, movement_type, quantity, reason, reference_id, actor_id, created_at)
                         VALUES (?1, 'edit', ?2, 'Edicion de producto', NULL, ?3, ?4)",
                        params![id, stock_delta, actor_id, now],
                    )
                    .map_err(|error| error.to_string())?;
                }
                if !details.is_empty() {
                    tx.execute(
                        "INSERT INTO audit_log (actor_id, action, entity, entity_id, details, created_at)
                         VALUES (?1, 'product_update', 'product', ?2, ?3, ?4)",
                        params![actor_id, id, details.join(" · "), now],
                    )
                    .map_err(|error| error.to_string())?;
                }
            }
            id
        }
        None => {
            tx.execute(
                "INSERT INTO products
                 (sku, barcode, name, category, unit, price, wholesale_price, cost, stock, min_stock, tax_rate, active, search_text, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)",
                params![
                    sku,
                    barcode,
                    name,
                    category,
                    unit,
                    input.price,
                    input.wholesale_price,
                    input.cost,
                    input.stock,
                    input.min_stock,
                    tax_rate,
                    active,
                    product_search_text(sku, barcode, name, category, unit),
                    now
                ],
            )
            .map_err(|error| error.to_string())?;
            let id = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO audit_log (actor_id, action, entity, entity_id, details, created_at)
                 VALUES (?1, 'product_create', 'product', ?2, ?3, ?4)",
                params![actor_id, id, format!("{name} · precio {:.2} · stock {:.3}", input.price, input.stock), now],
            )
            .map_err(|error| error.to_string())?;
            id
        }
    };
    save_product_taxes(&tx, id, &input.tax_ids)?;
    tx.commit().map_err(|error| error.to_string())?;
    get_product(&conn, id)
}

#[tauri::command]
pub(crate) fn product_bulk_import(
    state: State<'_, AppState>,
    actor_id: i64,
    rows: Vec<ProductImportRow>,
) -> CommandResult<ProductImportResult> {
    let mut conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "products")?;
    import_products_with_conn(&mut conn, rows)
}

#[tauri::command]
pub(crate) fn product_bulk_validate(
    state: State<'_, AppState>,
    actor_id: i64,
    rows: Vec<ProductImportRow>,
) -> CommandResult<ProductImportResult> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "products")?;
    let mut issues = Vec::new();
    let mut seen_barcodes = HashSet::new();
    for row in &rows {
        let barcode = row.barcode.trim();
        if barcode.is_empty() {
            issues.push(product_import_issue(row, "Codigo requerido"));
            continue;
        }
        if !seen_barcodes.insert(barcode.to_string()) {
            issues.push(product_import_issue(row, "Codigo duplicado en archivo"));
            continue;
        }
        match existing_product_id_for_import(&conn, barcode, barcode) {
            Ok(_) => {}
            Err(message) => issues.push(product_import_issue(row, message)),
        }
    }
    Ok(ProductImportResult {
        imported: 0,
        created: rows.len() as i64 - issues.len() as i64,
        updated: 0,
        failed: issues.len() as i64,
        committed: false,
        issues,
    })
}

#[tauri::command]
pub(crate) fn product_delete(state: State<'_, AppState>, actor_id: i64, id: i64) -> CommandResult<()> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    require_permission(&conn, actor_id, "products")?;
    let name: String = conn
        .query_row("SELECT name FROM products WHERE id = ?1", params![id], |row| row.get(0))
        .map_err(|_| "Producto no encontrado".to_string())?;
    conn.execute(
        "UPDATE products SET active = 0, updated_at = ?1 WHERE id = ?2",
        params![now_iso(), id],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT INTO audit_log (actor_id, action, entity, entity_id, details, created_at)
         VALUES (?1, 'product_delete', 'product', ?2, ?3, ?4)",
        params![actor_id, id, name, now_iso()],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn get_product(conn: &Connection, id: i64) -> CommandResult<Product> {
    let mut product = conn.query_row(
        "SELECT id, sku, barcode, name, category, unit, price, wholesale_price, cost, stock, min_stock, tax_rate, active
         FROM products WHERE id = ?1",
        params![id],
        map_product,
    )
    .map_err(|error| error.to_string())?;
    hydrate_product_taxes(conn, &mut product)?;
    Ok(product)
}

#[tauri::command]
pub(crate) fn inventory_adjust(
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
        "INSERT INTO inventory_movements (product_id, movement_type, quantity, reason, reference_id, actor_id, created_at)
         VALUES (?1, 'adjustment', ?2, ?3, NULL, ?4, ?5)",
        params![input.product_id, input.quantity, reason, actor_id, now],
    )
    .map_err(|error| error.to_string())?;
    tx.execute(
        "INSERT INTO audit_log (actor_id, action, entity, entity_id, details, created_at)
         VALUES (?1, 'stock_adjust', 'product', ?2, ?3, ?4)",
        params![
            actor_id,
            input.product_id,
            format!("stock {:.3} -> {:.3} · {}", current_stock, current_stock + input.quantity, reason),
            now
        ],
    )
    .map_err(|error| error.to_string())?;
    tx.commit().map_err(|error| error.to_string())?;
    get_product(&conn, input.product_id)
}

#[tauri::command]
pub(crate) fn inventory_kardex(
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
