import { AlertTriangle, Banknote, ReceiptText } from "lucide-react";
import { FormEvent, useEffect, useState } from "react";
import { formatDateTimeMx } from "../../lib/date";
import { money } from "../../lib/money";
import { selectNumericInput } from "../../lib/numberInput";
import { getSaleDetail } from "../../lib/posApi";
import type { SaleListItem, SaleTicketDetail } from "../../types";

const denominations = [1000, 500, 200, 100, 50, 20, 10, 5, 2, 1, 0.5];

const paymentMethodLabel: Record<string, string> = {
  cash: "Efectivo",
  card: "Tarjeta",
  transfer: "Transferencia",
  voucher: "Vale",
  credit: "Credito",
};

export function SaleTicketModal({
  saleId,
  onClose,
  showToast,
}: {
  saleId: number;
  onClose: () => void;
  showToast: (message: string) => void;
}) {
  const [detail, setDetail] = useState<SaleTicketDetail | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getSaleDetail(saleId)
      .then((result) => {
        if (!cancelled) setDetail(result);
      })
      .catch((error) => {
        if (!cancelled) showToast(String(error));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [saleId, showToast]);

  return (
    <div className="modal-backdrop" role="presentation">
      <section className="ticket-name-modal sale-ticket-modal" role="dialog" aria-modal="true" aria-label={`Ticket ${detail?.folio ?? saleId}`}>
        <div className="modal-title">
          <ReceiptText size={22} />
          <div>
            <h2>Ticket {detail?.folio ?? ""}</h2>
            <p>{detail ? `${formatDateTimeMx(detail.created_at)} · ${detail.cashier_name} · ${detail.status === "paid" ? "Pagada" : "Cancelada"}` : "Cargando..."}</p>
          </div>
        </div>
        {loading ? (
          <div className="empty-state compact"><span>Cargando ticket</span></div>
        ) : !detail ? (
          <div className="empty-state compact"><span>No se pudo cargar el ticket</span></div>
        ) : (
          <>
            <div className="sale-ticket-items">
              {detail.items.length === 0 ? (
                <div className="muted-note">Sin articulos registrados</div>
              ) : detail.items.map((item, index) => {
                const returned = item.returned_quantity > 1e-6;
                return (
                  <div className="sale-ticket-item" key={`${item.product_name}-${index}`}>
                    <div>
                      <strong>{item.product_name}</strong>
                      {returned && <span className="return-badge">devuelto {item.returned_quantity} {item.unit}</span>}
                      <small>{item.quantity} {item.unit} x {money(item.unit_price)}{item.discount > 0 ? ` · desc ${money(item.discount)}` : ""}</small>
                    </div>
                    <strong className="money-cell">{money(item.line_total)}</strong>
                  </div>
                );
              })}
            </div>
            <div className="sale-ticket-totals">
              <div><span>Subtotal</span><strong>{money(detail.subtotal)}</strong></div>
              <div><span>Impuestos</span><strong>{money(detail.tax)}</strong></div>
              {detail.discount > 0 && <div><span>Descuento</span><strong>-{money(detail.discount)}</strong></div>}
              {Math.abs(detail.rounding) >= 0.005 && <div><span>Redondeo</span><strong>{money(detail.rounding)}</strong></div>}
              <div className="sale-ticket-total-row"><span>Total</span><strong>{money(detail.total)}</strong></div>
              <div><span>Pagado</span><strong>{money(detail.paid)}</strong></div>
              {detail.change_due > 0 && <div><span>Cambio</span><strong>{money(detail.change_due)}</strong></div>}
            </div>
            <div className="sale-ticket-payments">
              {detail.payments.map((payment, index) => {
                const label = paymentMethodLabel[payment.method] ?? payment.method;
                return (
                  <span className={`pay-tag pay-${label.toLowerCase()}`} key={index}>
                    {label}: {money(payment.amount)}
                    {payment.reference ? ` (${payment.reference})` : ""}
                  </span>
                );
              })}
            </div>
          </>
        )}
        <div className="modal-actions">
          <button className="ghost-button" type="button" onClick={onClose}>Cerrar</button>
        </div>
      </section>
    </div>
  );
}

export function SaleCancelModal({
  sale,
  onClose,
  onConfirm,
}: {
  sale: SaleListItem;
  onClose: () => void;
  onConfirm: (reason: string) => Promise<void>;
}) {
  const [reason, setReason] = useState("Cancelacion autorizada");
  const [busy, setBusy] = useState(false);
  return (
    <div className="modal-backdrop" role="presentation">
      <section className="ticket-name-modal" role="dialog" aria-modal="true" aria-label={`Cancelar ${sale.folio}`}>
        <div className="modal-title danger-title">
          <AlertTriangle size={24} />
          <div>
            <h2>Cancelar venta</h2>
            <p>{sale.folio} · {money(sale.total)}. Se restaura inventario y caja.</p>
          </div>
        </div>
        <form className="dialog-form" onSubmit={async (event) => {
          event.preventDefault();
          if (reason.trim().length < 3) return;
          setBusy(true);
          try {
            await onConfirm(reason.trim());
          } finally {
            setBusy(false);
          }
        }}>
          <label>
            Motivo
            <input value={reason} onChange={(event) => setReason(event.target.value)} autoFocus />
          </label>
          <div className="modal-actions">
            <button className="ghost-button" type="button" onClick={onClose} disabled={busy}>Conservar</button>
            <button className="danger-button" type="submit" disabled={busy || reason.trim().length < 3}>Cancelar venta</button>
          </div>
        </form>
      </section>
    </div>
  );
}

export function CashDialog({
  kind,
  expectedCash,
  totalSales,
  tickets,
  cashierName,
  onClose,
  onOpenCash,
  onMovement,
  onAudit,
  onFinalCut,
}: {
  kind: "open" | "in" | "out" | "audit" | "close";
  expectedCash: number;
  totalSales: number;
  tickets: number;
  cashierName: string;
  onClose: () => void;
  onOpenCash: (amount: number) => void;
  onMovement: (type: "in" | "out", amount: number, reason: string) => Promise<void>;
  onAudit: (counted: number, denominationsJson: string, differenceReason?: string) => void | Promise<void>;
  onFinalCut: (counted: number, denominationsJson: string, differenceReason?: string) => Promise<void>;
}) {
  const [amount, setAmount] = useState(kind === "open" ? "0" : String(expectedCash.toFixed(2)));
  const [reason, setReason] = useState(kind === "in" ? "Entrada a caja" : kind === "out" ? "Retiro de caja" : "");
  const [counts, setCounts] = useState<Record<string, string>>({});
  const amountValue = Number(amount.replace(",", "."));
  const denominationTotal = denominations.reduce((sum, value) => sum + value * (Number(counts[String(value)]) || 0), 0);
  const countedValue = (kind === "audit" || kind === "close") && Object.values(counts).some((value) => value.trim())
    ? denominationTotal
    : amountValue;
  const diff = Number.isFinite(countedValue) ? countedValue - expectedCash : 0;
  const needsReason = kind === "in" || kind === "out";
  const needsDifferenceReason = (kind === "audit" || kind === "close") && Math.round(diff * 100) !== 0;
  const title = kind === "open" ? "Abrir caja" : kind === "in" ? "Entrada de efectivo" : kind === "out" ? "Retiro de efectivo" : kind === "audit" ? "Arqueo de caja" : "Corte turno";
  const denominationsJson = JSON.stringify(denominations.map((value) => ({
    denomination: value,
    quantity: Number(counts[String(value)]) || 0,
    total: value * (Number(counts[String(value)]) || 0),
  })));

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!Number.isFinite(countedValue) || countedValue < 0 || (needsReason && reason.trim().length < 2) || (needsDifferenceReason && reason.trim().length < 2)) return;
    if (kind === "open") onOpenCash(amountValue);
    if (kind === "in" || kind === "out") await onMovement(kind, amountValue, reason.trim());
    if (kind === "audit") await onAudit(countedValue, denominationsJson, reason.trim());
    if (kind === "close") await onFinalCut(countedValue, denominationsJson, reason.trim());
  };

  return (
    <div className="modal-backdrop" role="presentation">
      <section className="ticket-name-modal" role="dialog" aria-modal="true" aria-label={title}>
        <div className="modal-title">
          <Banknote size={24} />
          <div>
            <h2>{title}</h2>
            <p>{cashierName} · {tickets} tickets, vendido {money(totalSales)}, esperado {money(expectedCash)}{kind === "close" ? " · Corte Z final" : ""}</p>
          </div>
        </div>
        <form className="dialog-form" onSubmit={submit}>
          <label>
            {kind === "open" ? "Fondo inicial" : kind === "close" || kind === "audit" ? "Efectivo contado" : "Importe"}
            <input value={amount} onFocus={selectNumericInput} onChange={(event) => setAmount(event.target.value)} inputMode="decimal" autoFocus />
          </label>
          {(kind === "audit" || kind === "close") && (
            <div className="denomination-grid">
              {denominations.map((value) => (
                <label key={value}>
                  ${value}
                  <input
                    value={counts[String(value)] ?? ""}
                    onFocus={selectNumericInput}
                    onChange={(event) => setCounts((current) => ({ ...current, [String(value)]: event.target.value }))}
                    inputMode="numeric"
                    placeholder="0"
                  />
                </label>
              ))}
            </div>
          )}
          {(needsReason || needsDifferenceReason) && (
            <label>
              {needsDifferenceReason ? "Motivo diferencia" : "Motivo"}
              <input value={reason} onChange={(event) => setReason(event.target.value)} />
            </label>
          )}
          {(kind === "audit" || kind === "close") && (
            <div className={diff === 0 ? "change-box" : "change-box warning"}>
              <span>Diferencia</span>
              <strong>{money(diff)}</strong>
            </div>
          )}
          <div className="modal-actions">
            <button className="ghost-button" type="button" onClick={onClose}>Cancelar</button>
            <button className={kind === "close" ? "danger-button" : "primary-button"} type="submit" disabled={!Number.isFinite(countedValue) || countedValue < 0 || (needsReason && reason.trim().length < 2) || (needsDifferenceReason && reason.trim().length < 2)}>
              {kind === "close" ? "Cerrar caja" : "Guardar"}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}

