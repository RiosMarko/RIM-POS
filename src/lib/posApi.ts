import { invoke } from "@tauri-apps/api/core";
import type {
  ActiveSaleDraft,
  AppBootstrap,
  AuditLogEntry,
  BackupFile,
  BackupRestoreResult,
  BackupResult,
  CashCount,
  CashMovement,
  CashSession,
  Customer,
  CustomerInput,
  DailyCutSummary,
  DashboardSummary,
  HeldTicket,
  HeldTicketItem,
  HardwareDevice,
  HardwareResult,
  InventoryMovement,
  InvoiceDraft,
  MonthlySalesReport,
  Payment,
  Product,
  ProductImportRow,
  ProductImportResult,
  ProductInput,
  ProductSalesReport,
  ReportMovement,
  PurchaseInput,
  PurchaseReceipt,
  ReportSummary,
  SaleListItem,
  SaleReceipt,
  ScaleReading,
  ShiftCutSnapshot,
  Supplier,
  SupplierInput,
  TaxBreakdown,
  TaxOption,
  UserAccount,
  PermissionKey,
  UserRole,
  UserSession,
} from "../types";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

let currentActorId: number | null = null;

export function setApiActor(session: Pick<UserSession, "id"> | null) {
  currentActorId = session?.id ?? null;
}

function requireActorId() {
  if (!currentActorId) throw new Error("Sesion requerida");
  return currentActorId;
}

let demoProducts: Product[] = [
  {
    id: 1,
    sku: "SKU-COCA-600",
    barcode: "7501055300075",
    name: "Refresco cola 600 ml",
    category: "Bebidas",
    unit: "pieza",
    price: 18,
    wholesale_price: null,
    cost: 12,
    stock: 48,
    min_stock: 12,
    tax_rate: 0.16,
    tax_ids: [1],
    active: true,
  },
  {
    id: 2,
    sku: "SKU-TORT-1K",
    barcode: "2000000000017",
    name: "Tortilla de maiz 1 kg",
    category: "Abarrotes",
    unit: "kg",
    price: 24,
    wholesale_price: null,
    cost: 18,
    stock: 30,
    min_stock: 5,
    tax_rate: 0,
    tax_ids: [3],
    active: true,
  },
  {
    id: 3,
    sku: "SKU-HUEVO-30",
    barcode: "2000000000024",
    name: "Huevo cartera 30 pzas",
    category: "Abarrotes",
    unit: "pieza",
    price: 82,
    wholesale_price: null,
    cost: 68,
    stock: 16,
    min_stock: 4,
    tax_rate: 0,
    tax_ids: [3],
    active: true,
  },
  {
    id: 4,
    sku: "SKU-SABR-45",
    barcode: "7501011131156",
    name: "Papas adobadas 45 g",
    category: "Botanas",
    unit: "pieza",
    price: 17,
    wholesale_price: null,
    cost: 11.5,
    stock: 40,
    min_stock: 10,
    tax_rate: 0.24,
    tax_ids: [1, 4],
    active: true,
  },
  {
    id: 5,
    sku: "SKU-LECHE-1L",
    barcode: "7501020513318",
    name: "Leche entera 1 L",
    category: "Lacteos",
    unit: "pieza",
    price: 29.5,
    wholesale_price: null,
    cost: 23,
    stock: 24,
    min_stock: 8,
    tax_rate: 0,
    tax_ids: [3],
    active: true,
  },
  {
    id: 6,
    sku: "SKU-JABON-Z",
    barcode: "7509546041899",
    name: "Jabon zote rosa 400 g",
    category: "Limpieza",
    unit: "pieza",
    price: 21,
    wholesale_price: null,
    cost: 15,
    stock: 20,
    min_stock: 6,
    tax_rate: 0.16,
    tax_ids: [1],
    active: true,
  },
];

let mockSaleId = 1;
let mockUserId = 3;
let mockHeldTicketId = 1;
let mockCustomerId = 1;
let mockInventoryMovementId = 1;
let mockCashMovementId = 1;
let mockCashSession: CashSession | null = {
  id: 1,
  opened_by: 1,
  opened_at: new Date().toISOString(),
  closed_at: null,
  opening_cash: 0,
  closing_cash: null,
  expected_cash: 0,
  sales_total: 0,
  status: "open",
};
const allMockPermissions: PermissionKey[] = ["products", "inventory", "customers", "reports", "purchases", "invoices"];

let mockUsers: UserAccount[] = [
  {
    id: 1,
    name: "Admin",
    role: "admin",
    active: true,
    created_at: new Date().toISOString(),
    permissions: allMockPermissions,
  },
  {
    id: 2,
    name: "Cajera",
    role: "cashier",
    active: true,
    created_at: new Date().toISOString(),
    permissions: [],
  },
];
const mockPins = new Map<string, string>([
  ["admin", "1234"],
  ["cajera", "1111"],
]);
let mockHeldTickets: HeldTicket[] = [];
let mockActiveSaleDrafts: ActiveSaleDraft[] = [];
let mockCustomers: Customer[] = [];
let mockSales: SaleListItem[] = [];
let mockSaleItems = new Map<number, Array<{ product_id: number; quantity: number }>>();
let mockInventoryMovements: InventoryMovement[] = [];
let mockCashMovements: CashMovement[] = [];
let mockCashCounts: CashCount[] = [];
let mockCashCountId = 1;
let mockCustomerCreditMovementId = 1;
let mockCustomerCreditMovements: Array<{
  id: number;
  customer_name: string;
  amount: number;
  reason: string;
  created_at: string;
}> = [];
let mockLastCutZ: ShiftCutSnapshot | null = null;
const mockMonthlySeq = new Map<string, number>();
let mockSuppliers: Supplier[] = [];
let mockSupplierId = 1;
let mockPurchases: PurchaseReceipt[] = [];
let mockPurchaseId = 1;
let mockInvoiceId = 1;
let mockInvoices: InvoiceDraft[] = [];
const mockSettings = new Map<string, string>();
const demoTaxes: TaxOption[] = [
  { id: 1, name: "IVA 16%", type: "IVA", rate: 0.16, country: "MX", is_active: true },
  { id: 2, name: "IVA 8%", type: "IVA", rate: 0.08, country: "MX", is_active: true },
  { id: 3, name: "Exento 0%", type: "IVA", rate: 0, country: "MX", is_active: true },
  { id: 4, name: "IEPS 8%", type: "IEPS", rate: 0.08, country: "MX", is_active: true },
  { id: 5, name: "IEPS 26.5%", type: "IEPS", rate: 0.265, country: "MX", is_active: true },
];

function taxRateFromIds(taxIds: number[]) {
  return demoTaxes
    .filter((tax) => taxIds.includes(tax.id) && tax.is_active)
    .reduce((sum, tax) => sum + tax.rate, 0);
}

