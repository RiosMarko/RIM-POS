export type Product = {
  id: number;
  sku: string;
  barcode: string;
  name: string;
  description?: string | null;
  category: string;
  unit: string;
  price: number;
  wholesale_price?: number | null;
  cost: number;
  stock: number;
  min_stock: number;
  tax_rate: number;
  tax_ids: number[];
  sat_product_key?: string | null;
  sat_unit_key?: string | null;
  active: boolean;
};

export type ProductInput = Omit<Product, "id"> & { id?: number };

export type ProductImportRow = Omit<ProductInput, "id"> & {
  row_number: number;
};

export type ProductImportIssue = {
  row_number: number;
  sku: string;
  barcode: string;
  message: string;
};

export type ProductImportResult = {
  imported: number;
  created: number;
  updated: number;
  failed: number;
  committed: boolean;
  issues: ProductImportIssue[];
};

export type CartLine = {
  product: Product;
  quantity: number;
  discount: number;
};

export type Payment = {
  method: "cash" | "card" | "transfer" | "voucher";
  amount: number;
  reference?: string;
};

export type HeldTicketItem = {
  product_id: number;
  quantity: number;
  unit_price: number;
  discount: number;
  tax_rate: number;
};

export type HeldTicket = {
  id: number;
  name: string;
  cashier_id: number;
  cashier_name: string;
  item_count: number;
  total: number;
  items: HeldTicketItem[];
  created_at: string;
  updated_at: string;
};

export type ActiveSaleDraft = {
  cashier_id: number;
  cash_session_id?: number | null;
  item_count: number;
  total: number;
  cash_received: number;
  card_received: number;
  transfer_received: number;
  items: HeldTicketItem[];
  updated_at: string;
};

export type SaleReceipt = {
  sale_id: number;
  folio: string;
  subtotal: number;
  tax: number;
  discount: number;
  total: number;
  paid: number;
  change_due: number;
  created_at: string;
};

export type SaleListItem = {
  id: number;
  folio: string;
  monthly_seq?: number;
  cashier_name: string;
  total: number;
  paid: number;
  cash_paid?: number;
  card_paid?: number;
  transfer_paid?: number;
  status: string;
  created_at: string;
};

export type ShiftCutSnapshot = {
  shift_id: number;
  cash_session_id: number;
  status: string;
  opened_at: string;
  closed_at?: string | null;
  opened_by_name?: string | null;
  closed_by_name?: string | null;
  total_tickets: number;
  canceled_tickets: number;
  gross_sales: number;
  net_sales: number;
  tax: number;
  discount: number;
  cash_paid: number;
  card_paid: number;
  transfer_paid: number;
  average_ticket: number;
  opening_cash: number;
  expected_cash: number;
  closing_cash?: number | null;
  counted_cash?: number | null;
  cash_difference?: number | null;
  difference_reason?: string | null;
};

export type DailyCutSummary = {
  date: string;
  cut_count: number;
  total_tickets: number;
  canceled_tickets: number;
  gross_sales: number;
  net_sales: number;
  tax: number;
  discount: number;
  cash_paid: number;
  card_paid: number;
  transfer_paid: number;
  average_ticket: number;
  opening_cash: number;
  expected_cash: number;
  counted_cash: number;
  cash_difference: number;
  cuts: ShiftCutSnapshot[];
};

export type MonthlySalesReport = {
  month: string;
  total_tickets: number;
  total_amount: number;
  average_ticket: number;
  canceled_tickets: number;
};

export type CashSession = {
  id: number;
  opened_by: number;
  opened_at: string;
  closed_at?: string | null;
  opening_cash: number;
  closing_cash?: number | null;
  expected_cash: number;
  sales_total: number;
  status: string;
};

export type CashMovement = {
  id: number;
  session_id: number;
  movement_type: "in" | "out" | "drawer";
  amount: number;
  reason: string;
  actor_name: string;
  created_at: string;
};

export type CashCount = {
  id: number;
  session_id: number;
  shift_id?: number | null;
  count_type: "audit" | "close";
  expected_cash: number;
  counted_cash: number;
  difference: number;
  denominations_json: string;
  difference_reason?: string | null;
  actor_name: string;
  created_at: string;
};

