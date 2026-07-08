import { AlertTriangle, Archive, PackagePlus, Scale, Ticket, Trash2 } from "lucide-react";
import { FormEvent, useEffect, useRef, useState } from "react";
import { formatDateTimeMx } from "../../lib/date";
import { money } from "../../lib/money";
import { selectNumericInput } from "../../lib/numberInput";
import { listTaxes } from "../../lib/posApi";
import type { ActiveSaleDraft, HeldTicket, Product, TaxOption } from "../../types";

export const functionKeys = [
  { key: "F1", label: "Ticket", title: "Cobrar e imprimir ticket" },
  { key: "F2", label: "No ticket", title: "Cobrar sin imprimir ticket" },
  { key: "F3", label: "Productos", compactLabel: true, title: "Abrir Productos" },
  { key: "F4", label: "Inventario", compactLabel: true, title: "Abrir Inventario" },
  { key: "F5", label: "Cliente", title: "Abrir Clientes" },
  { key: "F6", label: "Dejar abierto", title: "Guardar venta como ticket abierto" },
  { key: "F7", label: "Quitar", title: "Quitar producto seleccionado" },
  { key: "F8", label: "Gasto", title: "Registrar gasto o salida de caja" },
  { key: "F9", label: "Pago", title: "Ir a captura de pago" },
  { key: "F10", label: "Cajon", title: "Abrir cajon y registrar apertura" },
  { key: "F11", label: "Mayoreo", compactLabel: true, title: "Aplicar precio de mayoreo" },
  { key: "F12", label: "Admin", title: "Abrir Configuracion" },
];

export function WeightPromptModal({
  product,
  onCancel,
  onConfirm,
}: {
  product: Product;
  onCancel: () => void;
  onConfirm: (quantity: number) => void;
}) {
  const [value, setValue] = useState("1");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    window.setTimeout(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    }, 40);
  }, []);

  const quantity = Number(value.replace(",", "."));
  const valid = Number.isFinite(quantity) && quantity > 0;
  const total = valid ? product.price * quantity : 0;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (valid) onConfirm(quantity);
  };

  return (
    <div className="modal-backdrop" role="presentation">
      <section className="ticket-name-modal weight-prompt-modal" role="dialog" aria-modal="true" aria-label={`Cantidad de ${product.name}`}>
        <div className="modal-title">
          <Scale size={22} />
          <div>
            <h2>{product.name}</h2>
            <p>Precio por {product.unit}: {money(product.price)}</p>
          </div>
        </div>
        <form className="dialog-form" onSubmit={submit}>
          <label>
            Cantidad ({product.unit})
            <input
              ref={inputRef}
              value={value}
              onChange={(event) => setValue(event.target.value)}
              inputMode="decimal"
            />
          </label>
          <div className="weight-prompt-total">
            <span>Total</span>
            <strong>{money(total)}</strong>
          </div>
          <div className="modal-actions">
            <button className="ghost-button" type="button" onClick={onCancel}>
              Cancelar
            </button>
            <button className="primary-button" type="submit" disabled={!valid}>
              Agregar
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}

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