function isTauri() {
  const hasTauriRuntime = typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);
  if (!hasTauriRuntime && !import.meta.env.DEV && import.meta.env.MODE !== "test") {
    throw new Error("Runtime Tauri requerido: mock navegador desactivado en produccion");
  }
  return hasTauriRuntime;
}

function mockBoolSetting(key: string, defaultValue: boolean) {
  return (mockSettings.get(key) ?? String(defaultValue)) !== "false";
}

function lineAmounts(base: number, discount: number, taxRate: number) {
  const taxable = Math.max(0, base - discount);
  const taxEnabled = mockBoolSetting("tax_enabled", true);
  const pricesIncludeTax = mockBoolSetting("tax_prices_include_tax", true);
  if (!taxEnabled || taxRate <= 0) return { subtotal: taxable, tax: 0, total: taxable };
  if (pricesIncludeTax) {
    const subtotal = taxable / (1 + taxRate);
    return { subtotal, tax: taxable - subtotal, total: taxable };
  }
  return { subtotal: taxable, tax: taxable * taxRate, total: taxable + taxable * taxRate };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw new Error("Tauri runtime no disponible");
  }
  return invoke<T>(command, args);
}

export type ProductSearchOptions = {
  limit?: number;
  offset?: number;
};

export async function searchProducts(query: string, options: ProductSearchOptions = {}): Promise<Product[]> {
  const limit = options.limit ?? 40;
  const offset = options.offset ?? 0;
  if (isTauri()) {
    return call<Product[]>("product_search", { actorId: requireActorId(), query, limit, offset });
  }
  const normalized = query.trim().toLowerCase();
  const matched = normalized
    ? demoProducts.filter((product) =>
      product.name.toLowerCase().includes(normalized) ||
      product.category.toLowerCase().includes(normalized) ||
      product.sku.toLowerCase().includes(normalized) ||
      product.barcode === normalized,
    )
    : demoProducts;
  return [...matched]
    .sort((left, right) => left.barcode.localeCompare(right.barcode) || left.name.localeCompare(right.name))
    .slice(offset, offset + limit);
}

export async function getProductsByIds(ids: number[]): Promise<Product[]> {
  const uniqueIds = Array.from(new Set(ids.filter((id) => Number.isFinite(id) && id > 0)));
  if (uniqueIds.length === 0) return [];
  if (isTauri()) {
    return call<Product[]>("product_get_many", { actorId: requireActorId(), ids: uniqueIds });
  }
  return demoProducts.filter((product) => uniqueIds.includes(product.id));
}

export async function upsertProduct(input: ProductInput): Promise<Product> {
  if (isTauri()) {
    return call<Product>("product_upsert", { actorId: requireActorId(), input });
  }
  const nowId = input.id ?? Math.max(0, ...demoProducts.map((product) => product.id)) + 1;
  const tax_ids = input.tax_ids ?? [];
  const tax_rate = tax_ids.length ? taxRateFromIds(tax_ids) : input.tax_rate;
  const product: Product = { ...input, tax_ids, tax_rate, id: nowId };
  demoProducts = [product, ...demoProducts.filter((current) => current.id !== nowId)];
  return product;
}

export async function bulkImportProducts(rows: ProductImportRow[]): Promise<ProductImportResult> {
  if (isTauri()) {
    return call<ProductImportResult>("product_bulk_import", { actorId: requireActorId(), rows });
  }
  const issues: ProductImportResult["issues"] = [];
  const seenBarcodes = new Set<string>();
  const prepared = rows.map((row) => {
    const barcode = row.barcode.trim();
    const sku = barcode;
    const name = row.name.trim();
    const category = row.category.trim();
    const unit = row.unit.trim();
    const issue = (message: string) => {
      issues.push({ row_number: row.row_number, sku: "", barcode, message });
    };
    if (barcode.length < 1) issue("Codigo requerido");
    if (name.length < 2) issue("Nombre requerido");
    if (category.length < 2) issue("Departamento requerido");
    if (unit.length < 1) issue("Unidad requerida");
    if ([row.price, row.wholesale_price ?? 0, row.cost, row.stock, row.min_stock, row.tax_rate].some((value) => !Number.isFinite(value) || value < 0)) {
      issue("Importe o existencia invalida");
    }
    if (seenBarcodes.has(barcode)) issue("Codigo duplicado en archivo");
    seenBarcodes.add(barcode);
    return { ...row, sku, barcode, name, category, unit };
  });
  if (issues.length) {
    return { imported: 0, created: 0, updated: 0, failed: issues.length, committed: false, issues };
  }
  let created = 0;
  let updated = 0;
  prepared.forEach((row) => {
    const existing = demoProducts.find(
      (product) => product.barcode === row.barcode,
    );
    const id = existing?.id ?? Math.max(0, ...demoProducts.map((product) => product.id)) + 1;
    if (existing) updated += 1;
    else created += 1;
    const tax_ids = row.tax_ids ?? [];
    const tax_rate = tax_ids.length ? taxRateFromIds(tax_ids) : row.tax_rate;
    const product: Product = { ...row, id, tax_ids, tax_rate };
    demoProducts = [product, ...demoProducts.filter((current) => current.id !== id)];
  });
  return { imported: created + updated, created, updated, failed: 0, committed: true, issues: [] };
}

export async function deleteProduct(id: number): Promise<void> {
  if (isTauri()) {
    return call<void>("product_delete", { actorId: requireActorId(), id });
  }
  demoProducts = demoProducts.map((product) => (product.id === id ? { ...product, active: false } : product));
}

export async function adjustInventory(input: { product_id: number; quantity: number; reason: string }): Promise<Product> {
  if (isTauri()) {
    return call<Product>("inventory_adjust", { actorId: requireActorId(), input });
  }
  const product = demoProducts.find((candidate) => candidate.id === input.product_id);
  if (!product) throw new Error("Producto no disponible");
  if (product.stock + input.quantity < 0) throw new Error("El ajuste deja stock negativo");
  const next = { ...product, stock: product.stock + input.quantity };
  demoProducts = demoProducts.map((candidate) => (candidate.id === next.id ? next : candidate));
  mockInventoryMovements.unshift({
    id: mockInventoryMovementId,
    product_id: next.id,
    product_name: next.name,
    movement_type: "adjustment",
    quantity: input.quantity,
    reason: input.reason,
    reference_id: null,
    created_at: new Date().toISOString(),
  });
  mockInventoryMovementId += 1;
  return next;
}

export async function listInventoryMovements(productId?: number): Promise<InventoryMovement[]> {
  if (isTauri()) {
    return call<InventoryMovement[]>("inventory_kardex", { actorId: requireActorId(), productId, limit: 80 });
  }
  return productId ? mockInventoryMovements.filter((movement) => movement.product_id === productId) : mockInventoryMovements;
}

export async function login(input: { name: string; pin: string }): Promise<UserSession> {
  if (isTauri()) {
    return call<UserSession>("auth_login", { input });
  }
  const user = mockUsers.find((candidate) => candidate.name.toLowerCase() === input.name.trim().toLowerCase() && candidate.active);
  if (!user || mockPins.get(user.name.toLowerCase()) !== input.pin.trim()) {
    throw new Error("Usuario o PIN incorrecto");
  }
  return { id: user.id, name: user.name, role: user.role, permissions: user.permissions };
}

