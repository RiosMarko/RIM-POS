import { AlertTriangle, Banknote, ReceiptText } from "lucide-react";
import { FormEvent, useState } from "react";
import { money } from "../../lib/money";
import type { SaleListItem } from "../../types";

const denominations = [1000, 500, 200, 100, 50, 20, 10, 5, 2, 1, 0.5];

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
  onClose: () => void;
  onOpenCash: (amount: number) => void;
  onMovement: (type: "in" | "out", amount: number, reason: string) => Promise<void>;
  onAudit: (counted: number, denominationsJson: string, differenceReason?: string) => void | Promise<void>;
  onFinalCut: (counted: number, denominationsJson: string, differenceReason?: string) => Promise<void>;
}) {
  const [amount, setAmount] = useState(kind === "open" ? "800" : String(expectedCash.toFixed(2)));
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
  const title = kind === "open" ? "Abrir caja" : kind === "in" ? "Entrada de efectivo" : kind === "out" ? "Retiro de efectivo" : kind === "audit" ? "Arqueo de caja" : "Corte Z final";
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
            <p>{tickets} tickets, vendido {money(totalSales)}, esperado {money(expectedCash)}</p>
          </div>
        </div>
        <form className="dialog-form" onSubmit={submit}>
          <label>
            {kind === "open" ? "Fondo inicial" : kind === "close" || kind === "audit" ? "Efectivo contado" : "Importe"}
            <input value={amount} onChange={(event) => setAmount(event.target.value)} inputMode="decimal" autoFocus />
          </label>
          {(kind === "audit" || kind === "close") && (
            <div className="denomination-grid">
              {denominations.map((value) => (
                <label key={value}>
                  ${value}
                  <input
                    value={counts[String(value)] ?? ""}
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
            <input value={amount} onChange={(event) => setAmount(event.target.value)} inputMode="decimal" />
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
