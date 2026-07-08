import { AlertTriangle, History, RotateCcw, Search } from "lucide-react";
import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { AdminGate } from "../auth/AuthScreens";
import { SaleTicketModal } from "../cash/CashModals";
import { formatTimeMx } from "../../lib/date";
import { money } from "../../lib/money";
import { selectNumericInput } from "../../lib/numberInput";
import { listCashierOptions, listSaleLineHistory, returnSaleItem } from "../../lib/posApi";
import type { CashierOption, SaleLineHistory } from "../../types";

function todayIso() {
  const now = new Date();
  const local = new Date(now.getTime() - now.getTimezoneOffset() * 60000);
  return local.toISOString().slice(0, 10);
}

function remainingQty(line: SaleLineHistory) {
  return Math.max(0, line.quantity - line.returned_quantity);
}

type ReturnDraft = { line: SaleLineHistory; quantity: string; reason: string };

function ReturnItemModal({
  line,
  onCancel,
  onConfirm,
}: {
  line: SaleLineHistory;
  onCancel: () => void;
  onConfirm: (quantity: number, reason: string) => void;
}) {
  const remaining = remainingQty(line);
  const unitEffective = line.quantity > 0 ? line.line_total / line.quantity : 0;
  const [quantity, setQuantity] = useState(String(remaining));
  const [reason, setReason] = useState("Devolucion");
  const qty = Number(quantity.replace(",", "."));
  const valid = Number.isFinite(qty) && qty > 0 && qty <= remaining + 1e-6 && reason.trim().length >= 2;
  const refund = valid ? money(unitEffective * qty) : money(0);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (valid) onConfirm(qty, reason.trim());
  };

  return (
    <div className="modal-backdrop" role="presentation">
      <section className="ticket-name-modal" role="dialog" aria-modal="true" aria-label={`Devolver ${line.product_name}`}>
        <div className="modal-title danger-title">
          <AlertTriangle size={22} />
          <div>
            <h2>Devolver {line.product_name}</h2>
            <p>Ticket {line.folio} · vendido {line.quantity} {line.unit} · quedan {remaining} {line.unit}</p>
          </div>
        </div>
        <form className="dialog-form" onSubmit={submit}>
          <label>
            Cantidad a devolver ({line.unit})
            <input
              value={quantity}
              inputMode="decimal"
              onFocus={selectNumericInput}
              onChange={(event) => setQuantity(event.target.value)}
              autoFocus
            />
          </label>
          <label>
            Motivo
            <input value={reason} onChange={(event) => setReason(event.target.value)} />
          </label>
          <div className="weight-prompt-total">
            <span>Reembolso</span>
            <strong>{refund}</strong>
          </div>
          <div className="modal-actions">
            <button className="ghost-button" type="button" onClick={onCancel}>Cancelar</button>
            <button className="danger-button" type="submit" disabled={!valid}>Devolver</button>
          </div>
        </form>
      </section>
    </div>
  );
}