export async function needsInitialSetup(): Promise<boolean> {
  if (isTauri()) {
    return call<boolean>("auth_needs_setup");
  }
  return false;
}

export async function createInitialAdmin(input: { name: string; pin: string }): Promise<UserSession> {
  if (isTauri()) {
    return call<UserSession>("auth_create_initial_admin", { input });
  }
  return createUser({ name: input.name, pin: input.pin, role: "admin", active: true, permissions: allMockPermissions });
}

export async function listUsers(): Promise<UserAccount[]> {
  if (isTauri()) {
    return call<UserAccount[]>("user_list", { actorId: requireActorId() });
  }
  return mockUsers;
}

export async function createUser(input: {
  name: string;
  pin: string;
  role: UserRole;
  active: boolean;
  permissions: PermissionKey[];
}): Promise<UserAccount> {
  if (isTauri()) {
    return call<UserAccount>("user_create", { actorId: requireActorId(), input });
  }
  if (input.name.trim().length < 2) throw new Error("Nombre muy corto");
  if (input.pin.trim().length < 4) throw new Error("PIN minimo de 4 digitos");
  if (mockUsers.some((user) => user.name.toLowerCase() === input.name.trim().toLowerCase())) {
    throw new Error("Ya existe usuario con ese nombre");
  }
  const user: UserAccount = {
    id: mockUserId,
    name: input.name.trim(),
    role: input.role,
    active: input.active,
    created_at: new Date().toISOString(),
    permissions: input.role === "admin" ? allMockPermissions : input.permissions,
  };
  mockUserId += 1;
  mockUsers = [...mockUsers, user];
  mockPins.set(user.name.toLowerCase(), input.pin.trim());
  return user;
}

export async function updateUser(input: {
  id: number;
  name: string;
  pin?: string;
  role: UserRole;
  active: boolean;
  permissions: PermissionKey[];
}): Promise<UserAccount> {
  if (isTauri()) {
    return call<UserAccount>("user_update", { actorId: requireActorId(), input });
  }
  const name = input.name.trim();
  if (name.length < 2) throw new Error("Nombre muy corto");
  if (input.pin && input.pin.trim().length < 4) throw new Error("PIN minimo de 4 digitos");
  if (mockUsers.some((user) => user.id !== input.id && user.name.toLowerCase() === name.toLowerCase())) {
    throw new Error("Ya existe usuario con ese nombre");
  }
  const current = mockUsers.find((user) => user.id === input.id);
  if (!current) throw new Error("Usuario no encontrado");
  if (current.role === "admin" && !input.active && mockUsers.filter((user) => user.role === "admin" && user.active && user.id !== input.id).length === 0) {
    throw new Error("Debe quedar al menos un admin activo");
  }
  const previousPin = mockPins.get(current.name.toLowerCase()) || "";
  mockPins.delete(current.name.toLowerCase());
  const next = {
    ...current,
    name,
    role: input.role,
    active: input.active,
    permissions: input.role === "admin" ? allMockPermissions : input.permissions,
  };
  mockUsers = mockUsers.map((user) => (user.id === input.id ? next : user));
  mockPins.set(name.toLowerCase(), input.pin?.trim() || previousPin);
  return next;
}

export async function deleteUser(id: number): Promise<void> {
  if (isTauri()) {
    return call<void>("user_delete", { actorId: requireActorId(), id });
  }
  const current = mockUsers.find((user) => user.id === id);
  if (!current) return;
  if (current.role === "admin" && mockUsers.filter((user) => user.role === "admin" && user.active && user.id !== id).length === 0) {
    throw new Error("Debe quedar al menos un admin activo");
  }
  mockUsers = mockUsers.map((user) => (user.id === id ? { ...user, active: false } : user));
}

export async function listCustomers(): Promise<Customer[]> {
  if (isTauri()) {
    return call<Customer[]>("customer_list", { actorId: requireActorId() });
  }
  return mockCustomers;
}

export async function listSuppliers(): Promise<Supplier[]> {
  if (isTauri()) {
    return call<Supplier[]>("supplier_list", { actorId: requireActorId() });
  }
  return mockSuppliers;
}

export async function upsertSupplier(input: SupplierInput): Promise<Supplier> {
  if (isTauri()) {
    return call<Supplier>("supplier_upsert", { actorId: requireActorId(), input });
  }
  const id = input.id ?? mockSupplierId;
  const previous = mockSuppliers.find((supplier) => supplier.id === id);
  const supplier: Supplier = {
    id,
    name: input.name.trim(),
    phone: input.phone ?? null,
    contact: input.contact ?? null,
    created_at: previous?.created_at ?? new Date().toISOString(),
  };
  if (!input.id) mockSupplierId += 1;
  mockSuppliers = [supplier, ...mockSuppliers.filter((current) => current.id !== id)];
  return supplier;
}

export async function createPurchase(input: PurchaseInput): Promise<PurchaseReceipt> {
  if (isTauri()) {
    return call<PurchaseReceipt>("purchase_create", { actorId: requireActorId(), input });
  }
  const product = demoProducts.find((candidate) => candidate.id === input.product_id);
  if (!product) throw new Error("Producto no disponible");
  const supplier = mockSuppliers.find((candidate) => candidate.id === input.supplier_id);
  const nextProduct = { ...product, stock: product.stock + input.quantity, cost: input.unit_cost };
  demoProducts = demoProducts.map((candidate) => (candidate.id === product.id ? nextProduct : candidate));
  mockInventoryMovements.unshift({
    id: mockInventoryMovementId,
    product_id: product.id,
    product_name: product.name,
    movement_type: "purchase",
    quantity: input.quantity,
    reason: input.note || "Compra",
    reference_id: mockPurchaseId,
    created_at: new Date().toISOString(),
  });
  mockInventoryMovementId += 1;
  const receipt: PurchaseReceipt = {
    id: mockPurchaseId,
    supplier_name: supplier?.name ?? null,
    product_name: product.name,
    quantity: input.quantity,
    unit_cost: input.unit_cost,
    total: input.quantity * input.unit_cost,
    created_at: new Date().toISOString(),
  };
  mockPurchaseId += 1;
  mockPurchases = [receipt, ...mockPurchases];
  return receipt;
}

export async function listPurchases(): Promise<PurchaseReceipt[]> {
  if (isTauri()) {
    return call<PurchaseReceipt[]>("purchase_list", { actorId: requireActorId() });
  }
  return mockPurchases;
}

export async function listTaxes(): Promise<TaxOption[]> {
  if (isTauri()) {
    return call<TaxOption[]>("tax_list", { actorId: requireActorId() });
  }
  return demoTaxes;
}

