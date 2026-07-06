import { useState } from "react";
import type { ConfirmDraft } from "../../components/modals/CommonModals";
import { hasPermission } from "../../navigation";
import type { UserSession } from "../../types";
import { CustomersView } from "./CustomersView";
import { PurchasesView } from "../purchases/PurchasesView";

type SubTab = "customers" | "purchases";

export function CustomersPurchasesView({
  session,
  showToast,
  requestConfirm,
}: {
  session: UserSession;
  showToast: (message: string) => void;
  requestConfirm: (draft: ConfirmDraft) => void;
}) {
  const isAdmin = session.role === "admin";
  const canViewCustomers = isAdmin || hasPermission(session.permissions, "customers");
  const canViewPurchases = isAdmin || hasPermission(session.permissions, "purchases");
  const [tab, setTab] = useState<SubTab>(canViewCustomers ? "customers" : "purchases");

  const tabs: Array<{ key: SubTab; label: string }> = [
    ...(canViewCustomers ? [{ key: "customers" as const, label: "Clientes" }] : []),
    ...(canViewPurchases ? [{ key: "purchases" as const, label: "Compras" }] : []),
  ];

  return (
    <section className="admin-panel compact">
      {tabs.length > 1 && (
        <div className="report-tabs" role="tablist" aria-label="Clientes y compras">
          {tabs.map((item) => (
            <button
              className={tab === item.key ? "active" : undefined}
              type="button"
              role="tab"
              aria-selected={tab === item.key}
              key={item.key}
              onClick={() => setTab(item.key)}
            >
              {item.label}
            </button>
          ))}
        </div>
      )}
      {tab === "customers" && canViewCustomers ? (
        <CustomersView showToast={showToast} />
      ) : (
        <PurchasesView showToast={showToast} requestConfirm={requestConfirm} />
      )}
    </section>
  );
}
