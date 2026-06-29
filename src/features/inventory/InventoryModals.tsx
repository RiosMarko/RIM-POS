import { Boxes } from "lucide-react";
import { FormEvent, useState } from "react";
import type { Product } from "../../types";

export function InventoryAdjustmentModal({
  product,
  onClose,
  onSave,
}: {
  product: Product;
  onClose: () => void;
  onSave: (quantity: number, reason: string) => Promise<void>;
}) {
  const [quantity, setQuantity] = useState(1);
  const [reason, setReason] = useState("Ajuste manual");
  const [busy, setBusy] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!Number.isFinite(quantity) || quantity === 0 || !reason.trim()) return;
    setBusy(true);
    try {
      await onSave(quantity, reason.trim());
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="modal-backdrop" role="presentation">
      <section className="ticket-name-modal" role="dialog" aria-modal="true" aria-label="Ajustar inventario">
        <div className="modal-title">
          <Boxes size={24} />
          <div>
            <h2>Ajustar inventario</h2>
            <p>{product.name} · actual {product.stock} {product.unit}</p>
          </div>
        </div>
        <form className="dialog-form" onSubmit={submit}>
          <label>
            Cantidad (+ entrada / - salida)
            <input
              type="number"
              step="1"
              value={quantity}
              onChange={(event) => setQuantity(Number(event.target.value))}
              autoFocus
            />
          </label>
          <label>
            Motivo
            <input value={reason} onChange={(event) => setReason(event.target.value)} />
          </label>
          <div className="modal-actions">
            <button className="ghost-button" type="button" onClick={onClose} disabled={busy}>Cancelar</button>
            <button className="primary-button" type="submit" disabled={busy || quantity === 0 || !reason.trim()}>Aplicar ajuste</button>
          </div>
        </form>
      </section>
    </div>
  );
}