export async function createInvoiceDraft(saleId: number, customerId?: number | null): Promise<InvoiceDraft> {
  if (isTauri()) {
    return call<InvoiceDraft>("invoice_prepare", { actorId: requireActorId(), saleId, customerId });
  }
  const sale = mockSales.find((candidate) => candidate.id === saleId);
  if (!sale) throw new Error("Venta no encontrada");
  const customer = mockCustomers.find((candidate) => candidate.id === customerId);
  const invoice: InvoiceDraft = {
    id: mockInvoiceId,
    sale_id: saleId,
    customer_id: customerId ?? null,
    customer_name: customer?.name ?? null,
    folio: `PRE-${String(mockInvoiceId).padStart(6, "0")}`,
    status: "draft",
    total: sale.total,
    pac_message: "Listo para PAC real. Configura credenciales antes de timbrar.",
    created_at: new Date().toISOString(),
  };
  mockInvoiceId += 1;
  mockInvoices = [invoice, ...mockInvoices];
  return invoice;
}

export async function listInvoices(): Promise<InvoiceDraft[]> {
  if (isTauri()) {
    return call<InvoiceDraft[]>("invoice_list", { actorId: requireActorId() });
  }
  return mockInvoices;
}

export async function upsertCustomer(input: CustomerInput): Promise<Customer> {
  if (isTauri()) {
    return call<Customer>("customer_upsert", { actorId: requireActorId(), input });
  }
  const now = new Date().toISOString();
  const id = input.id ?? mockCustomerId;
  const previous = mockCustomers.find((customer) => customer.id === id);
  const customer: Customer = {
    id,
    name: input.name.trim(),
    rfc: input.rfc ?? null,
    phone: input.phone ?? null,
    email: input.email ?? null,
    credit_limit: input.credit_limit,
    balance: previous?.balance ?? 0,
    created_at: previous?.created_at ?? now,
  };
  if (!input.id) mockCustomerId += 1;
  mockCustomers = [customer, ...mockCustomers.filter((current) => current.id !== id)];
  return customer;
}

export async function adjustCustomerCredit(input: { customer_id: number; amount: number; reason: string }): Promise<Customer> {
  if (isTauri()) {
    return call<Customer>("customer_credit_adjust", { actorId: requireActorId(), input });
  }
  const customer = mockCustomers.find((current) => current.id === input.customer_id);
  if (!customer) throw new Error("Cliente no encontrado");
  const next = { ...customer, balance: customer.balance + input.amount };
  mockCustomers = mockCustomers.map((current) => (current.id === next.id ? next : current));
  mockCustomerCreditMovements.unshift({
    id: mockCustomerCreditMovementId,
    customer_name: customer.name,
    amount: input.amount,
    reason: input.reason,
    created_at: new Date().toISOString(),
  });
  mockCustomerCreditMovementId += 1;
  return next;
}

export async function listHeldTickets(): Promise<HeldTicket[]> {
  if (isTauri()) {
    return call<HeldTicket[]>("held_ticket_list");
  }
  return mockHeldTickets;
}

export async function saveHeldTicket(input: {
  id?: number;
  name: string;
  cashier_id: number;
  items: HeldTicketItem[];
}): Promise<HeldTicket> {
  if (isTauri()) {
    return call<HeldTicket>("held_ticket_save", { input });
  }
  const name = input.name.trim();
  if (name.length < 2) throw new Error("Nombre de ticket muy corto");
  if (input.items.length === 0) throw new Error("Ticket sin articulos");
  const now = new Date().toISOString();
  const cashierName = mockUsers.find((user) => user.id === input.cashier_id)?.name ?? "Cajero";
  const ticket: HeldTicket = {
    id: input.id ?? mockHeldTicketId,
    name,
    cashier_id: input.cashier_id,
    cashier_name: cashierName,
    item_count: input.items.length,
    total: input.items.reduce((sum, item) => {
      return sum + lineAmounts(item.quantity * item.unit_price, item.discount, item.tax_rate).total;
    }, 0),
    items: input.items,
    created_at: mockHeldTickets.find((current) => current.id === input.id)?.created_at ?? now,
    updated_at: now,
  };
  if (!input.id) {
    mockHeldTicketId += 1;
    mockHeldTickets = [...mockHeldTickets, ticket];
  } else {
    mockHeldTickets = mockHeldTickets.map((current) => (current.id === ticket.id ? ticket : current));
  }
  return ticket;
}

export async function deleteHeldTicket(id: number): Promise<void> {
  if (isTauri()) {
    return call<void>("held_ticket_delete", { id });
  }
  mockHeldTickets = mockHeldTickets.filter((ticket) => ticket.id !== id);
}

export async function getActiveSaleDraft(cashierId: number, cashSessionId?: number | null): Promise<ActiveSaleDraft | null> {
  if (isTauri()) {
    return call<ActiveSaleDraft | null>("active_sale_draft_get", { cashierId, cashSessionId: cashSessionId ?? null });
  }
  return mockActiveSaleDrafts.find((draft) =>
    draft.cashier_id === cashierId && (draft.cash_session_id == null || draft.cash_session_id === (cashSessionId ?? null)),
  ) ?? null;
}

export async function saveActiveSaleDraft(input: {
  cashier_id: number;
  cash_session_id?: number | null;
  items: HeldTicketItem[];
  cash_received: number;
  card_received: number;
  transfer_received: number;
}): Promise<ActiveSaleDraft> {
  if (isTauri()) {
    return call<ActiveSaleDraft>("active_sale_draft_save", { input });
  }
  if (input.items.length === 0) throw new Error("Borrador sin articulos");
  const now = new Date().toISOString();
  const draft: ActiveSaleDraft = {
    cashier_id: input.cashier_id,
    cash_session_id: input.cash_session_id ?? null,
    item_count: input.items.length,
    total: input.items.reduce((sum, item) => {
      return sum + lineAmounts(item.quantity * item.unit_price, item.discount, item.tax_rate).total;
    }, 0),
    cash_received: input.cash_received,
    card_received: input.card_received,
    transfer_received: input.transfer_received,
    items: input.items,
    updated_at: now,
  };
  mockActiveSaleDrafts = [draft, ...mockActiveSaleDrafts.filter((current) => current.cashier_id !== input.cashier_id)];
  return draft;
}

export async function clearActiveSaleDraft(cashierId: number): Promise<void> {
  if (isTauri()) {
    return call<void>("active_sale_draft_clear", { cashierId });
  }
  mockActiveSaleDrafts = mockActiveSaleDrafts.filter((draft) => draft.cashier_id !== cashierId);
}

