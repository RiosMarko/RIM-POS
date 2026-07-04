import type { ConfirmDraft } from "../../components/modals/CommonModals";
import { lazy, Suspense } from "react";
import type { ProductSearchOptions } from "../../lib/posApi";
import type { ViewKey } from "../../navigation";
import type { DashboardSummary, Product, UserSession } from "../../types";

const AdministrationView = lazy(() => import("./AdministrationView").then((module) => ({ default: module.AdministrationView })));
const CashView = lazy(() => import("../cash/CashView").then((module) => ({ default: module.CashView })));
const CustomersView = lazy(() => import("../customers/CustomersView").then((module) => ({ default: module.CustomersView })));
const InventoryView = lazy(() => import("../inventory/InventoryView").then((module) => ({ default: module.InventoryView })));
const InvoicesView = lazy(() => import("../invoices/InvoicesView").then((module) => ({ default: module.InvoicesView })));
const ProductsView = lazy(() => import("../products/ProductsView").then((module) => ({ default: module.ProductsView })));
const PurchasesView = lazy(() => import("../purchases/PurchasesView").then((module) => ({ default: module.PurchasesView })));
const ReportsView = lazy(() => import("../reports/ReportsView").then((module) => ({ default: module.ReportsView })));
const SettingsView = lazy(() => import("../settings/SettingsView").then((module) => ({ default: module.SettingsView })));
const UsersView = lazy(() => import("../users/UsersView").then((module) => ({ default: module.UsersView })));

export function preloadAdminViews() {
  void import("./AdministrationView");
  void import("../cash/CashView");
  void import("../customers/CustomersView");
  void import("../inventory/InventoryView");
  void import("../invoices/InvoicesView");
  void import("../products/ProductsView");
  void import("../purchases/PurchasesView");
  void import("../reports/ReportsView");
  void import("../settings/SettingsView");
  void import("../users/UsersView");
}

export function AdminView({
  view,
  session,
  products,
  summary,
  openCash,
  refreshProducts,
  refreshSummary,
  showToast,
  onTaxModeChange,
  requestConfirm,
  requestView,
}: {
  view: ViewKey;
  session: UserSession;
  products: Product[];
  summary: DashboardSummary | null;
  openCash: (openingCash?: number) => void;
  refreshProducts: (query?: string, options?: ProductSearchOptions) => Promise<Product[]>;
  refreshSummary: () => Promise<void>;
  showToast: (message: string) => void;
  onTaxModeChange: (mode: { enabled: boolean; pricesIncludeTax: boolean }) => void;
  requestConfirm: (draft: ConfirmDraft) => void;
  requestView: (view: ViewKey) => void;
}) {
  let content;
  if (view === "users") content = <UsersView showToast={showToast} requestConfirm={requestConfirm} />;

  else if (view === "products") content = <ProductsView products={products} refreshProducts={refreshProducts} showToast={showToast} requestConfirm={requestConfirm} />;
  else if (view === "inventory") content = <InventoryView products={products} refreshProducts={refreshProducts} showToast={showToast} />;

  else if (view === "cash") {
    const cashSession = summary?.open_cash_session;
    content = (
      <CashView
        session={session}
        cashSession={cashSession ?? null}
        tickets={summary?.today_tickets ?? 0}
        canViewProfit={session.role === "admin" || session.permissions.includes("view_profit")}
        openCash={openCash}
        refreshSummary={refreshSummary}
        showToast={showToast}
        requestConfirm={requestConfirm}
      />
    );
  }

  else if (view === "reports") content = <ReportsView showToast={showToast} />;

  else if (view === "purchases") {
    content = (
      <PurchasesView
        session={session}
        products={products}
        refreshProducts={refreshProducts}
        showToast={showToast}
      />
    );
  }

  else if (view === "invoices") content = <InvoicesView showToast={showToast} />;

  else if (view === "settings") content = <SettingsView showToast={showToast} onTaxModeChange={onTaxModeChange} />;
  else if (view === "administration") content = <AdministrationView showToast={showToast} />;
  else content = <CustomersView showToast={showToast} />;

  return <Suspense fallback={null}>{content}</Suspense>;
}
