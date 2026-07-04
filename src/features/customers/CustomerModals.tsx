import { CircleDollarSign } from "lucide-react";
import { FormEvent, useState } from "react";
import { money } from "../../lib/money";
import { selectNumericInput } from "../../lib/numberInput";
import type { Customer } from "../../types";

export type CustomerCreditDraft = {
  customer: Customer;
  mode: "charge" | "payment";
  initialAmount?: number;
  reason?: string;
  paymentMethod?: "cash" | "card" | "transfer";
};

export function CustomerCreditModal({
  draft,
  onClose,
  onSave,
}: {
  draft: CustomerCreditDraft;
  onClose: () => void;
  onSave: (amount: number, reason: string, paymentMethod?: "cash" | "card" | "transfer") => Promise<void>;
}) {
  const [amount, setAmount] = useState(String(draft.initialAmount ?? 100));
  const [reason, setReason] = useState(draft.reason ?? (draft.mode === "charge" ? "Cargo a credito" : "Abono"));
  const [paymentMethod, setPaymentMethod] = useState<"cash" | "card" | "transfer">(draft.paymentMethod ?? "cash");
  const [busy, setBusy] = useState(false);
  const isCharge = draft.mode === "charge";
  const available = Math.max(0, draft.customer.credit_limit - draft.customer.balance);
  const amountValue = Number(amount.replace(",", "."));

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!Number.isFinite(amountValue) || amountValue <= 0 || !reason.trim()) return;
    setBusy(true);
    try {
      await onSave(amountValue, reason.trim(), isCharge ? undefined : paymentMethod);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="modal-backdrop" role="presentation">
      <section className="ticket-name-modal" role="dialog" aria-modal="true" aria-label={isCharge ? "Cargo a credito" : "Abono a cliente"}>
        <div className="modal-title">
          <CircleDollarSign size={24} />
          <div>
            <h2>{isCharge ? "Cargo a credito" : "Abono"}</h2>
            <p>{draft.customer.name} · saldo {money(draft.customer.balance)} · disponible {money(available)}</p>
          </div>
        </div>
        <form className="dialog-form" onSubmit={submit}>
          <label>
            Importe
            <input
              type="text"
              inputMode="decimal"
              value={amount}
              onFocus={selectNumericInput}
              onChange={(event) => setAmount(event.target.value)}
              autoFocus
            />
          </label>
          <label>
            Motivo
            <input value={reason} onChange={(event) => setReason(event.target.value)} />
          </label>
          {!isCharge && (
            <label>
              Forma de pago
              <select value={paymentMethod} onChange={(event) => setPaymentMethod(event.target.value as "cash" | "card" | "transfer")}>
                <option value="cash">Efectivo</option>
                <option value="card">Tarjeta</option>
                <option value="transfer">Transferencia</option>
              </select>
            </label>
          )}
          <div className="modal-actions">
            <button className="ghost-button" type="button" onClick={onClose} disabled={busy}>Cancelar</button>
            <button className="primary-button" type="submit" disabled={busy || !Number.isFinite(amountValue) || amountValue <= 0 || !reason.trim()}>{isCharge ? "Aplicar cargo" : "Aplicar abono"}</button>
          </div>
        </form>
      </section>
    </div>
  );
}
