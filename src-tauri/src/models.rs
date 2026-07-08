use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub(crate) struct Product {
    pub(crate) id: i64,
    pub(crate) sku: String,
    pub(crate) barcode: String,
    pub(crate) name: String,
    pub(crate) category: String,
    pub(crate) unit: String,
    pub(crate) price: f64,
    pub(crate) wholesale_price: Option<f64>,
    pub(crate) cost: f64,
    pub(crate) stock: f64,
    pub(crate) min_stock: f64,
    pub(crate) tax_rate: f64,
    pub(crate) tax_ids: Vec<i64>,
    pub(crate) active: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProductInput {
    pub(crate) id: Option<i64>,
    pub(crate) sku: String,
    pub(crate) barcode: String,
    pub(crate) name: String,
    pub(crate) category: String,
    pub(crate) unit: String,
    pub(crate) price: f64,
    pub(crate) wholesale_price: Option<f64>,
    pub(crate) cost: f64,
    pub(crate) stock: f64,
    pub(crate) min_stock: f64,
    pub(crate) tax_rate: f64,
    #[serde(default)]
    pub(crate) tax_ids: Vec<i64>,
    pub(crate) active: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProductImportRow {
    pub(crate) row_number: i64,
    pub(crate) sku: String,
    pub(crate) barcode: String,
    pub(crate) name: String,
    pub(crate) category: String,
    pub(crate) unit: String,
    pub(crate) price: f64,
    pub(crate) wholesale_price: Option<f64>,
    pub(crate) cost: f64,
    pub(crate) stock: f64,
    pub(crate) min_stock: f64,
    pub(crate) tax_rate: f64,
    #[serde(default)]
    pub(crate) tax_ids: Vec<i64>,
    pub(crate) active: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct ProductImportIssue {
    pub(crate) row_number: i64,
    pub(crate) sku: String,
    pub(crate) barcode: String,
    pub(crate) message: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ProductImportResult {
    pub(crate) imported: i64,
    pub(crate) created: i64,
    pub(crate) updated: i64,
    pub(crate) failed: i64,
    pub(crate) committed: bool,
    pub(crate) issues: Vec<ProductImportIssue>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct InventoryAdjustmentInput {
    pub(crate) product_id: i64,
    pub(crate) quantity: f64,
    pub(crate) reason: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct InventoryMovement {
    pub(crate) id: i64,
    pub(crate) product_id: i64,
    pub(crate) product_name: String,
    pub(crate) movement_type: String,
    pub(crate) quantity: f64,
    pub(crate) reason: String,
    pub(crate) reference_id: Option<i64>,
    pub(crate) created_at: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaleItemInput {
    pub(crate) product_id: i64,
    pub(crate) quantity: f64,
    pub(crate) unit_price: f64,
    pub(crate) discount: f64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PaymentInput {
    pub(crate) method: String,
    pub(crate) amount: f64,
    pub(crate) reference: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaleDraft {
    pub(crate) cashier_id: i64,
    pub(crate) customer_id: Option<i64>,
    pub(crate) items: Vec<SaleItemInput>,
    pub(crate) payments: Vec<PaymentInput>,
    pub(crate) notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct HeldTicketItem {
    pub(crate) product_id: i64,
    pub(crate) quantity: f64,
    pub(crate) unit_price: f64,
    pub(crate) discount: f64,
    #[serde(default)]
    pub(crate) tax_rate: f64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct HeldTicketInput {
    pub(crate) id: Option<i64>,
    pub(crate) name: String,
    pub(crate) cashier_id: i64,
    pub(crate) items: Vec<HeldTicketItem>,
}

#[derive(Debug, Serialize)]
pub(crate) struct HeldTicket {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) cashier_id: i64,
    pub(crate) cashier_name: String,
    pub(crate) item_count: i64,
    pub(crate) total: f64,
    pub(crate) items: Vec<HeldTicketItem>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ActiveSaleDraftInput {
    pub(crate) cashier_id: i64,
    pub(crate) cash_session_id: Option<i64>,
    pub(crate) items: Vec<HeldTicketItem>,
    pub(crate) cash_received: f64,
    pub(crate) card_received: f64,
    pub(crate) transfer_received: f64,
}

#[derive(Debug, Serialize)]
pub(crate) struct ActiveSaleDraft {
    pub(crate) cashier_id: i64,
    pub(crate) cash_session_id: Option<i64>,
    pub(crate) item_count: i64,
    pub(crate) total: f64,
    pub(crate) cash_received: f64,
    pub(crate) card_received: f64,
    pub(crate) transfer_received: f64,
    pub(crate) items: Vec<HeldTicketItem>,
    pub(crate) updated_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct SaleReceipt {
    pub(crate) sale_id: i64,
    pub(crate) folio: String,
    pub(crate) subtotal: f64,
    pub(crate) tax: f64,
    pub(crate) discount: f64,
    pub(crate) total: f64,
    pub(crate) paid: f64,
    pub(crate) change_due: f64,
    pub(crate) created_at: String,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct ShiftCutSnapshot {
    pub(crate) shift_id: i64,
    pub(crate) cash_session_id: i64,
    pub(crate) workstation_id: Option<String>,
    pub(crate) status: String,
    pub(crate) opened_at: String,
    pub(crate) closed_at: Option<String>,
    pub(crate) duration_minutes: i64,
    pub(crate) opened_by_name: Option<String>,
    pub(crate) closed_by_name: Option<String>,
    pub(crate) total_tickets: i64,
    pub(crate) canceled_tickets: i64,
    pub(crate) gross_sales: f64,
    pub(crate) net_sales: f64,
    pub(crate) gross_profit: f64,
    pub(crate) tax: f64,
    pub(crate) discount: f64,
    pub(crate) cash_paid: f64,
    pub(crate) card_paid: f64,
    pub(crate) transfer_paid: f64,
    pub(crate) credit_sales: f64,
    pub(crate) cash_entries_total: f64,
    pub(crate) cash_out_total: f64,
    pub(crate) cash_refunds_total: f64,
    pub(crate) credit_payments_total: f64,
    pub(crate) counted_income_total: f64,
    pub(crate) average_ticket: f64,
    pub(crate) opening_cash: f64,
    pub(crate) expected_cash: f64,
    pub(crate) closing_cash: Option<f64>,
    pub(crate) counted_cash: Option<f64>,
    pub(crate) cash_difference: Option<f64>,
    pub(crate) difference_reason: Option<String>,
    pub(crate) payment_breakdown: Vec<CutPaymentSummary>,
    pub(crate) departments: Vec<CutDepartmentSummary>,
    pub(crate) cash_movements: Vec<CutCashMovementSummary>,
    pub(crate) refunds: Vec<CutRefundSummary>,
    pub(crate) credit_payments: Vec<CutCreditPaymentSummary>,
    pub(crate) taxes: Vec<CutTaxSummary>,
    pub(crate) top_customers_by_sales: Vec<CutCustomerSummary>,
    pub(crate) top_customers_by_profit: Vec<CutCustomerSummary>,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct DailyCutSummary {
    pub(crate) date: String,
    pub(crate) cut_count: i64,
    pub(crate) total_tickets: i64,
    pub(crate) canceled_tickets: i64,
    pub(crate) gross_sales: f64,
    pub(crate) net_sales: f64,
    pub(crate) gross_profit: f64,
    pub(crate) tax: f64,
    pub(crate) discount: f64,
    pub(crate) cash_paid: f64,
    pub(crate) card_paid: f64,
    pub(crate) transfer_paid: f64,
    pub(crate) credit_sales: f64,
    pub(crate) cash_entries_total: f64,
    pub(crate) cash_out_total: f64,
    pub(crate) cash_refunds_total: f64,
    pub(crate) credit_payments_total: f64,
    pub(crate) counted_income_total: f64,
    pub(crate) average_ticket: f64,
    pub(crate) opening_cash: f64,
    pub(crate) expected_cash: f64,
    pub(crate) counted_cash: f64,
    pub(crate) cash_difference: f64,
    pub(crate) payment_breakdown: Vec<CutPaymentSummary>,
    pub(crate) departments: Vec<CutDepartmentSummary>,
    pub(crate) refunds: Vec<CutRefundSummary>,
    pub(crate) credit_payments: Vec<CutCreditPaymentSummary>,
    pub(crate) taxes: Vec<CutTaxSummary>,
    pub(crate) top_customers_by_sales: Vec<CutCustomerSummary>,
    pub(crate) top_customers_by_profit: Vec<CutCustomerSummary>,
    pub(crate) cuts: Vec<ShiftCutSnapshot>,
}

#[derive(Debug, Serialize)]
pub(crate) struct MonthlySalesReport {
    pub(crate) month: String,
    pub(crate) total_tickets: i64,
    pub(crate) total_amount: f64,
    pub(crate) average_ticket: f64,
    pub(crate) canceled_tickets: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct SaleLineHistory {
    pub(crate) sale_id: i64,
    pub(crate) sale_item_id: i64,
    pub(crate) folio: String,
    pub(crate) created_at: String,
    pub(crate) cashier_id: i64,
    pub(crate) cashier_name: String,
    pub(crate) payment_method: String,
    pub(crate) product_id: i64,
    pub(crate) product_name: String,
    pub(crate) quantity: f64,
    pub(crate) returned_quantity: f64,
    pub(crate) unit: String,
    pub(crate) unit_price: f64,
    pub(crate) discount: f64,
    pub(crate) line_total: f64,
    // true solo si la venta esta pagada y su turno sigue abierto (devolucion directa).
    pub(crate) returnable: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct SaleListItem {
    pub(crate) id: i64,
    pub(crate) folio: String,
    pub(crate) cashier_name: String,
    pub(crate) total: f64,
    pub(crate) paid: f64,
    pub(crate) cash_paid: f64,
    pub(crate) card_paid: f64,
    pub(crate) transfer_paid: f64,
    pub(crate) status: String,
    pub(crate) created_at: String,
    // true solo si la venta esta pagada y su turno sigue abierto (cancelable directo).
    pub(crate) cancelable: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct SaleTicketItem {
    pub(crate) product_name: String,
    pub(crate) unit: String,
    pub(crate) quantity: f64,
    pub(crate) returned_quantity: f64,
    pub(crate) unit_price: f64,
    pub(crate) discount: f64,
    pub(crate) line_total: f64,
}

#[derive(Debug, Serialize)]
pub(crate) struct SaleTicketPayment {
    pub(crate) method: String,
    pub(crate) amount: f64,
    pub(crate) reference: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SaleTicketDetail {
    pub(crate) sale_id: i64,
    pub(crate) folio: String,
    pub(crate) status: String,
    pub(crate) cashier_name: String,
    pub(crate) created_at: String,
    pub(crate) subtotal: f64,
    pub(crate) tax: f64,
    pub(crate) discount: f64,
    pub(crate) rounding: f64,
    pub(crate) total: f64,
    pub(crate) paid: f64,
    pub(crate) change_due: f64,
    pub(crate) items: Vec<SaleTicketItem>,
    pub(crate) payments: Vec<SaleTicketPayment>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CashSession {
    pub(crate) id: i64,
    pub(crate) opened_by: i64,
    pub(crate) opened_at: String,
    pub(crate) closed_at: Option<String>,
    pub(crate) opening_cash: f64,
    pub(crate) closing_cash: Option<f64>,
    pub(crate) expected_cash: f64,
    pub(crate) sales_total: f64,
    pub(crate) status: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CashMovementInput {
    pub(crate) session_id: i64,
    pub(crate) movement_type: String,
    pub(crate) amount: f64,
    pub(crate) reason: String,
    pub(crate) actor_id: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct CashMovement {
    pub(crate) id: i64,
    pub(crate) session_id: i64,
    pub(crate) movement_type: String,
    pub(crate) amount: f64,
    pub(crate) reason: String,
    pub(crate) actor_name: String,
    pub(crate) created_at: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CashCountInput {
    pub(crate) session_id: i64,
    pub(crate) shift_id: Option<i64>,
    pub(crate) count_type: String,
    pub(crate) expected_cash: f64,
    pub(crate) counted_cash: f64,
    pub(crate) denominations_json: String,
    pub(crate) difference_reason: Option<String>,
    pub(crate) actor_id: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct CashCount {
    pub(crate) id: i64,
    pub(crate) session_id: i64,
    pub(crate) shift_id: Option<i64>,
    pub(crate) count_type: String,
    pub(crate) expected_cash: f64,
    pub(crate) counted_cash: f64,
    pub(crate) difference: f64,
    pub(crate) denominations_json: String,
    pub(crate) difference_reason: Option<String>,
    pub(crate) actor_name: String,
    pub(crate) created_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct AuditLogEntry {
    pub(crate) id: i64,
    pub(crate) actor_name: Option<String>,
    pub(crate) action: String,
    pub(crate) entity: String,
    pub(crate) entity_id: Option<i64>,
    pub(crate) details: Option<String>,
    pub(crate) created_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct BackupFile {
    pub(crate) path: String,
    pub(crate) name: String,
    pub(crate) size_bytes: u64,
    pub(crate) created_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct BackupRestoreResult {
    pub(crate) restored_path: String,
    pub(crate) safety_backup_path: String,
    pub(crate) restored_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct HardwareResult {
    pub(crate) ok: bool,
    pub(crate) message: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ScaleReading {
    pub(crate) ok: bool,
    pub(crate) weight: f64,
    pub(crate) unit: String,
    pub(crate) source: String,
    pub(crate) baud_rate: u32,
}

#[derive(Debug, Serialize)]
pub(crate) struct DashboardSummary {
    pub(crate) active_products: i64,
    pub(crate) low_stock_products: i64,
    pub(crate) today_sales: f64,
    pub(crate) today_tickets: i64,
    pub(crate) open_cash_session: Option<CashSession>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AppBootstrap {
    pub(crate) summary: DashboardSummary,
    pub(crate) products: Vec<Product>,
    pub(crate) held_tickets: Vec<HeldTicket>,
    pub(crate) tax_enabled: bool,
    pub(crate) tax_prices_include_tax: bool,
    pub(crate) total_round_up: bool,
    pub(crate) unclean_shutdown: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct ReportSummary {
    pub(crate) today_sales: f64,
    pub(crate) today_tickets: i64,
    pub(crate) average_ticket: f64,
    pub(crate) gross_profit: f64,
    pub(crate) cash_expected: f64,
    pub(crate) cash_sales: f64,
    pub(crate) card_sales: f64,
    pub(crate) transfer_sales: f64,
    pub(crate) low_stock_products: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct ProductSalesReport {
    pub(crate) product_id: i64,
    pub(crate) product_name: String,
    pub(crate) category: String,
    pub(crate) quantity: f64,
    pub(crate) total: f64,
    pub(crate) gross_profit: f64,
}

#[derive(Debug, Serialize)]
pub(crate) struct TaxBreakdown {
    pub(crate) tax_rate: f64,
    pub(crate) taxable_sales: f64,
    pub(crate) tax_collected: f64,
    pub(crate) gross_sales: f64,
}

#[derive(Debug, Serialize)]
pub(crate) struct ReportMovement {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) title: String,
    pub(crate) detail: String,
    pub(crate) amount: f64,
    pub(crate) gross_profit: f64,
    pub(crate) cash_paid: f64,
    pub(crate) card_paid: f64,
    pub(crate) transfer_paid: f64,
    pub(crate) tax_total: f64,
    pub(crate) card_terminal: Option<String>,
    pub(crate) actor_name: Option<String>,
    pub(crate) cash_session_id: Option<i64>,
    pub(crate) created_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct UserAccount {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) role: String,
    pub(crate) active: bool,
    pub(crate) created_at: String,
    pub(crate) permissions: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CashierOption {
    pub(crate) id: i64,
    pub(crate) name: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LoginInput {
    pub(crate) name: String,
    pub(crate) pin: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct InitialAdminInput {
    pub(crate) name: String,
    pub(crate) pin: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UserCreateInput {
    pub(crate) name: String,
    pub(crate) pin: String,
    pub(crate) role: String,
    pub(crate) active: bool,
    pub(crate) permissions: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UserUpdateInput {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) pin: Option<String>,
    pub(crate) role: String,
    pub(crate) active: bool,
    pub(crate) permissions: Vec<String>,
}

pub(crate) const USER_PERMISSION_KEYS: &[&str] = &[
    "products",
    "inventory",
    "customers",
    "reports",
    "purchases",
    "invoices",
    "view_profit",
    // Elevates a trusted cashier to admin-level access (grants every permission
    // and admin-only actions) without changing their role.
    "admin",
];

#[derive(Debug, Deserialize)]
pub(crate) struct CustomerInput {
    pub(crate) id: Option<i64>,
    pub(crate) name: String,
    pub(crate) rfc: Option<String>,
    pub(crate) phone: Option<String>,
    pub(crate) email: Option<String>,
    pub(crate) credit_limit: f64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CustomerCreditInput {
    pub(crate) customer_id: i64,
    pub(crate) amount: f64,
    pub(crate) reason: String,
    pub(crate) payment_method: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct CutPaymentSummary {
    pub(crate) method: String,
    pub(crate) label: String,
    pub(crate) amount: f64,
    pub(crate) counts_as_cash: bool,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct CutDepartmentSummary {
    pub(crate) category: String,
    pub(crate) quantity: f64,
    pub(crate) total_sales: f64,
    pub(crate) gross_profit: f64,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct CutCashMovementSummary {
    pub(crate) id: i64,
    pub(crate) movement_type: String,
    pub(crate) amount: f64,
    pub(crate) reason: String,
    pub(crate) actor_name: String,
    pub(crate) created_at: String,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct CutRefundSummary {
    pub(crate) sale_id: i64,
    pub(crate) folio: String,
    pub(crate) amount: f64,
    pub(crate) cash_amount: f64,
    pub(crate) reason: String,
    pub(crate) created_at: String,
    pub(crate) products: String,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct CutCreditPaymentSummary {
    pub(crate) id: i64,
    pub(crate) customer_name: String,
    pub(crate) payment_method: String,
    pub(crate) amount: f64,
    pub(crate) reason: String,
    pub(crate) created_at: String,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct CutTaxSummary {
    pub(crate) tax_name: String,
    pub(crate) tax_type: String,
    pub(crate) rate: f64,
    pub(crate) taxable_sales: f64,
    pub(crate) tax_collected: f64,
    pub(crate) gross_sales: f64,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct CutCustomerSummary {
    pub(crate) customer_id: i64,
    pub(crate) customer_name: String,
    pub(crate) total_sales: f64,
    pub(crate) gross_profit: f64,
    pub(crate) ticket_count: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct Customer {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) rfc: Option<String>,
    pub(crate) phone: Option<String>,
    pub(crate) email: Option<String>,
    pub(crate) credit_limit: f64,
    pub(crate) balance: f64,
    pub(crate) created_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct Supplier {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) phone: Option<String>,
    pub(crate) contact: Option<String>,
    pub(crate) created_at: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SupplierInput {
    pub(crate) id: Option<i64>,
    pub(crate) name: String,
    pub(crate) phone: Option<String>,
    pub(crate) contact: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct TaxOption {
    pub(crate) id: i64,
    pub(crate) name: String,
    #[serde(rename = "type")]
    pub(crate) tax_type: String,
    pub(crate) rate: f64,
    pub(crate) country: String,
    pub(crate) is_active: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct InvoiceDraft {
    pub(crate) id: i64,
    pub(crate) sale_id: Option<i64>,
    pub(crate) customer_id: Option<i64>,
    pub(crate) customer_name: Option<String>,
    pub(crate) folio: String,
    pub(crate) status: String,
    pub(crate) total: f64,
    pub(crate) pac_message: String,
    pub(crate) created_at: String,
}