export async function createSale(input: {
  cashier_id: number;
  customer_id?: number | null;
  items: Array<{ product_id: number; quantity: number; unit_price: number; discount: number }>;
  payments: Payment[];
  notes?: string | null;
}): Promise<SaleReceipt> {
  if (isTauri()) {
    return call<SaleReceipt>("sale_create", { draft: input });
  }
  if (!mockCashSession || mockCashSession.status !== "open") throw new Error("No hay turno abierto para registrar venta");
  for (const item of input.items) {
    const product = demoProducts.find((candidate) => candidate.id === item.product_id && candidate.active);
    if (!product) throw new Error(`Producto no disponible: ${item.product_id}`);
    if (item.quantity <= 0) throw new Error("Cantidad invalida");
    if (product.stock < item.quantity) throw new Error(`Stock insuficiente para producto ${item.product_id}`);
  }
  const subtotal = input.items.reduce((sum, item) => {
    const product = demoProducts.find((candidate) => candidate.id === item.product_id);
    return sum + lineAmounts(item.quantity * item.unit_price, item.discount, product?.tax_rate ?? 0).subtotal;
  }, 0);
  const tax = input.items.reduce((sum, item) => {
    const product = demoProducts.find((candidate) => candidate.id === item.product_id);
    return sum + lineAmounts(item.quantity * item.unit_price, item.discount, product?.tax_rate ?? 0).tax;
  }, 0);
  const total = Math.round((subtotal + tax) * 100) / 100;
  const paid = input.payments.reduce((sum, payment) => sum + payment.amount, 0);
  if (paid < total) throw new Error("Pago insuficiente");
  const cashPaid = input.payments.filter((payment) => payment.method === "cash").reduce((sum, payment) => sum + payment.amount, 0);
  const nonCashPaid = paid - cashPaid;
  if (nonCashPaid > total) throw new Error("Tarjeta/credito excede total");
  const cashNeeded = Math.max(0, total - nonCashPaid);
  const changeDue = Math.max(0, cashPaid - cashNeeded);
  const createdAt = new Date().toISOString();
  const month = createdAt.slice(0, 7);
  const monthlySeq = (mockMonthlySeq.get(month) ?? 0) + 1;
  mockMonthlySeq.set(month, monthlySeq);
  const receipt: SaleReceipt = {
    sale_id: mockSaleId,
    folio: `${month}-${String(monthlySeq).padStart(3, "0")}`,
    subtotal,
    tax,
    discount: input.items.reduce((sum, item) => sum + item.discount, 0),
    total,
    paid,
    change_due: changeDue,
    created_at: createdAt,
  };
  mockSaleId += 1;
  mockSales.unshift({
    id: receipt.sale_id,
    folio: receipt.folio,
    monthly_seq: monthlySeq,
    cashier_name: mockUsers.find((user) => user.id === input.cashier_id)?.name ?? "Cajero",
    total,
    paid,
    cash_paid: cashPaid,
    card_paid: input.payments.filter((payment) => payment.method === "card").reduce((sum, payment) => sum + payment.amount, 0),
    transfer_paid: input.payments.filter((payment) => payment.method === "transfer").reduce((sum, payment) => sum + payment.amount, 0),
    status: "paid",
    created_at: receipt.created_at,
  });
  mockSaleItems.set(receipt.sale_id, input.items.map((item) => ({ product_id: item.product_id, quantity: item.quantity })));
  input.items.forEach((item) => {
    const product = demoProducts.find((candidate) => candidate.id === item.product_id);
    if (!product) return;
    demoProducts = demoProducts.map((candidate) =>
      candidate.id === product.id ? { ...candidate, stock: Math.max(0, candidate.stock - item.quantity) } : candidate,
    );
    mockInventoryMovements.unshift({
      id: mockInventoryMovementId,
      product_id: product.id,
      product_name: product.name,
      movement_type: "sale",
      quantity: -item.quantity,
      reason: "Venta",
      reference_id: receipt.sale_id,
      created_at: receipt.created_at,
    });
    mockInventoryMovementId += 1;
  });
  if (mockCashSession) {
    mockCashSession.sales_total += total;
    mockCashSession.expected_cash += cashPaid - receipt.change_due;
  }
  return receipt;
}

export async function listSales(): Promise<SaleListItem[]> {
  if (isTauri()) {
    return call<SaleListItem[]>("sale_list", { actorId: requireActorId(), limit: 80 });
  }
  return mockSales;
}

export async function cancelSale(input: { sale_id: number; actor_id: number; reason: string }): Promise<void> {
  if (isTauri()) {
    return call<void>("sale_cancel", { saleId: input.sale_id, actorId: input.actor_id, reason: input.reason });
  }
  const sale = mockSales.find((candidate) => candidate.id === input.sale_id);
  if (!sale) throw new Error("Venta no encontrada");
  if (sale.status === "canceled") throw new Error("Venta ya cancelada");
  const now = new Date().toISOString();
  const items = mockSaleItems.get(input.sale_id) ?? [];
  items.forEach((item) => {
    const product = demoProducts.find((candidate) => candidate.id === item.product_id);
    if (!product) return;
    demoProducts = demoProducts.map((candidate) =>
      candidate.id === product.id ? { ...candidate, stock: candidate.stock + item.quantity } : candidate,
    );
    mockInventoryMovements.unshift({
      id: mockInventoryMovementId,
      product_id: product.id,
      product_name: product.name,
      movement_type: "cancel",
      quantity: item.quantity,
      reason: input.reason,
      reference_id: input.sale_id,
      created_at: now,
    });
    mockInventoryMovementId += 1;
  });
  if (mockCashSession?.status === "open") {
    const cashPaid = sale.cash_paid ?? 0;
    const changeDue = Math.max(0, (sale.paid ?? 0) - sale.total);
    mockCashSession.sales_total = Math.max(0, mockCashSession.sales_total - sale.total);
    mockCashSession.expected_cash -= cashPaid - changeDue;
  }
  mockSales = mockSales.map((candidate) => (candidate.id === input.sale_id ? { ...candidate, status: "canceled" } : candidate));
}

export async function openCashSession(openingCash: number, openedBy = 1): Promise<CashSession> {
  if (isTauri()) {
    return call<CashSession>("cash_session_open", { openedBy, openingCash });
  }
  mockCashSession = {
    id: 1,
    opened_by: 1,
    opened_at: new Date().toISOString(),
    opening_cash: openingCash,
    expected_cash: openingCash,
    sales_total: 0,
    status: "open",
  };
  return mockCashSession;
}

export async function closeCashSession(sessionId: number, closingCash: number): Promise<CashSession> {
  if (isTauri()) {
    return call<CashSession>("cash_session_close", { sessionId, closingCash });
  }
  if (!mockCashSession) throw new Error("No hay caja abierta");
  mockCashSession = {
    ...mockCashSession,
    closed_at: new Date().toISOString(),
    closing_cash: closingCash,
    status: "closed",
  };
  return mockCashSession;
}