export function ExpenseDialog({
  onClose,
  onSave,
}: {
  onClose: () => void;
  onSave: (provider: string, amount: number) => Promise<void>;
}) {
  const [provider, setProvider] = useState("");
  const [amount, setAmount] = useState("");
  const [busy, setBusy] = useState(false);
  const amountValue = Number(amount.replace(",", "."));

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (provider.trim().length < 2 || !Number.isFinite(amountValue) || amountValue <= 0) return;
    setBusy(true);
    try {
      await onSave(provider.trim(), amountValue);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="modal-backdrop" role="presentation">
      <section className="ticket-name-modal" role="dialog" aria-modal="true" aria-label="Registrar gasto">
        <div className="modal-title">
          <ReceiptText size={24} />
          <div>
            <h2>Registrar gasto</h2>
            <p>Proveedor, compra o salida del dia. Se resta de caja y reportes.</p>
          </div>
        </div>
        <form className="dialog-form" onSubmit={submit}>
          <label>
            Proveedor o gasto
            <input value={provider} onChange={(event) => setProvider(event.target.value)} autoFocus />
          </label>
          <label>
            Importe
            <input value={amount} onFocus={selectNumericInput} onChange={(event) => setAmount(event.target.value)} inputMode="decimal" />
          </label>
          <div className="modal-actions">
            <button className="ghost-button" type="button" onClick={onClose} disabled={busy}>Cancelar</button>
            <button className="primary-button" type="submit" disabled={busy || provider.trim().length < 2 || !Number.isFinite(amountValue) || amountValue <= 0}>
              Guardar gasto
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}
