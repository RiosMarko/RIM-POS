import { BarChart3, Banknote, CalendarDays, FileText, Printer, ShoppingCart } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Metric } from "../../components/display/SummaryCards";
import type { ConfirmDraft } from "../../components/modals/CommonModals";
import { money } from "../../lib/money";
import {
  cancelSale,
  createCashCount,
  closeShiftCutZ,
  createCashMovement,
  getDailyCutSummary,
  getProductSalesReport,
  listAuditLog,
  listCashCounts,
  getShiftCutX,
  listShiftCuts,
  listCashMovements,
  listSales,
  printDailyCut,
  printShiftCut,
} from "../../lib/posApi";
import type { AuditLogEntry, CashCount, CashMovement, CashSession, DailyCutSummary, ProductSalesReport, SaleListItem, ShiftCutSnapshot, UserSession } from "../../types";
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
  const [counts, setCounts] = useState<CashCount[]>([]);
  const [cuts, setCuts] = useState<ShiftCutSnapshot[]>([]);
  const [auditLog, setAuditLog] = useState<AuditLogEntry[]>([]);
  const [sales, setSales] = useState<SaleListItem[]>([]);
  const [departmentRows, setDepartmentRows] = useState<ProductSalesReport[]>([]);
  const [cashDialog, setCashDialog] = useState<"open" | "in" | "out" | "audit" | "close" | null>(null);
  const [cutSnapshot, setCutSnapshot] = useState<ShiftCutSnapshot | null>(null);
  const [dailyCut, setDailyCut] = useState<DailyCutSummary | null>(null);
  const [cancelDraft, setCancelDraft] = useState<SaleListItem | null>(null);
  const paidSales = useMemo(() => sales.filter((sale) => sale.status === "paid"), [sales]);
  const cashSales = useMemo(() => paidSales.reduce((sum, sale) => sum + (sale.cash_paid ?? 0), 0), [paidSales]);
  const cardSales = useMemo(() => paidSales.reduce((sum, sale) => sum + (sale.card_paid ?? 0), 0), [paidSales]);
  const transferSales = useMemo(() => paidSales.reduce((sum, sale) => sum + (sale.transfer_paid ?? 0), 0), [paidSales]);
  const expectedCash = cashSession?.expected_cash ?? 0;
  const totalSales = cashSales + cardSales + transferSales;
  const cashEntries = useMemo(() => movements.filter((movement) => movement.movement_type === "in").reduce((sum, movement) => sum + movement.amount, 0), [movements]);
  const cashOut = useMemo(() => movements.filter((movement) => movement.movement_type === "out").reduce((sum, movement) => sum + movement.amount, 0), [movements]);
  const cashInDrawer = cutSnapshot?.closing_cash ?? ((cashSession?.opening_cash ?? 0) + cashSales + cashEntries - cashOut);
  const cutOwner = cutSnapshot?.opened_by_name || session.name;
  const cutStarted = cutSnapshot?.opened_at ?? cashSession?.opened_at ?? new Date().toISOString();
  const cutEnded = cutSnapshot?.closed_at ?? new Date().toISOString();
  const cutDifference = cutSnapshot ? cutSnapshot.cash_difference ?? ((cutSnapshot.closing_cash ?? cutSnapshot.expected_cash) - cutSnapshot.expected_cash) : 0;
  const cutCountedCash = cutSnapshot?.counted_cash ?? cutSnapshot?.closing_cash ?? null;
  const cutStatusLabel = cutSnapshot?.status === "closed" ? "Corte Z cerrado" : "Corte X consulta";
  const outMovements = useMemo(() => movements.filter((movement) => movement.movement_type === "out"), [movements]);
  const cutDifferenceClass = cutDifference === 0 ? "balanced" : cutDifference > 0 ? "positive" : "negative";
  const dailyDifferenceClass = !dailyCut || dailyCut.cash_difference === 0 ? "balanced" : dailyCut.cash_difference > 0 ? "positive" : "negative";
  const departmentTotals = useMemo(() => {
    const totals = new Map<string, number>();
    departmentRows.forEach((row) => {
      const key = row.category || "- Sin Departamento -";
      totals.set(key, (totals.get(key) ?? 0) + row.total);
    });
    return Array.from(totals, ([name, total]) => ({ name, total })).sort((left, right) => right.total - left.total);
  }, [departmentRows]);

  const refresh = useCallback(async () => {
    if (cashSession) {
      setMovements(await listCashMovements(cashSession.id));
      setCounts(await listCashCounts(cashSession.id));
      setDepartmentRows(await getProductSalesReport({
        fromDate: cashSession.opened_at.slice(0, 10),
        toDate: (cashSession.closed_at ?? new Date().toISOString()).slice(0, 10),
      }));
    } else {
      setMovements([]);
      setCounts([]);
      setDepartmentRows([]);
    }
    setCuts(await listShiftCuts());
    if (session.role === "admin") setAuditLog(await listAuditLog());
    setSales(await listSales());
  }, [cashSession, session.role]);

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

  const cashAudit = async (counted: number, denominationsJson: string, differenceReason?: string) => {
    if (!cashSession) return;
    const diff = counted - expectedCash;
    await createCashCount({
      session_id: cashSession.id,
      shift_id: cutSnapshot?.shift_id ?? null,
      count_type: "audit",
      expected_cash: expectedCash,
      counted_cash: counted,
      denominations_json: denominationsJson,
      difference_reason: differenceReason || null,
      actor_id: session.id,
    });
    await refresh();
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

  const finalCut = async (counted: number, denominationsJson: string, differenceReason?: string) => {
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
          const snapshot = await closeShiftCutZ({
            shift_id: preview.shift_id,
            closing_cash: counted,
            closed_by: session.id,
            denominations_json: denominationsJson,
            difference_reason: differenceReason || null,
          });
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

  const generalCut = async () => {
    try {
      const summary = await getDailyCutSummary();
      setDailyCut(summary);
      showToast(`Corte general: ${summary.cut_count} turnos, ${money(summary.net_sales)}`);
    } catch (error) {
      showToast(String(error));
    }
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
        <button
          className="ghost-button"
          type="button"
          disabled={!cashSession}
          title="Corte X: consulta del turno actual, no cierra caja"
          onClick={partialCut}
        >
          Corte cajero
        </button>
        <button
          className="danger-button"
          type="button"
          disabled={!cashSession}
          title="Corte Z: cierre final del turno, cierra caja"
          onClick={() => setCashDialog("close")}
        >
          Corte del dia
        </button>
        <button
          className="ghost-button"
          type="button"
          title="Suma todos los Cortes Z cerrados hoy. No modifica turnos."
          onClick={generalCut}
        >
          Corte general
        </button>
      </div>
      {dailyCut && (
        <div className="cut-detail daily-cut-detail">
          <div className="cut-detail-head">
            <div>
              <span className="cut-status-pill">Resumen final del dia</span>
              <h2>Corte general {new Date(`${dailyCut.date}T00:00:00`).toLocaleDateString("es-MX", { day: "2-digit", month: "short", year: "numeric" })}</h2>
              <p>{dailyCut.cut_count} turnos cerrados, no modifica cajas ni ventas.</p>
            </div>
            <button
              className="ghost-button"
              type="button"
              disabled={dailyCut.cut_count === 0}
              onClick={() => printDailyCut(dailyCut.date).then((result) => showToast(result.message)).catch((error) => showToast(String(error)))}
            >
              <Printer size={16} />
              Imprimir general
            </button>
          </div>
          <div className="cut-kpi-grid">
            <div className="cut-kpi primary">
              <span>Ventas del dia</span>
              <strong>{money(dailyCut.net_sales)}</strong>
              <small>{dailyCut.total_tickets} tickets</small>
            </div>
            <div className="cut-kpi">
              <span>Turnos cerrados</span>
              <strong>{dailyCut.cut_count}</strong>
              <small>Cortes Z incluidos</small>
            </div>
            <div className="cut-kpi">
              <span>Efectivo contado</span>
              <strong>{money(dailyCut.counted_cash)}</strong>
              <small>sumado de cierres</small>
            </div>
            <div className={`cut-kpi difference ${dailyDifferenceClass}`}>
              <span>Diferencia total</span>
              <strong>{money(dailyCut.cash_difference)}</strong>
              <small>{dailyCut.cash_difference === 0 ? "cuadra" : "revisar cortes"}</small>
            </div>
          </div>
          <div className="cut-detail-grid">
            <div className="cut-detail-section cut-cash-card">
              <h3>Efectivo consolidado <strong>{money(dailyCut.expected_cash)}</strong></h3>
              <div className="cut-line"><span>Fondos iniciales</span><strong>{money(dailyCut.opening_cash)}</strong></div>
              <div className="cut-line positive"><span>Ventas efectivo</span><strong>{money(dailyCut.cash_paid)}</strong></div>
              <div className="cut-line"><span>Esperado</span><strong>{money(dailyCut.expected_cash)}</strong></div>
              <div className="cut-line total"><span>Contado final</span><strong>{money(dailyCut.counted_cash)}</strong></div>
            </div>
            <div className="cut-detail-section">
              <h3>Ventas consolidadas</h3>
              <div className="cut-payment-grid">
                <div><Banknote size={18} /><span>Efectivo</span><strong>{money(dailyCut.cash_paid)}</strong></div>
                <div><FileText size={18} /><span>Tarjeta</span><strong>{money(dailyCut.card_paid)}</strong></div>
                <div><CalendarDays size={18} /><span>Credito</span><strong>{money(dailyCut.transfer_paid)}</strong></div>
              </div>
              <div className="cut-line total"><span>Total vendido</span><strong>{money(dailyCut.net_sales)}</strong></div>
              <div className="cut-line"><span>Ticket promedio</span><strong>{money(dailyCut.average_ticket)}</strong></div>
              <div className="cut-line"><span>Impuestos</span><strong>{money(dailyCut.tax)}</strong></div>
            </div>
            <div className="cut-detail-section daily-cut-list">
              <h3>Cortes incluidos</h3>
              {dailyCut.cuts.length === 0 ? (
                <span className="muted-note">Sin Cortes Z cerrados hoy</span>
              ) : dailyCut.cuts.map((cut) => (
                <div className="cut-line" key={cut.shift_id}>
                  <span>#{cut.shift_id} {cut.closed_by_name || cut.opened_by_name || "sin cajero"}</span>
                  <strong>{money(cut.net_sales)}</strong>
                </div>
              ))}
            </div>
            <div className="cut-detail-section">
              <h3>Resumen</h3>
              <div className="cut-line"><span>Cancelados</span><strong>{dailyCut.canceled_tickets}</strong></div>
              <div className="cut-line"><span>Descuentos</span><strong>{money(dailyCut.discount)}</strong></div>
              <div className={`cut-line ${dailyDifferenceClass}`}><span>Diferencia</span><strong>{money(dailyCut.cash_difference)}</strong></div>
            </div>
          </div>
        </div>
      )}
      {cutSnapshot && (
        <div className="cut-detail">
          <div className="cut-detail-head">
            <div>
              <span className="cut-status-pill">{cutStatusLabel}</span>
              <h2>Corte de {cutOwner}</h2>
              <p>
                {new Date(cutStarted).toLocaleDateString("es-MX", { day: "2-digit", month: "short", year: "numeric" })}
                {" de "}
                {new Date(cutStarted).toLocaleTimeString("es-MX", { hour: "2-digit", minute: "2-digit" })}
                {" a "}
                {new Date(cutEnded).toLocaleTimeString("es-MX", { hour: "2-digit", minute: "2-digit" })}
                {cutSnapshot.closed_by_name ? `, cerrado por ${cutSnapshot.closed_by_name}` : ", turno actual"}
              </p>
            </div>
            <button
              className="ghost-button"
              type="button"
              onClick={() => printShiftCut(cutSnapshot.shift_id).then((result) => showToast(result.message)).catch((error) => showToast(String(error)))}
            >
              <Printer size={16} />
              Imprimir corte
            </button>
          </div>
          <div className="cut-kpi-grid">
            <div className="cut-kpi primary">
              <span>Ventas netas</span>
              <strong>{money(cutSnapshot.net_sales)}</strong>
              <small>{cutSnapshot.total_tickets} tickets</small>
            </div>
            <div className="cut-kpi">
              <span>Efectivo esperado</span>
              <strong>{money(cutSnapshot.expected_cash)}</strong>
              <small>fondo + efectivo + entradas - salidas</small>
            </div>
            <div className="cut-kpi">
              <span>{cutCountedCash == null ? "Conteo pendiente" : "Efectivo contado"}</span>
              <strong>{cutCountedCash == null ? "-" : money(cutCountedCash)}</strong>
              <small>{cutSnapshot.status === "closed" ? "registrado al cerrar" : "sin cerrar caja"}</small>
            </div>
            <div className={`cut-kpi difference ${cutDifferenceClass}`}>
              <span>Diferencia</span>
              <strong>{money(cutDifference)}</strong>
              <small>{cutDifference === 0 ? "cuadra" : cutSnapshot.difference_reason || "requiere revision"}</small>
            </div>
          </div>
          <div className="cut-detail-grid">
            <div className="cut-detail-section cut-cash-card">
              <h3>Formula de caja <strong>{money(cashInDrawer)}</strong></h3>
              <div className="cut-line"><span>Fondo inicial</span><strong>{money(cutSnapshot.opening_cash)}</strong></div>
              <div className="cut-line positive"><span>Ventas en efectivo</span><strong>{money(cutSnapshot.cash_paid)}</strong></div>
              <div className="cut-line positive"><span>Entradas</span><strong>{money(cashEntries)}</strong></div>
              <div className="cut-line negative"><span>Salidas</span><strong>-{money(cashOut)}</strong></div>
              <div className="cut-line total"><span>Dinero en caja</span><strong>{money(cashInDrawer)}</strong></div>
            </div>
            <div className="cut-detail-section">
              <h3>Ventas por forma de pago</h3>
              <div className="cut-payment-grid">
                <div><Banknote size={18} /><span>Efectivo</span><strong>{money(cutSnapshot.cash_paid)}</strong></div>
                <div><FileText size={18} /><span>Tarjeta</span><strong>{money(cutSnapshot.card_paid)}</strong></div>
                <div><CalendarDays size={18} /><span>Credito</span><strong>{money(cutSnapshot.transfer_paid)}</strong></div>
              </div>
              <div className="cut-line total"><span>Total vendido</span><strong>{money(cutSnapshot.net_sales)}</strong></div>
              <div className="cut-line"><span>Ticket promedio</span><strong>{money(cutSnapshot.average_ticket)}</strong></div>
              <div className="cut-line"><span>Impuestos</span><strong>{money(cutSnapshot.tax)}</strong></div>
            </div>
            <div className="cut-detail-section">
              <h3>Ventas por departamento</h3>
              {departmentTotals.length === 0 ? (
                <span className="muted-note">Sin departamentos</span>
              ) : departmentTotals.slice(0, 12).map((department) => (
                <div className="cut-line" key={department.name}><span>{department.name}</span><strong>{money(department.total)}</strong></div>
              ))}
            </div>
            <div className="cut-detail-section">
              <h3>Salidas de efectivo</h3>
              {outMovements.length === 0 ? (
                <span className="muted-note">No hubo salidas</span>
              ) : outMovements.slice(0, 8).map((movement) => (
                <div className="cut-line" key={movement.id}><span>{new Date(movement.created_at).toLocaleTimeString("es-MX", { hour: "2-digit", minute: "2-digit" })} {movement.reason}</span><strong>{money(movement.amount)}</strong></div>
              ))}
              <h3>Resumen</h3>
              <div className="cut-line"><span>Tickets</span><strong>{cutSnapshot.total_tickets}</strong></div>
              <div className="cut-line"><span>Cancelados</span><strong>{cutSnapshot.canceled_tickets}</strong></div>
              <div className={`cut-line ${cutDifferenceClass}`}><span>Diferencia</span><strong>{money(cutDifference)}</strong></div>
            </div>
          </div>
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
          <h3>Arqueos</h3>
          {counts.map((count) => (
            <div className="kardex-row" key={count.id}>
              <strong>{count.count_type === "close" ? "Cierre" : "Arqueo"} {money(count.counted_cash)}</strong>
              <span>Diferencia {money(count.difference)}</span>
              <small>{count.difference_reason || count.actor_name}</small>
            </div>
          ))}
          <h3>Cortes</h3>
          {cuts.map((cut) => (
            <div className="kardex-row" key={cut.shift_id}>
              <strong title={cut.status === "closed" ? "Corte Z" : "Corte X"}>{cut.status === "closed" ? "Corte del dia" : "Corte cajero"} #{cut.shift_id}</strong>
              <span>{money(cut.net_sales)} · dif {money(cut.cash_difference ?? 0)}</span>
              <button className="ghost-button mini" type="button" onClick={() => printShiftCut(cut.shift_id).then((result) => showToast(result.message)).catch((error) => showToast(String(error)))}>
                Reimprimir
              </button>
            </div>
          ))}
          {auditLog.length > 0 && <h3>Bitacora</h3>}
          {auditLog.slice(0, 8).map((entry) => (
            <div className="kardex-row" key={entry.id}>
              <strong>{entry.action}</strong>
              <span>{entry.entity} {entry.entity_id ?? ""}</span>
              <small>{entry.actor_name ?? "Sistema"}</small>
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
          cashierName={session.name}
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