export async function createCashMovement(input: {
  session_id: number;
  movement_type: "in" | "out" | "drawer";
  amount: number;
  reason: string;
  actor_id: number;
}): Promise<CashMovement> {
  if (isTauri()) {
    return call<CashMovement>("cash_movement_create", { input });
  }
  const movement: CashMovement = {
    id: mockCashMovementId,
    session_id: input.session_id,
    movement_type: input.movement_type,
    amount: input.amount,
    reason: input.reason,
    actor_name: mockUsers.find((user) => user.id === input.actor_id)?.name ?? "Cajero",
    created_at: new Date().toISOString(),
  };
  mockCashMovementId += 1;
  mockCashMovements.unshift(movement);
  if (mockCashSession?.id === input.session_id) {
    mockCashSession = {
      ...mockCashSession,
      expected_cash: mockCashSession.expected_cash + (input.movement_type === "in" ? input.amount : input.movement_type === "out" ? -input.amount : 0),
    };
  }
  return movement;
}

export async function listCashMovements(sessionId: number): Promise<CashMovement[]> {
  if (isTauri()) {
    return call<CashMovement[]>("cash_movement_list", { actorId: requireActorId(), sessionId });
  }
  return mockCashMovements.filter((movement) => movement.session_id === sessionId);
}

function mockShiftCut(status = mockCashSession?.status ?? "closed"): ShiftCutSnapshot {
  if (!mockCashSession) throw new Error("No hay turno abierto");
  const paidSales = mockSales.filter((sale) => sale.status === "paid");
  const canceledSales = mockSales.filter((sale) => sale.status === "canceled");
  const netSales = paidSales.reduce((sum, sale) => sum + sale.total, 0);
  const tax = 0;
  const discount = 0;
  const cashReceived = paidSales.reduce((sum, sale) => sum + (sale.cash_paid ?? 0), 0);
  const changeDue = paidSales.reduce((sum, sale) => sum + Math.max(0, (sale.paid ?? 0) - sale.total), 0);
  return {
    shift_id: mockCashSession.id,
    cash_session_id: mockCashSession.id,
    status,
    opened_at: mockCashSession.opened_at,
    closed_at: mockCashSession.closed_at ?? null,
    opened_by_name: "Admin",
    closed_by_name: status === "closed" ? "Admin" : null,
    total_tickets: paidSales.length,
    canceled_tickets: canceledSales.length,
    gross_sales: netSales + discount,
    net_sales: netSales,
    tax,
    discount,
    cash_paid: Math.max(0, cashReceived - changeDue),
    card_paid: paidSales.reduce((sum, sale) => sum + (sale.card_paid ?? 0), 0),
    transfer_paid: paidSales.reduce((sum, sale) => sum + (sale.transfer_paid ?? 0), 0),
    average_ticket: paidSales.length ? netSales / paidSales.length : 0,
    opening_cash: mockCashSession.opening_cash,
    expected_cash: mockCashSession.expected_cash,
    closing_cash: mockCashSession.closing_cash ?? null,
    counted_cash: mockCashSession.closing_cash ?? null,
    cash_difference: mockCashSession.closing_cash == null ? null : mockCashSession.closing_cash - mockCashSession.expected_cash,
    difference_reason: null,
  };
}

export async function getShiftCutX(shiftId?: number): Promise<ShiftCutSnapshot> {
  if (isTauri()) {
    return call<ShiftCutSnapshot>("shift_cut_x", { actorId: requireActorId(), shiftId: shiftId ?? null });
  }
  return mockShiftCut("open");
}

export async function closeShiftCutZ(input: {
  shift_id: number;
  closing_cash: number;
  closed_by: number;
  denominations_json?: string;
  difference_reason?: string | null;
}): Promise<ShiftCutSnapshot> {
  if (isTauri()) {
    return call<ShiftCutSnapshot>("shift_cut_z", {
      shiftId: input.shift_id,
      closingCash: input.closing_cash,
      closedBy: input.closed_by,
      denominationsJson: input.denominations_json ?? "[]",
      differenceReason: input.difference_reason ?? null,
    });
  }
  if (!mockCashSession || mockCashSession.status !== "open") throw new Error("No hay turno abierto");
  const difference = input.closing_cash - mockCashSession.expected_cash;
  if (difference !== 0 && !input.difference_reason?.trim()) throw new Error("Motivo de diferencia requerido");
  mockCashSession = {
    ...mockCashSession,
    closed_at: new Date().toISOString(),
    closing_cash: input.closing_cash,
    status: "closed",
  };
  mockCashCounts.unshift({
    id: mockCashCountId,
    session_id: mockCashSession.id,
    shift_id: input.shift_id,
    count_type: "close",
    expected_cash: mockCashSession.expected_cash,
    counted_cash: input.closing_cash,
    difference,
    denominations_json: input.denominations_json ?? "[]",
    difference_reason: input.difference_reason ?? null,
    actor_name: mockUsers.find((user) => user.id === input.closed_by)?.name ?? "Cajero",
    created_at: new Date().toISOString(),
  });
  mockCashCountId += 1;
  mockLastCutZ = mockShiftCut("closed");
  return mockLastCutZ;
}

export async function createCashCount(input: {
  session_id: number;
  shift_id?: number | null;
  count_type: "audit" | "close";
  expected_cash: number;
  counted_cash: number;
  denominations_json: string;
  difference_reason?: string | null;
  actor_id: number;
}): Promise<CashCount> {
  if (isTauri()) {
    return call<CashCount>("cash_count_create", { input });
  }
  const difference = input.counted_cash - input.expected_cash;
  if (difference !== 0 && !input.difference_reason?.trim()) throw new Error("Motivo de diferencia requerido");
  const count: CashCount = {
    id: mockCashCountId,
    session_id: input.session_id,
    shift_id: input.shift_id ?? null,
    count_type: input.count_type,
    expected_cash: input.expected_cash,
    counted_cash: input.counted_cash,
    difference,
    denominations_json: input.denominations_json,
    difference_reason: input.difference_reason ?? null,
    actor_name: mockUsers.find((user) => user.id === input.actor_id)?.name ?? "Cajero",
    created_at: new Date().toISOString(),
  };
  mockCashCountId += 1;
  mockCashCounts.unshift(count);
  return count;
}

export async function listCashCounts(sessionId: number): Promise<CashCount[]> {
  if (isTauri()) {
    return call<CashCount[]>("cash_count_list", { actorId: requireActorId(), sessionId });
  }
  return mockCashCounts.filter((count) => count.session_id === sessionId);
}

export async function listShiftCuts(): Promise<ShiftCutSnapshot[]> {
  if (isTauri()) {
    return call<ShiftCutSnapshot[]>("shift_cut_history", { actorId: requireActorId(), limit: 30 });
  }
  return mockLastCutZ ? [mockLastCutZ] : mockCashSession ? [mockShiftCut(mockCashSession.status)] : [];
}

export async function printShiftCut(shiftId: number): Promise<HardwareResult> {
  if (isTauri()) {
    return call<HardwareResult>("print_shift_cut", { actorId: requireActorId(), shiftId });
  }
  return { ok: true, message: `Demo navegador: corte ${shiftId}` };
}