export type AuditLogEntry = {
  id: number;
  actor_name?: string | null;
  action: string;
  entity: string;
  entity_id?: number | null;
  details?: string | null;
  created_at: string;
};

export type DashboardSummary = {
  active_products: number;
  low_stock_products: number;
  today_sales: number;
  today_tickets: number;
  open_cash_session?: CashSession | null;
};

export type AppBootstrap = {
  summary: DashboardSummary;
  products: Product[];
  held_tickets: HeldTicket[];
  tax_enabled: boolean;
  tax_prices_include_tax: boolean;
  unclean_shutdown: boolean;
};

export type InventoryMovement = {
  id: number;
  product_id: number;
  product_name: string;
  movement_type: string;
  quantity: number;
  reason: string;
  reference_id?: number | null;
  created_at: string;
};

export type Customer = {
  id: number;
  name: string;
  rfc?: string | null;
  phone?: string | null;
  email?: string | null;
  credit_limit: number;
  balance: number;
  created_at: string;
};

export type CustomerInput = Omit<Customer, "id" | "balance" | "created_at"> & { id?: number };

export type ReportSummary = {
  today_sales: number;
  today_tickets: number;
  average_ticket: number;
  gross_profit: number;
  cash_expected: number;
  cash_sales?: number;
  card_sales?: number;
  transfer_sales?: number;
  low_stock_products: number;
};

export type ProductSalesReport = {
  product_id: number;
  product_name: string;
  category: string;
  quantity: number;
  total: number;
  gross_profit: number;
};

export type TaxBreakdown = {
  tax_rate: number;
  taxable_sales: number;
  tax_collected: number;
  gross_sales: number;
};

export type ReportMovement = {
  id: string;
  kind: "sale" | "purchase" | "cash" | "inventory" | "credit" | "cut";
  title: string;
  detail: string;
  amount: number;
  gross_profit?: number;
  cash_paid?: number;
  card_paid?: number;
  transfer_paid?: number;
  tax_total?: number;
  card_terminal?: string | null;
  actor_name?: string | null;
  cash_session_id?: number | null;
  created_at: string;
};

export type BackupResult = {
  path: string;
  created_at: string;
};

export type BackupFile = {
  path: string;
  name: string;
  size_bytes: number;
  created_at: string;
};

export type BackupRestoreResult = {
  restored_path: string;
  safety_backup_path: string;
  restored_at: string;
};

export type HardwareResult = {
  ok: boolean;
  message: string;
};

export type HardwareDeviceType = "printer" | "serial" | "scale" | "cash_drawer";

export type HardwareDevice = {
  id: string;
  name: string;
  device_type: HardwareDeviceType;
  connection: string;
  detail: string;
  is_default: boolean;
};

export type ScaleReading = {
  ok: boolean;
  weight: number;
  unit: string;
  source: string;
  baud_rate: number;
};

export type Supplier = {
  id: number;
  name: string;
  phone?: string | null;
  contact?: string | null;
  created_at: string;
};

export type SupplierInput = {
  id?: number;
  name: string;
  phone?: string | null;
  contact?: string | null;
};

export type PurchaseInput = {
  supplier_id?: number | null;
  product_id: number;
  quantity: number;
  unit_cost: number;
  user_id: number;
  note?: string | null;
};

export type PurchaseReceipt = {
  id: number;
  supplier_name?: string | null;
  product_name: string;
  quantity: number;
  unit_cost: number;
  total: number;
  created_at: string;
};

export type TaxOption = {
  id: number;
  name: string;
  type: "IVA" | "IEPS" | "RETENCION";
  rate: number;
  country: string;
  is_active: boolean;
};

export type InvoiceDraft = {
  id: number;
  sale_id?: number | null;
  customer_id?: number | null;
  customer_name?: string | null;
  folio: string;
  status: string;
  total: number;
  pac_message: string;
  created_at: string;
};

export type UserRole = "admin" | "cashier";

export type PermissionKey =
  | "products"
  | "inventory"
  | "customers"
  | "reports"
  | "purchases"
  | "invoices";

export type UserSession = {
  id: number;
  name: string;
  role: UserRole;
  permissions: PermissionKey[];
};

export type UserAccount = {
  id: number;
  name: string;
  role: UserRole;
  active: boolean;
  created_at: string;
  permissions: PermissionKey[];
};
