import type { ConfirmDraft } from "../../components/modals/CommonModals";
import { lazy, Suspense } from "react";
import type { ProductSearchOptions } from "../../lib/posApi";
import type { ViewKey } from "../../navigation";
import type { DashboardSummary, Product, UserSession } from "../../types";

const AdministrationView = lazy(() => import("./AdministrationView").then((module) => ({ default: module.AdministrationView })));
const CashView = lazy(() => import("../cash/CashView").then((module) => ({ default: module.CashView })));
const CustomersPurchasesView = lazy(() => import("../customers/CustomersPurchasesView").then((module) => ({ default: module.CustomersPurchasesView })));
const HistoryReturnsView = lazy(() => import("../history/HistoryReturnsView").then((module) => ({ default: module.HistoryReturnsView })));
const InventoryView = lazy(() => import("../inventory/InventoryView").then((module) => ({ default: module.InventoryView })));
const InvoicesView = lazy(() => import("../invoices/InvoicesView").then((module) => ({ default: module.InvoicesView })));
const ProductsView = lazy(() => import("../products/ProductsView").then((module) => ({ default: module.ProductsView })));
const ReportsView = lazy(() => import("../reports/ReportsView").then((module) => ({ default: module.ReportsView })));
const SettingsView = lazy(() => import("../settings/SettingsView").then((module) => ({ default: module.SettingsView })));
const UsersView = lazy(() => import("../users/UsersView").then((module) => ({ default: module.UsersView })));

export function preloadAdminViews() {
  void import("./AdministrationView");
  void import("../cash/CashView");
  void import("../customers/CustomersPurchasesView");
  void import("../history/HistoryReturnsView");
  void import("../inventory/InventoryView");
  void import("../invoices/InvoicesView");
  void import("../products/ProductsView");
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
  onTaxModeChange: (mode: { enabled: boolean; pricesIncludeTax: boolean; roundTotalUp?: boolean }) => void;
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
        canViewProfit={session.role === "admin" || session.permissions.includes("admin") || session.permissions.includes("view_profit")}
        openCash={openCash}
        refreshSummary={refreshSummary}
        showToast={showToast}
        requestConfirm={requestConfirm}
      />
    );
  }

  else if (view === "reports") content = <ReportsView showToast={showToast} />;

  else if (view === "invoices") content = <InvoicesView showToast={showToast} />;

  else if (view === "history") content = <HistoryReturnsView showToast={showToast} />;

  else if (view === "settings") content = <SettingsView showToast={showToast} onTaxModeChange={onTaxModeChange} />;
  else if (view === "administration") content = <AdministrationView showToast={showToast} />;
  else content = <CustomersPurchasesView session={session} showToast={showToast} requestConfirm={requestConfirm} />;

  return <Suspense fallback={null}>{content}</Suspense>;
}