export function HistoryReturnsView({
  showToast,
}: {
  showToast: (message: string) => void;
}) {
  const [day, setDay] = useState(todayIso());
  const [cashierFilter, setCashierFilter] = useState<number | "all">("all");
  const [lines, setLines] = useState<SaleLineHistory[]>([]);
  const [cashiers, setCashiers] = useState<CashierOption[]>([]);
  const [loading, setLoading] = useState(true);
  const [productQuery, setProductQuery] = useState("");
  const [returnDraft, setReturnDraft] = useState<ReturnDraft | null>(null);
  const [returnAdminDraft, setReturnAdminDraft] = useState<ReturnDraft | null>(null);
  const [ticketSaleId, setTicketSaleId] = useState<number | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setLines(await listSaleLineHistory({ day }));
    } catch (error) {
      showToast(String(error));
    } finally {
      setLoading(false);
    }
  }, [day, showToast]);

  useEffect(() => {
    load();
  }, [load]);

  useEffect(() => {
    // Every active cashier, not just those who sold on the selected day.
    listCashierOptions().catch((error) => showToast(String(error))).then((result) => {
      if (result) setCashiers(result);
    });
  }, [showToast]);

  const visibleLines = useMemo(() => {
    const term = productQuery.trim().toLowerCase();
    return lines.filter((line) => {
      if (cashierFilter !== "all" && line.cashier_id !== cashierFilter) return false;
      if (term && !(line.product_name.toLowerCase().includes(term) || line.folio.toLowerCase().includes(term))) return false;
      return true;
    });
  }, [lines, cashierFilter, productQuery]);

  const totals = useMemo(() => {
    return visibleLines.reduce(
      (acc, line) => {
        acc.items += 1;
        acc.amount += line.line_total;
        return acc;
      },
      { items: 0, amount: 0 },
    );
  }, [visibleLines]);

  const doReturn = useCallback(async (draft: ReturnDraft, quantity: number, actorId: number) => {
    try {
      await returnSaleItem({ sale_item_id: draft.line.sale_item_id, quantity, actor_id: actorId, reason: draft.reason });
      showToast(`Devolucion registrada: ${draft.line.product_name}`);
      await load();
    } catch (error) {
      showToast(String(error));
    }
  }, [load, showToast]);

  return (
    <section className="history-view">
      <header className="module-toolbar">
        <div>
          <h2>Historial y Devoluciones</h2>
          <p>Que se vendio, en que ticket y quien lo cobro. Devuelve un articulo directo desde la lista.</p>
        </div>
      </header>

      <div className="history-filters">
        <label className="field-label">
          Fecha
          <input type="date" value={day} max={todayIso()} onChange={(event) => setDay(event.target.value)} />
        </label>
        <label className="field-label">
          Cajero
          <select value={cashierFilter} onChange={(event) => setCashierFilter(event.target.value === "all" ? "all" : Number(event.target.value))}>
            <option value="all">Todos los cajeros</option>
            {cashiers.map((cashier) => (
              <option value={cashier.id} key={cashier.id}>{cashier.name}</option>
            ))}
          </select>
        </label>
        <div className="search-row inline history-search">
          <Search size={18} />
          <input
            value={productQuery}
            onChange={(event) => setProductQuery(event.target.value)}
            placeholder="Busca producto o folio"
          />
        </div>
        <div className="history-totals">
          <span>{totals.items} articulos</span>
          <strong>{money(totals.amount)}</strong>
        </div>
      </div>

      <div className="data-table">
        <div className="table-head history-row">
          <span>Producto</span>
          <span>Cant.</span>
          <span>Precio</span>
          <span>Total</span>
          <span>Ticket</span>
          <span>Pago</span>
          <span>Hora</span>
          <span>Cajero</span>
          <span />
        </div>
        {loading ? (
          <div className="empty-state compact"><History size={28} /><strong>Cargando</strong></div>
        ) : visibleLines.length === 0 ? (
          <div className="empty-state compact">
            <History size={28} />
            <strong>Sin ventas ese dia</strong>
            <span>Cambia la fecha o el cajero.</span>
          </div>
        ) : (
          visibleLines.map((line) => {
            const remaining = remainingQty(line);
            const returned = line.returned_quantity > 1e-6;
            const canReturn = line.returnable && remaining > 1e-6;
            return (
              <div className="history-row" key={line.sale_item_id}>
                <strong>
                  {line.product_name}
                  {returned && <span className="return-badge">devuelto {line.returned_quantity} {line.unit}</span>}
                </strong>
                <span>{line.quantity} {line.unit}</span>
                <span>{money(line.unit_price)}</span>
                <strong className="money-cell">{money(line.line_total)}</strong>
                <span className="ticket-cell">{line.folio}</span>
                <span className={`pay-tag pay-${line.payment_method.toLowerCase()}`}>{line.payment_method}</span>
                <span>{formatTimeMx(line.created_at)}</span>
                <span>{line.cashier_name}</span>
                <div className="history-row-actions">
                  <button
                    className="ghost-button mini"
                    type="button"
                    onClick={() => setTicketSaleId(line.sale_id)}
                  >
                    Ver ticket
                  </button>
                  <button
                    className="danger-button mini"
                    type="button"
                    disabled={!canReturn}
                    title={
                      !line.returnable
                        ? "Turno cerrado: registra la devolucion en el turno actual"
                        : remaining <= 1e-6
                          ? "Articulo ya devuelto por completo"
                          : "Devolver este articulo"
                    }
                    onClick={() => setReturnDraft({ line, quantity: String(remaining), reason: "Devolucion" })}
                  >
                    <RotateCcw size={12} />
                    Devolver
                  </button>
                </div>
              </div>
            );
          })
        )}
      </div>

      {ticketSaleId !== null && (
        <SaleTicketModal
          saleId={ticketSaleId}
          onClose={() => setTicketSaleId(null)}
          showToast={showToast}
        />
      )}

      {returnDraft && (
        <ReturnItemModal
          line={returnDraft.line}
          onCancel={() => setReturnDraft(null)}
          onConfirm={(quantity, reason) => {
            setReturnAdminDraft({ line: returnDraft.line, quantity: String(quantity), reason });
            setReturnDraft(null);
          }}
        />
      )}
      {returnAdminDraft && (
        <AdminGate
          targetLabel="devolver articulo"
          onCancel={() => setReturnAdminDraft(null)}
          onSuccess={(adminSession) => {
            const draft = returnAdminDraft;
            setReturnAdminDraft(null);
            doReturn(draft, Number(draft.quantity), adminSession.id);
          }}
          showToast={showToast}
        />
      )}
    </section>
  );
}
