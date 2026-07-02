import { Archive, Ticket, Trash2 } from "lucide-react";
import { FormEvent, useEffect, useRef, useState } from "react";
import { money } from "../../lib/money";
import type { ActiveSaleDraft, HeldTicket } from "../../types";

export const functionKeys = [
  { key: "F1", label: "Ticket" },
  { key: "F2", label: "No ticket" },
  { key: "F3", label: "Productos", compactLabel: true },
  { key: "F4", label: "Inventario", compactLabel: true },
  { key: "F5", label: "Cliente" },
  { key: "F6", label: "Dejar abierto" },
  { key: "F7", label: "Quitar" },
  { key: "F8", label: "Gasto" },
  { key: "F9", label: "Pago" },
  { key: "F10", label: "Cajon" },
  { key: "F11", label: "Corte", compactLabel: true },
  { key: "F12", label: "Admin" },
];

export function ShortcutHelp({ onClose }: { onClose: () => void }) {
  return (
    <div className="modal-backdrop" role="presentation">
      <section className="shortcut-modal" role="dialog" aria-modal="true" aria-label="Funciones rapidas">
        <div className="module-toolbar">
          <div>
            <h2>Funciones F1-F12</h2>
            <p>Accesos rapidos para vender sin mouse.</p>
          </div>
          <button className="ghost-button" type="button" onClick={onClose}>
            Cerrar
          </button>
        </div>
        <div className="function-key-grid large">
          {functionKeys.map((item) => (
            <div className={item.compactLabel ? "function-key compact-label" : "function-key"} key={item.key}>
              <strong>{item.key}</strong>
              <span>{item.label}</span>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}

export function HeldTicketsModal({
  tickets,
  onClose,
  onRecover,
  onDelete,
}: {
  tickets: HeldTicket[];
  onClose: () => void;
  onRecover: (ticket: HeldTicket) => void;
  onDelete: (ticket: HeldTicket) => void;
}) {
  return (
    <div className="modal-backdrop" role="presentation">
      <section className="held-ticket-modal" role="dialog" aria-modal="true" aria-label="Tickets abiertos">
        <div className="module-toolbar">
          <div>
            <h2>Tickets abiertos</h2>
            <p>{tickets.length} ventas en espera</p>
          </div>
          <button className="ghost-button" type="button" onClick={onClose}>
            Cerrar
          </button>
        </div>
        <div className="held-ticket-list">
          {tickets.length === 0 ? (
            <div className="empty-state compact">
              <Ticket size={28} />
              <strong>Sin tickets abiertos</strong>
              <span>Deja una venta abierta para recuperarla despues.</span>
            </div>
          ) : (
            tickets.map((ticket) => (
              <div className="held-ticket-row" key={ticket.id}>
                <div>
                  <strong>{ticket.name}</strong>
                  <span>{ticket.item_count} articulos, {ticket.cashier_name}</span>
                </div>
                <strong className="money-cell">{money(ticket.total)}</strong>
                <button className="primary-button" type="button" onClick={() => onRecover(ticket)}>
                  Recuperar
                </button>
                <button className="icon-button danger" type="button" aria-label={`Eliminar ${ticket.name}`} onClick={() => onDelete(ticket)}>
                  <Trash2 size={18} />
                </button>
              </div>
            ))
          )}
        </div>
      </section>
    </div>
  );
}

export function TicketNameModal({
  initialName,
  onClose,
  onSave,
}: {
  initialName: string;
  onClose: () => void;
  onSave: (name: string) => void;
}) {
  const [name, setName] = useState(initialName);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    window.setTimeout(() => inputRef.current?.focus(), 40);
  }, []);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    onSave(name);
  };

  return (
    <div className="modal-backdrop" role="presentation">
      <section className="ticket-name-modal" role="dialog" aria-modal="true" aria-label="Nombrar ticket abierto">
        <div className="modal-title">
          <Ticket size={22} />
          <div>
            <h2>Nombre del ticket</h2>
            <p>Usa nombre corto: cliente, mesa o referencia.</p>
          </div>
        </div>
        <form className="dialog-form" onSubmit={submit}>
          <label>
            Nombre
            <input
              ref={inputRef}
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="Ej. Juan, Mesa 2, Pedido pan"
            />
          </label>
          <div className="modal-actions">
            <button className="ghost-button" type="button" onClick={onClose}>
              Cancelar
            </button>
            <button className="primary-button" type="submit" disabled={name.trim().length < 2}>
              Guardar ticket
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}

export function DeleteHeldTicketModal({
  ticket,
  onCancel,
  onConfirm,
}: {
  ticket: HeldTicket;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <div className="modal-backdrop" role="presentation">
      <section className="ticket-name-modal" role="dialog" aria-modal="true" aria-label="Eliminar ticket abierto">
        <div className="modal-title danger-title">
          <Trash2 size={22} />
          <div>
            <h2>Eliminar ticket abierto</h2>
            <p>Esta accion no se puede deshacer.</p>
          </div>
        </div>
        <div className="delete-ticket-summary">
          <strong>{ticket.name}</strong>
          <span>{ticket.item_count} articulos, {money(ticket.total)}</span>
        </div>
        <div className="modal-actions">
          <button className="ghost-button" type="button" onClick={onCancel}>
            Conservar
          </button>
          <button className="danger-button" type="button" onClick={onConfirm}>
            Eliminar ticket
          </button>
        </div>
      </section>
    </div>
  );
}

export function RecoveryDraftModal({
  draft,
  onRecover,
  onDiscard,
}: {
  draft: ActiveSaleDraft;
  onRecover: () => void;
  onDiscard: () => void;
}) {
  const paid = draft.cash_received + draft.card_received + draft.transfer_received;
  return (
    <div className="modal-backdrop" role="presentation">
      <section className="ticket-name-modal" role="dialog" aria-modal="true" aria-label="Recuperar venta pendiente">
        <div className="modal-title">
          <Archive size={22} />
          <div>
            <h2>Recuperar venta pendiente</h2>
            <p>Hay carrito guardado de cierre inesperado.</p>
          </div>
        </div>
        <div className="recovery-summary">
          <div>
            <span>Articulos</span>
            <strong>{draft.item_count}</strong>
          </div>
          <div>
            <span>Total</span>
            <strong>{money(draft.total)}</strong>
          </div>
          <div>
            <span>Pago capturado</span>
            <strong>{money(paid)}</strong>
          </div>
          <div>
            <span>Guardado</span>
            <strong>{new Date(draft.updated_at).toLocaleString("es-MX", { dateStyle: "short", timeStyle: "short" })}</strong>
          </div>
        </div>
        <div className="modal-actions">
          <button className="ghost-button" type="button" onClick={onDiscard}>
            Descartar
          </button>
          <button className="primary-button" type="button" onClick={onRecover}>
            Recuperar
          </button>
        </div>
      </section>
    </div>
  );
}