export function QuickSaleModal({
  onCancel,
  onConfirm,
  showToast,
}: {
  onCancel: () => void;
  onConfirm: (input: { name: string; quantity: number; price: number; taxRate: number }) => void;
  showToast: (message: string) => void;
}) {
  const [taxes, setTaxes] = useState<TaxOption[]>([]);
  const [selectedTaxIds, setSelectedTaxIds] = useState<number[]>([]);
  const [name, setName] = useState("");
  const [quantity, setQuantity] = useState("1");
  const [price, setPrice] = useState("");
  const nameRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    window.setTimeout(() => nameRef.current?.focus(), 40);
    listTaxes()
      .then((options) => setTaxes(options.filter((tax) => tax.is_active && tax.rate > 0)))
      .catch((error) => showToast(String(error)));
  }, [showToast]);

  const quantityValue = Number(quantity.replace(",", "."));
  const priceValue = Number(price.replace(",", "."));
  const taxRate = taxes
    .filter((tax) => selectedTaxIds.includes(tax.id))
    .reduce((sum, tax) => sum + tax.rate, 0);
  const valid =
    name.trim().length > 0 &&
    Number.isFinite(quantityValue) && quantityValue > 0 &&
    Number.isFinite(priceValue) && priceValue > 0;
  const total = valid ? priceValue * quantityValue : 0;

  const toggleTax = (id: number) => {
    setSelectedTaxIds((current) =>
      current.includes(id) ? current.filter((item) => item !== id) : [...current, id],
    );
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (valid) onConfirm({ name: name.trim(), quantity: quantityValue, price: priceValue, taxRate });
  };

  return (
    <div className="modal-backdrop" role="presentation">
      <section className="ticket-name-modal" role="dialog" aria-modal="true" aria-label="Venta rapida sin catalogo">
        <div className="modal-title">
          <PackagePlus size={22} />
          <div>
            <h2>Producto fuera de catalogo</h2>
            <p>Vende un producto sin agregarlo. Precio con impuestos incluidos.</p>
          </div>
        </div>
        <form className="dialog-form" onSubmit={submit}>
          <label>
            Nombre
            <input ref={nameRef} value={name} onChange={(event) => setName(event.target.value)} placeholder="Ej. Producto suelto" />
          </label>
          <div className="quick-sale-grid">
            <label>
              Cantidad
              <input value={quantity} inputMode="decimal" onFocus={selectNumericInput} onChange={(event) => setQuantity(event.target.value)} />
            </label>
            <label>
              Precio (con impuestos)
              <input value={price} inputMode="decimal" onFocus={selectNumericInput} onChange={(event) => setPrice(event.target.value)} placeholder="0.00" />
            </label>
          </div>
          <div className="quick-sale-taxes">
            <span>Impuestos aplicados</span>
            {taxes.length === 0 ? (
              <small className="muted-copy">Sin impuestos configurados</small>
            ) : (
              <div className="quick-sale-tax-list">
                {taxes.map((tax) => (
                  <label key={tax.id} className="quick-sale-tax">
                    <input type="checkbox" checked={selectedTaxIds.includes(tax.id)} onChange={() => toggleTax(tax.id)} />
                    <span>{tax.name}</span>
                  </label>
                ))}
              </div>
            )}
          </div>
          <div className="weight-prompt-total">
            <span>Total</span>
            <strong>{money(total)}</strong>
          </div>
          <div className="modal-actions">
            <button className="ghost-button" type="button" onClick={onCancel}>Cancelar</button>
            <button className="primary-button" type="submit" disabled={!valid}>Agregar al carrito</button>
          </div>
        </form>
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
              placeholder="Vacio + Enter = Ticket automatico"
            />
          </label>
          <div className="modal-actions">
            <button className="ghost-button" type="button" onClick={onClose}>
              Cancelar
            </button>
            <button className="primary-button" type="submit">
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
            <strong>{formatDateTimeMx(draft.updated_at)}</strong>
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

export function UnexpectedShutdownModal({
  hasDraft,
  onRecover,
  onDiscard,
  onContinue,
}: {
  hasDraft: boolean;
  onRecover: () => void;
  onDiscard: () => void;
  onContinue: () => void;
}) {
  return (
    <div className="modal-backdrop" role="presentation">
      <section className="ticket-name-modal" role="dialog" aria-modal="true" aria-label="Cierre inesperado detectado">
        <div className="modal-title danger-title">
          <AlertTriangle size={22} />
          <div>
            <h2>Cierre inesperado detectado</h2>
            <p>App no cerro normal. Revisa venta pendiente, caja y ultimo backup antes de seguir.</p>
          </div>
        </div>
        <div className="recovery-summary">
          <div>
            <span>Estado</span>
            <strong>{hasDraft ? "Venta pendiente encontrada" : "Sin venta pendiente guardada"}</strong>
          </div>
          <div>
            <span>Accion</span>
            <strong>{hasDraft ? "Recuperar o descartar antes de seguir" : "Validar operacion y continuar"}</strong>
          </div>
        </div>
        <div className="modal-actions">
          {hasDraft ? (
            <>
              <button className="ghost-button" type="button" onClick={onDiscard}>
                Descartar venta
              </button>
              <button className="primary-button" type="button" onClick={onRecover}>
                Recuperar venta
              </button>
            </>
          ) : (
            <button className="primary-button" type="button" onClick={onContinue}>
              Entendido
            </button>
          )}
        </div>
      </section>
    </div>
  );
}