function emptyDailyCutSummary(date = new Date().toISOString().slice(0, 10)): DailyCutSummary {
  return {
    date,
    cut_count: 0,
    total_tickets: 0,
    canceled_tickets: 0,
    gross_sales: 0,
    net_sales: 0,
    tax: 0,
    discount: 0,
    cash_paid: 0,
    card_paid: 0,
    transfer_paid: 0,
    average_ticket: 0,
    opening_cash: 0,
    expected_cash: 0,
    counted_cash: 0,
    cash_difference: 0,
    cuts: [],
  };
}

export async function getDailyCutSummary(date?: string): Promise<DailyCutSummary> {
  if (isTauri()) {
    return call<DailyCutSummary>("daily_cut_summary", { actorId: requireActorId(), date: date ?? null });
  }
  const targetDate = date ?? new Date().toISOString().slice(0, 10);
  const cuts = (mockLastCutZ ? [mockLastCutZ] : [])
    .filter((cut) => cut.status === "closed" && (cut.closed_at ?? "").slice(0, 10) === targetDate);
  const summary = emptyDailyCutSummary(targetDate);
  summary.cuts = cuts;
  summary.cut_count = cuts.length;
  summary.total_tickets = cuts.reduce((sum, cut) => sum + cut.total_tickets, 0);
  summary.canceled_tickets = cuts.reduce((sum, cut) => sum + cut.canceled_tickets, 0);
  summary.gross_sales = cuts.reduce((sum, cut) => sum + cut.gross_sales, 0);
  summary.net_sales = cuts.reduce((sum, cut) => sum + cut.net_sales, 0);
  summary.tax = cuts.reduce((sum, cut) => sum + cut.tax, 0);
  summary.discount = cuts.reduce((sum, cut) => sum + cut.discount, 0);
  summary.cash_paid = cuts.reduce((sum, cut) => sum + cut.cash_paid, 0);
  summary.card_paid = cuts.reduce((sum, cut) => sum + cut.card_paid, 0);
  summary.transfer_paid = cuts.reduce((sum, cut) => sum + cut.transfer_paid, 0);
  summary.opening_cash = cuts.reduce((sum, cut) => sum + cut.opening_cash, 0);
  summary.expected_cash = cuts.reduce((sum, cut) => sum + cut.expected_cash, 0);
  summary.counted_cash = cuts.reduce((sum, cut) => sum + (cut.counted_cash ?? cut.closing_cash ?? 0), 0);
  summary.cash_difference = cuts.reduce((sum, cut) => sum + (cut.cash_difference ?? 0), 0);
  summary.average_ticket = summary.total_tickets ? summary.net_sales / summary.total_tickets : 0;
  return summary;
}

export async function printDailyCut(date?: string): Promise<HardwareResult> {
  if (isTauri()) {
    return call<HardwareResult>("print_daily_cut", { actorId: requireActorId(), date: date ?? null });
  }
  return { ok: true, message: `Demo navegador: corte general ${date ?? new Date().toISOString().slice(0, 10)}` };
}

export async function getMonthlySalesReport(month?: string): Promise<MonthlySalesReport[]> {
  if (isTauri()) {
    return call<MonthlySalesReport[]>("monthly_sales_report", { actorId: requireActorId(), month: month ?? null });
  }
  const closed = mockLastCutZ ? mockSales : [];
  const groups = new Map<string, { paid: SaleListItem[]; canceled: SaleListItem[] }>();
  closed.forEach((sale) => {
    const key = sale.created_at.slice(0, 7);
    if (month && key !== month) return;
    const group = groups.get(key) ?? { paid: [], canceled: [] };
    if (sale.status === "paid") group.paid.push(sale);
    if (sale.status === "canceled") group.canceled.push(sale);
    groups.set(key, group);
  });
  return Array.from(groups.entries()).map(([key, group]) => {
    const total = group.paid.reduce((sum, sale) => sum + sale.total, 0);
    return {
      month: key,
      total_tickets: group.paid.length,
      total_amount: total,
      average_ticket: group.paid.length ? total / group.paid.length : 0,
      canceled_tickets: group.canceled.length,
    };
  }).sort((left, right) => right.month.localeCompare(left.month));
}

export async function getDashboardSummary(): Promise<DashboardSummary> {
  if (isTauri()) {
    return call<DashboardSummary>("dashboard_summary", { actorId: requireActorId() });
  }
  return {
    active_products: demoProducts.length,
    low_stock_products: demoProducts.filter((product) => product.stock <= 0).length,
    today_sales: mockCashSession?.sales_total ?? 0,
    today_tickets: mockSaleId - 1,
    open_cash_session: mockCashSession?.status === "open" ? mockCashSession : null,
  };
}

export async function getAppBootstrap(): Promise<AppBootstrap> {
  if (isTauri()) {
    return call<AppBootstrap>("app_bootstrap", { actorId: requireActorId() });
  }
  return {
    summary: await getDashboardSummary(),
    products: await searchProducts(""),
    held_tickets: await listHeldTickets(),
    tax_enabled: mockBoolSetting("tax_enabled", true),
    tax_prices_include_tax: mockBoolSetting("tax_prices_include_tax", true),
  };
}

export async function getReportSummary(): Promise<ReportSummary> {
  if (isTauri()) {
    return call<ReportSummary>("report_summary", { actorId: requireActorId() });
  }
  const paidSales = mockSales.filter((sale) => sale.status === "paid");
  const todaySales = paidSales.reduce((sum, sale) => sum + sale.total, 0);
  return {
    today_sales: todaySales,
    today_tickets: paidSales.length,
    average_ticket: paidSales.length ? todaySales / paidSales.length : 0,
    gross_profit: 0,
    cash_expected: mockCashSession?.expected_cash ?? 0,
    cash_sales: paidSales.reduce((sum, sale) => sum + (sale.cash_paid ?? 0), 0),
    card_sales: paidSales.reduce((sum, sale) => sum + (sale.card_paid ?? 0), 0),
    transfer_sales: paidSales.reduce((sum, sale) => sum + (sale.transfer_paid ?? 0), 0),
    low_stock_products: demoProducts.filter((product) => product.active && product.stock <= 0).length,
  };
}

export async function getProductSalesReport(filters?: { fromDate?: string; toDate?: string }): Promise<ProductSalesReport[]> {
  if (isTauri()) {
    return call<ProductSalesReport[]>("report_product_sales", {
      actorId: requireActorId(),
      limit: 100,
      fromDate: filters?.fromDate ?? null,
      toDate: filters?.toDate ?? null,
    });
  }
  return demoProducts.slice(0, 6).map((product) => ({
    product_id: product.id,
    product_name: product.name,
    category: product.category,
    quantity: 0,
    total: 0,
    gross_profit: 0,
  }));
}

export async function getTaxBreakdown(filters?: { fromDate?: string; toDate?: string }): Promise<TaxBreakdown[]> {
  if (isTauri()) {
    return call<TaxBreakdown[]>("report_tax_breakdown", {
      actorId: requireActorId(),
      fromDate: filters?.fromDate ?? null,
      toDate: filters?.toDate ?? null,
    });
  }
  return [];
}

