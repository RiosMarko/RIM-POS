import { CircleDollarSign } from "lucide-react";
import { FormEvent, useState } from "react";
import { money } from "../../lib/money";
import type { Customer } from "../../types";

export type CustomerCreditDraft = {
  customer: Customer;
  mode: "charge" | "payment";
  initialAmount?: number;
  reason?: string;
};

export function CustomerCreditModal({
  draft,
  onClose,
  onSave,
}: {
  draft: CustomerCreditDraft;
  onClose: () => void;
  onSave: (amount: number, reason: string) => Promise<void>;
}) {
  const [amount, setAmount] = useState(String(draft.initialAmount ?? 100));
  const [reason, setReason] = useState(draft.reason ?? (draft.mode === "charge" ? "Cargo a credito" : "Abono"));
  const [busy, setBusy] = useState(false);
  const isCharge = draft.mode === "charge";
  const available = Math.max(0, draft.customer.credit_limit - draft.customer.balance);
  const amountValue = Number(amount.replace(",", "."));

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!Number.isFinite(amountValue) || amountValue <= 0 || !reason.trim()) return;
    setBusy(true);
    try {
      await onSave(amountValue, reason.trim());
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
              onChange={(event) => setAmount(event.target.value)}
              autoFocus
            />
          </label>
          <label>
            Motivo
            <input value={reason} onChange={(event) => setReason(event.target.value)} />
          </label>
          <div className="modal-actions">
            <button className="ghost-button" type="button" onClick={onClose} disabled={busy}>Cancelar</button>
            <button className="primary-button" type="submit" disabled={busy || !Number.isFinite(amountValue) || amountValue <= 0 || !reason.trim()}>{isCharge ? "Aplicar cargo" : "Aplicar abono"}</button>
          </div>
        </form>
      </section>
    </div>
  );
}
