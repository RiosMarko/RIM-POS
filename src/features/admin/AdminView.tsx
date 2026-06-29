import type { ConfirmDraft } from "../../components/modals/CommonModals";
import { AdministrationView } from "./AdministrationView";
import { CashView } from "../cash/CashView";
import { CustomersView } from "../customers/CustomersView";
import { InventoryView } from "../inventory/InventoryView";
import { InvoicesView } from "../invoices/InvoicesView";
import { ProductsView } from "../products/ProductsView";
import { PurchasesView } from "../purchases/PurchasesView";
import { ReportsView } from "../reports/ReportsView";
import { SettingsView } from "../settings/SettingsView";
import { UsersView } from "../users/UsersView";
import type { ViewKey } from "../../navigation";
import type { DashboardSummary, Product, UserSession } from "../../types";

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
}: {
  view: ViewKey;
  session: UserSession;
  products: Product[];
  summary: DashboardSummary | null;
  openCash: (openingCash?: number) => void;
  refreshProducts: (query?: string) => Promise<void>;
  refreshSummary: () => Promise<void>;
  showToast: (message: string) => void;
  onTaxModeChange: (enabled: boolean) => void;
  requestConfirm: (draft: ConfirmDraft) => void;
}) {
  if (view === "users") return <UsersView showToast={showToast} requestConfirm={requestConfirm} />;

  if (view === "products") return <ProductsView products={products} refreshProducts={refreshProducts} showToast={showToast} requestConfirm={requestConfirm} />;
  if (view === "inventory") return <InventoryView products={products} refreshProducts={refreshProducts} showToast={showToast} />;

  if (view === "cash") {
    const cashSession = summary?.open_cash_session;
    return (
      <CashView
        session={session}
        cashSession={cashSession ?? null}
        tickets={summary?.today_tickets ?? 0}
        openCash={openCash}
        refreshSummary={refreshSummary}
        showToast={showToast}
        requestConfirm={requestConfirm}
      />
    );
  }

  if (view === "reports") return <ReportsView showToast={showToast} />;

  if (view === "purchases") {
    return (
      <PurchasesView
        session={session}
        products={products}
        refreshProducts={refreshProducts}
        showToast={showToast}
      />
    );
  }

  if (view === "invoices") return <InvoicesView showToast={showToast} />;

  if (view === "settings") return <SettingsView showToast={showToast} onTaxModeChange={onTaxModeChange} />;
  if (view === "administration") return <AdministrationView showToast={showToast} />;

  return <CustomersView showToast={showToast} />;
}
