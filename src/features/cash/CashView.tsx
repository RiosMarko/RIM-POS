import { BarChart3, Banknote, FileText, ShoppingCart } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Metric } from "../../components/display/SummaryCards";
import type { ConfirmDraft } from "../../components/modals/CommonModals";
import { money } from "../../lib/money";
import {
  cancelSale,
  closeShiftCutZ,
  createCashMovement,
  getShiftCutX,
  listCashMovements,
  listSales,
} from "../../lib/posApi";
import type { CashMovement, CashSession, SaleListItem, ShiftCutSnapshot, UserSession } from "../../types";
import { CashDialog, SaleCancelModal } from "./CashModals";

export function CashView({
  session,
  cashSession,
  tickets,
  openCash,
  refreshSummary,
  showToast,
  requestConfirm,
}: {
  session: UserSession;
  cashSession: CashSession | null;
  tickets: number;
  openCash: (openingCash?: number) => void;
  refreshSummary: () => Promise<void>;
  showToast: (message: string) => void;
  requestConfirm: (draft: ConfirmDraft) => void;
}) {
  const [movements, setMovements] = useState<CashMovement[]>([]);
  const [sales, setSales] = useState<SaleListItem[]>([]);
  const [cashDialog, setCashDialog] = useState<"open" | "in" | "out" | "audit" | "close" | null>(null);
  const [cutSnapshot, setCutSnapshot] = useState<ShiftCutSnapshot | null>(null);
  const [cancelDraft, setCancelDraft] = useState<SaleListItem | null>(null);
  const paidSales = useMemo(() => sales.filter((sale) => sale.status === "paid"), [sales]);
  const cashSales = useMemo(() => paidSales.reduce((sum, sale) => sum + (sale.cash_paid ?? 0), 0), [paidSales]);
  const cardSales = useMemo(() => paidSales.reduce((sum, sale) => sum + (sale.card_paid ?? 0), 0), [paidSales]);
  const transferSales = useMemo(() => paidSales.reduce((sum, sale) => sum + (sale.transfer_paid ?? 0), 0), [paidSales]);
  const expectedCash = cashSession?.expected_cash ?? 0;
  const totalSales = cashSales + cardSales + transferSales;

  const refresh = useCallback(async () => {
    if (cashSession) setMovements(await listCashMovements(cashSession.id));
    setSales(await listSales());
  }, [cashSession]);

  useEffect(() => {
    refresh().catch((error) => showToast(String(error)));
  }, [refresh, showToast]);

  const addMovement = async (movement_type: "in" | "out", amount: number, reason: string) => {
    if (!cashSession) {
      showToast("Abre caja primero");
      return;
    }
    try {
      await createCashMovement({ session_id: cashSession.id, movement_type, amount, reason, actor_id: session.id });
      await refreshSummary();
      await refresh();
      setCashDialog(null);
      showToast("Movimiento registrado");
    } catch (error) {
      showToast(String(error));
    }
  };

  const cancel = async (sale: SaleListItem, reason: string) => {
    if (!reason) return;
    try {
      await cancelSale({ sale_id: sale.id, actor_id: session.id, reason });
      await refreshSummary();
      await refresh();
      setCancelDraft(null);
      showToast("Venta cancelada");
    } catch (error) {
      showToast(String(error));
    }
  };

  const cashAudit = (counted: number) => {
    const diff = counted - expectedCash;
    setCashDialog(null);
    showToast(`Arqueo: contado ${money(counted)}, diferencia ${money(diff)}`);
  };

  const partialCut = async () => {
    try {
      const snapshot = await getShiftCutX();
      setCutSnapshot(snapshot);
      showToast(`Corte X: ${snapshot.total_tickets} tickets, ${money(snapshot.net_sales)}`);
    } catch (error) {
      showToast(String(error));
    }
  };

  const finalCut = async (counted: number) => {
    if (!cashSession) return;
    requestConfirm({
      title: "Cerrar turno",
      message: `Corte Z es irreversible. Efectivo contado: ${money(counted)}.`,
      confirmLabel: "Aplicar Corte Z",
      tone: "danger",
      onConfirm: async () => {
        setCashDialog(null);
        try {
          const preview = await getShiftCutX();
          const snapshot = await closeShiftCutZ({ shift_id: preview.shift_id, closing_cash: counted, closed_by: session.id });
          setCutSnapshot(snapshot);
          await refresh();
          await refreshSummary();
          showToast("Corte Z aplicado. Turno cerrado");
        } catch (error) {
          showToast(String(error));
        }
      },
    });
  };

  return (
    <section className="admin-panel compact">
      <div className="metric-grid">
        <Metric icon={Banknote} label="Esperado" value={money(expectedCash)} />
        <Metric icon={ShoppingCart} label="Efectivo" value={money(cashSales)} />
        <Metric icon={FileText} label="Tarjeta" value={money(cardSales)} />
        <Metric icon={BarChart3} label="Crédito" value={money(transferSales)} />
      </div>
      <div className="cash-actions">
        <button className="primary-button" type="button" disabled={Boolean(cashSession)} onClick={() => setCashDialog("open")}>
          Abrir caja
        </button>
        <button className="ghost-button" type="button" disabled={!cashSession} onClick={() => setCashDialog("in")}>
          Entrada
        </button>
        <button className="ghost-button" type="button" disabled={!cashSession} onClick={() => setCashDialog("out")}>
          Retiro
        </button>
        <button className="ghost-button" type="button" disabled={!cashSession} onClick={() => setCashDialog("audit")}>
          Arqueo
        </button>
        <button className="ghost-button" type="button" disabled={!cashSession} onClick={partialCut}>
          Corte X
        </button>
        <button className="danger-button" type="button" disabled={!cashSession} onClick={() => setCashDialog("close")}>
          Corte Z final
        </button>
      </div>
      {cutSnapshot && (
        <div className="cut-summary">
          <strong>{cutSnapshot.status === "closed" ? "Corte Z" : "Corte X"}</strong>
          <span>Tickets {cutSnapshot.total_tickets}</span>
          <span>Cancelados {cutSnapshot.canceled_tickets}</span>
          <span>Ventas {money(cutSnapshot.net_sales)}</span>
          <span>Impuestos {money(cutSnapshot.tax)}</span>
          <span>Efectivo {money(cutSnapshot.cash_paid)}</span>
          <span>Tarjeta {money(cutSnapshot.card_paid)}</span>
          <span>Crédito {money(cutSnapshot.transfer_paid)}</span>
        </div>
      )}
      <div className="cash-layout">
        <div className="data-table">
          <div className="table-head sale-row">
            <span>Venta</span>
            <span>Cajero</span>
            <span>Total</span>
            <span>Pago</span>
            <span>Estado</span>
            <span />
          </div>
          {sales.map((sale) => (
            <div className="sale-row" key={sale.id}>
              <strong>{sale.folio}</strong>
              <span>{sale.cashier_name}</span>
              <span>{money(sale.total)}</span>
              <span>E {money(sale.cash_paid ?? 0)} / Tar {money(sale.card_paid ?? 0)} / Crédito {money(sale.transfer_paid ?? 0)}</span>
              <span>{sale.status === "paid" ? "Pagada" : "Cancelada"}</span>
              <button className="danger-button mini" type="button" disabled={sale.status !== "paid"} onClick={() => setCancelDraft(sale)}>
                Cancelar
              </button>
            </div>
          ))}
        </div>
        <aside className="inventory-side">
          <h3>Movimientos caja</h3>
          {movements.map((movement) => (
            <div className="kardex-row" key={movement.id}>
              <strong>{movement.movement_type === "in" ? "+" : movement.movement_type === "out" ? "-" : ""}{money(movement.amount)}</strong>
              <span>{movement.reason}</span>
              <small>{movement.actor_name}</small>
            </div>
          ))}
        </aside>
      </div>
      {cashDialog && (
        <CashDialog
          kind={cashDialog}
          expectedCash={expectedCash}
          totalSales={totalSales}
          tickets={tickets}
          onClose={() => setCashDialog(null)}
          onOpenCash={(amount) => {
            setCashDialog(null);
            openCash(amount);
          }}
          onMovement={addMovement}
          onAudit={cashAudit}
          onFinalCut={finalCut}
        />
      )}
      {cancelDraft && (
        <SaleCancelModal
          sale={cancelDraft}
          onClose={() => setCancelDraft(null)}
          onConfirm={(reason) => cancel(cancelDraft, reason)}
        />
      )}
    </section>
  );
}