export async function listReportMovements(filters?: { fromDate?: string; toDate?: string }): Promise<ReportMovement[]> {
  if (isTauri()) {
    return call<ReportMovement[]>("report_movement_history", {
      actorId: requireActorId(),
      limit: 500,
      fromDate: filters?.fromDate ?? null,
      toDate: filters?.toDate ?? null,
    });
  }
  const rows: ReportMovement[] = [
    ...mockSales.map((sale) => ({
      id: `sale-${sale.id}`,
      kind: "sale" as const,
      title: sale.status === "paid" ? `Venta ${sale.folio}` : `Cancelacion ${sale.folio}`,
      detail: `${sale.cashier_name} · efectivo ${sale.cash_paid ?? 0} · tarjeta ${sale.card_paid ?? 0} · crédito ${sale.transfer_paid ?? 0}`,
      amount: sale.status === "paid" ? sale.total : -sale.total,
      gross_profit: 0,
      cash_paid: sale.cash_paid ?? 0,
      card_paid: sale.card_paid ?? 0,
      transfer_paid: sale.transfer_paid ?? 0,
      tax_total: 0,
      card_terminal: null,
      actor_name: sale.cashier_name,
      cash_session_id: mockCashSession?.id ?? null,
      created_at: sale.created_at,
    })),
    ...mockCashMovements.map((movement) => ({
      id: `cash-${movement.id}`,
      kind: "cash" as const,
      title: movement.movement_type === "in" ? "Entrada caja" : movement.movement_type === "out" ? "Retiro caja" : "Cajon abierto",
      detail: movement.reason,
      amount: movement.movement_type === "in" ? movement.amount : movement.movement_type === "out" ? -movement.amount : 0,
      gross_profit: 0,
      cash_paid: 0,
      card_paid: 0,
      transfer_paid: 0,
      tax_total: 0,
      card_terminal: null,
      actor_name: movement.actor_name,
      cash_session_id: movement.session_id,
      created_at: movement.created_at,
    })),
    ...mockPurchases.map((purchase) => ({
      id: `purchase-${purchase.id}`,
      kind: "purchase" as const,
      title: `Compra ${purchase.id}`,
      detail: `${purchase.product_name} · ${purchase.supplier_name ?? "Sin proveedor"}`,
      amount: -purchase.total,
      gross_profit: 0,
      cash_paid: 0,
      card_paid: 0,
      transfer_paid: 0,
      tax_total: 0,
      card_terminal: null,
      actor_name: null,
      cash_session_id: null,
      created_at: purchase.created_at,
    })),
    ...mockInventoryMovements.map((movement) => ({
      id: `inventory-${movement.id}`,
      kind: "inventory" as const,
      title: movement.movement_type === "purchase" ? "Inventario por compra" : "Inventario",
      detail: `${movement.product_name} · ${movement.reason} · ${movement.quantity}`,
      amount: 0,
      gross_profit: 0,
      cash_paid: 0,
      card_paid: 0,
      transfer_paid: 0,
      tax_total: 0,
      card_terminal: null,
      actor_name: null,
      cash_session_id: null,
      created_at: movement.created_at,
    })),
    ...mockCustomerCreditMovements.map((movement) => ({
      id: `credit-${movement.id}`,
      kind: "credit" as const,
      title: movement.amount > 0 ? "Cargo cliente" : "Abono cliente",
      detail: `${movement.customer_name} · ${movement.reason}`,
      amount: movement.amount,
      gross_profit: 0,
      cash_paid: 0,
      card_paid: 0,
      transfer_paid: 0,
      tax_total: 0,
      card_terminal: null,
      actor_name: null,
      cash_session_id: null,
      created_at: movement.created_at,
    })),
  ];
  return rows.sort((left, right) => right.created_at.localeCompare(left.created_at));
}

export async function createBackup(): Promise<BackupResult> {
  if (isTauri()) {
    return call<BackupResult>("backup_create", { actorId: requireActorId() });
  }
  return { path: "Mock: navegador no escribe backup SQLite", created_at: new Date().toISOString() };
}

export async function listBackups(): Promise<BackupFile[]> {
  if (isTauri()) {
    return call<BackupFile[]>("backup_list", { actorId: requireActorId() });
  }
  return [];
}

export async function restoreBackup(path: string): Promise<BackupRestoreResult> {
  if (isTauri()) {
    return call<BackupRestoreResult>("backup_restore", { actorId: requireActorId(), path });
  }
  return {
    restored_path: path,
    safety_backup_path: "Mock: backup de seguridad navegador",
    restored_at: new Date().toISOString(),
  };
}

export async function listAuditLog(): Promise<AuditLogEntry[]> {
  if (isTauri()) {
    return call<AuditLogEntry[]>("audit_log_list", { actorId: requireActorId(), limit: 80 });
  }
  return [];
}

export async function createAutoBackupIfDue(): Promise<BackupResult | null> {
  if (isTauri()) {
    return call<BackupResult | null>("backup_auto_if_due", { actorId: requireActorId() });
  }
  return null;
}

export async function getSetting(key: string): Promise<string | null> {
  if (isTauri()) {
    return call<string | null>("settings_get", { actorId: requireActorId(), key });
  }
  return mockSettings.get(key) ?? null;
}

export async function setSetting(key: string, value: string): Promise<void> {
  if (isTauri()) {
    return call<void>("settings_set", { actorId: requireActorId(), key, value });
  }
  mockSettings.set(key, value);
}

export async function listHardwareDevices(): Promise<HardwareDevice[]> {
  if (isTauri()) {
    return call<HardwareDevice[]>("hardware_device_list", { actorId: requireActorId() });
  }
  return [
    {
      id: "mock-printer-80mm",
      name: "Mock 80mm",
      device_type: "printer",
      connection: "mock",
      detail: "Impresora de prueba navegador",
      is_default: true,
    },
    {
      id: "mock-scale-serial",
      name: "Mock bascula serial",
      device_type: "serial",
      connection: "mock",
      detail: "Puerto serial de prueba",
      is_default: false,
    },
    {
      id: "mock-drawer-escpos",
      name: "Mock cajon ESC/POS",
      device_type: "cash_drawer",
      connection: "mock",
      detail: "Pulso por impresora",
      is_default: false,
    },
  ];
}

export async function printTicket(saleId: number): Promise<HardwareResult> {
  if (isTauri()) {
    return call<HardwareResult>("print_ticket", { saleId });
  }
  return { ok: true, message: `Demo navegador: ticket venta ${saleId}` };
}

export async function openDrawer(): Promise<HardwareResult> {
  if (isTauri()) {
    return call<HardwareResult>("open_cash_drawer");
  }
  return { ok: true, message: "Demo navegador: cajon no fisico" };
}

export async function readScale(): Promise<ScaleReading> {
  if (isTauri()) {
    return call<ScaleReading>("read_scale");
  }
  return { ok: true, weight: 1, unit: "kg", source: "demo navegador" };
}
