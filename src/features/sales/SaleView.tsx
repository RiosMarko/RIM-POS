import { CircleDollarSign, Search, ShoppingCart, Ticket, Trash2 } from "lucide-react";
import { FormEvent, KeyboardEvent as ReactKeyboardEvent, RefObject, useEffect, useMemo, useRef, useState } from "react";
import { cartTotals, money, roundMoney } from "../../lib/money";
import { selectNumericInput } from "../../lib/numberInput";
import type { CartLine, HeldTicket, Product, SaleReceipt } from "../../types";
import { functionKeys } from "./SaleModals";

const PRODUCT_SEARCH_DEBOUNCE_MS = 140;

export function SaleView({
  query,
  products,
  cart,
  totals,
  paid,
  shortage,
  change,
  cashReceived,
  cardReceived,
  transferReceived,
  cardTerminals,
  selectedCardTerminal,
  lastReceipt,
  heldTickets,
  activeHeldTicketId,
  selectedCartProductId,
  busy,
  hasOpenCash,
  searchRef,
  cashRef,
  setQuery,
  setCashReceived,
  setCardReceived,
  setTransferReceived,
  setSelectedCardTerminal,
  refreshProducts,
  submitSearch,
  addProduct,
  updateLine,
  selectCartLine,
  completeSale,
  onReprintLast,
  holdCurrentTicket,
  newTicket,
  recoverHeldTicket,
  removeHeldTicket,
  runFunctionKeyAction,
  showToast,
  openHeldTickets,
}: {
  query: string;
  products: Product[];
  cart: CartLine[];
  totals: ReturnType<typeof cartTotals>;
  paid: number;
  shortage: number;
  change: number;
  cashReceived: string;
  cardReceived: string;
  transferReceived: string;
  cardTerminals: string[];
  selectedCardTerminal: string;
  lastReceipt: SaleReceipt | null;
  heldTickets: HeldTicket[];
  activeHeldTicketId: number | null;
  selectedCartProductId: number | null;
  busy: boolean;
  hasOpenCash: boolean;
  searchRef: RefObject<HTMLInputElement>;
  cashRef: RefObject<HTMLInputElement>;
  setQuery: (value: string) => void;
  setCashReceived: (value: string) => void;
  setCardReceived: (value: string) => void;
  setTransferReceived: (value: string) => void;
  setSelectedCardTerminal: (value: string) => void;
  refreshProducts: (query?: string) => Promise<void>;
  submitSearch: (event?: FormEvent) => Promise<void>;
  addProduct: (product: Product, quantity?: number) => void;
  updateLine: (productId: number, patch: Partial<Pick<CartLine, "quantity" | "discount">>) => void;
  selectCartLine: (productId: number) => void;
  completeSale: (options?: { printTicket?: boolean }) => Promise<void>;
  onReprintLast: () => void | Promise<void>;
  holdCurrentTicket: () => void | Promise<void>;
  newTicket: () => void | Promise<void>;
  recoverHeldTicket: (ticket: HeldTicket) => Promise<void>;
  removeHeldTicket: (ticket: HeldTicket) => Promise<void>;
  runFunctionKeyAction: (key: string) => void | Promise<void>;
  showToast: (message: string) => void;
  openHeldTickets: () => void;
}) {
  const selectedLineRef = useRef<HTMLDivElement>(null);
  const [selectedSuggestionIndex, setSelectedSuggestionIndex] = useState(0);
  const [quantityDrafts, setQuantityDrafts] = useState<Record<number, string>>({});
  const normalizedQuery = useMemo(() => query.trim().toLowerCase(), [query]);
  const visibleSuggestions = useMemo(() => {
    if (!normalizedQuery) return [];
    return products.filter((product) =>
        product.name.toLowerCase().includes(normalizedQuery) ||
        product.category.toLowerCase().includes(normalizedQuery) ||
        product.barcode.includes(normalizedQuery),
      ).slice(0, 6);
  }, [normalizedQuery, products]);

  useEffect(() => {
    setSelectedSuggestionIndex(0);
  }, [normalizedQuery, visibleSuggestions.length]);

  useEffect(() => {
    if (!query.trim()) return;
    const handle = window.setTimeout(() => {
      refreshProducts(query).catch((error) => showToast(String(error)));
    }, PRODUCT_SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(handle);
  }, [query, refreshProducts, showToast]);

  useEffect(() => {
    selectedLineRef.current?.scrollIntoView({ block: "nearest" });
  }, [selectedCartProductId]);

  const handleSearchKeyDown = (event: ReactKeyboardEvent<HTMLInputElement>) => {
    if (visibleSuggestions.length === 0) return;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      event.stopPropagation();
      const direction = event.key === "ArrowDown" ? 1 : -1;
      setSelectedSuggestionIndex((current) => {
        const next = current + direction;
        if (next < 0) return visibleSuggestions.length - 1;
        if (next >= visibleSuggestions.length) return 0;
        return next;
      });
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      event.stopPropagation();
      addProduct(visibleSuggestions[selectedSuggestionIndex] ?? visibleSuggestions[0]);
    }
  };

  return (
    <section className="sale-grid">
      <div className="sale-main">
        <div className="search-box">
          <form className="search-row" onSubmit={submitSearch}>
            <Search size={20} />
            <input
              ref={searchRef}
              value={query}
              onChange={(event) => {
                setQuery(event.target.value);
              }}
              onKeyDown={handleSearchKeyDown}
              placeholder="Escanea codigo o busca producto"
              autoFocus
            />
            <button type="submit" className="primary-button">
              Buscar
            </button>
          </form>
          {visibleSuggestions.length > 0 && (
            <div className="search-suggestions" role="listbox" aria-label="Sugerencias de productos">
              {visibleSuggestions.map((product, index) => (
                <button
                  className={index === selectedSuggestionIndex ? "active" : undefined}
                  type="button"
                  role="option"
                  aria-selected={index === selectedSuggestionIndex}
                  key={product.id}
                  onMouseEnter={() => setSelectedSuggestionIndex(index)}
                  onClick={() => addProduct(product)}
                >
                  <span>{product.name}</span>
                  <small>{product.barcode || product.sku}</small>
                  <strong>{money(product.price)}</strong>
                </button>
              ))}
            </div>
          )}
        </div>

        <div className="sales-tools" aria-label="Funciones de venta">
          <button type="button" disabled={cart.length === 0} onClick={holdCurrentTicket}>Dejar abierto</button>
          <button type="button" onClick={newTicket}>Nuevo Ticket</button>
        </div>

        <div className="cart-panel">
          <div className="cart-header">
            <span>Articulo</span>
            <span>Cant.</span>
            <span>Precio</span>
            <span>Desc.</span>
            <span>Total</span>
            <span />
          </div>
          {heldTickets.length > 0 && (
            <div className="held-ticket-tabs" aria-label="Tickets abiertos">
              {heldTickets.map((ticket) => (
                <div className={activeHeldTicketId === ticket.id ? "held-ticket-tab active" : "held-ticket-tab"} key={ticket.id}>
                  <button type="button" onClick={() => recoverHeldTicket(ticket)}>
                    <Ticket size={15} />
                    <span>{ticket.name}</span>
                    <strong>{money(ticket.total)}</strong>
                  </button>
                  <button
                    className="held-ticket-close"
                    type="button"
                    aria-label={`Eliminar ${ticket.name}`}
                    onClick={() => removeHeldTicket(ticket)}
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              ))}
            </div>
          )}
          <div className="cart-scroll">
            {cart.length === 0 ? (
              <div className="empty-state">
                <ShoppingCart size={34} />
                <strong>Venta lista</strong>
                <span>Escanea producto. Si no aparece, busca por nombre.</span>
              </div>
            ) : (
              cart.map((line) => (
                <div
                  ref={selectedCartProductId === line.product.id ? selectedLineRef : undefined}
                  className={selectedCartProductId === line.product.id ? "cart-line selected" : "cart-line"}
                  key={line.product.id}
                  aria-selected={selectedCartProductId === line.product.id}
                  onClick={() => selectCartLine(line.product.id)}
                  onFocusCapture={() => selectCartLine(line.product.id)}
                >
                  <div>
                    <strong>{line.product.name}</strong>
                    <span>{line.product.barcode}</span>
                  </div>
                  <input
                    aria-label={`Cantidad ${line.product.name}`}
                    className="number-input"
                    type="number"
                    min="0"
                    step={line.product.unit === "pieza" ? "1" : "0.001"}
                    value={quantityDrafts[line.product.id] ?? String(line.quantity)}
                    onFocus={(event) => {
                      setQuantityDrafts((current) => ({ ...current, [line.product.id]: String(line.quantity) }));
                      selectNumericInput(event);
                    }}
                    onChange={(event) => {
                      const text = event.target.value;
                      setQuantityDrafts((current) => ({ ...current, [line.product.id]: text }));
                      const parsed = Number(text);
                      // Skip zero/invalid mid-typing values (e.g. leading "0" before "0.456")
                      // so the line isn't deleted while the user is still typing a decimal.
                      if (Number.isFinite(parsed) && parsed > 0) {
                        updateLine(line.product.id, { quantity: parsed });
                      }
                    }}
                    onBlur={() => {
                      const parsed = Number(quantityDrafts[line.product.id]);
                      updateLine(line.product.id, { quantity: Number.isFinite(parsed) && parsed > 0 ? parsed : 0 });
                      setQuantityDrafts((current) => {
                        const next = { ...current };
                        delete next[line.product.id];
                        return next;
                      });
                    }}
                  />
                  <span className="money-cell">{money(line.product.price)}</span>
                  <input
                    aria-label={`Descuento ${line.product.name}`}
                    className="number-input"
                    type="number"
                    min="0"
                    step="0.5"
                    value={line.discount === 0 ? "" : line.discount}
                    onFocus={selectNumericInput}
                    onChange={(event) => updateLine(line.product.id, { discount: Number(event.target.value) })}
                  />
                  <strong className="money-cell">{money(line.product.price * line.quantity - line.discount)}</strong>
                  <button className="icon-button danger" type="button" onClick={() => updateLine(line.product.id, { quantity: 0 })}>
                    <Trash2 size={18} />
                  </button>
                </div>
              ))
            )}
          </div>
        </div>
      </div>

      <aside className="payment-panel">
        <div className="total-block">
          <span>Total a cobrar</span>
          <strong>{money(totals.total)}</strong>
        </div>
        <div className="summary-lines">
          <div>
            <span>Subtotal</span>
            <strong>{money(totals.subtotal)}</strong>
          </div>
          <div>
            <span>Impuestos</span>
            <strong>{money(totals.tax)}</strong>
          </div>
          <div>
            <span>Pagado</span>
            <strong>{money(paid)}</strong>
          </div>
        </div>

        <label className="field-label">
          Efectivo recibido
          <input
            ref={cashRef}
            className="cash-input"
            type="number"
            min="0"
            step="0.5"
            value={cashReceived}
            onFocus={selectNumericInput}
            onChange={(event) => setCashReceived(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") completeSale();
            }}
          />
        </label>

        <div className="payment-method-grid">
          <label className="field-label">
            Tarjeta
            <input
              className="mini-money-input"
              type="number"
              min="0"
              step="0.5"
              value={cardReceived}
              onFocus={selectNumericInput}
              onChange={(event) => setCardReceived(event.target.value)}
            />
            {(Number(cardReceived) || 0) > 0 && (
              <div className="card-terminal-panel">
                {cardTerminals.length > 0 ? (
                  <select value={selectedCardTerminal} onChange={(event) => setSelectedCardTerminal(event.target.value)}>
                    <option value="">Seleccionar terminal</option>
                    {cardTerminals.map((terminal) => (
                      <option value={terminal} key={terminal}>{terminal}</option>
                    ))}
                  </select>
                ) : (
                  <small className="card-terminal-empty">Agrega terminales en Config.</small>
                )}
              </div>
            )}
          </label>
          <label className="field-label">
            Transferencia
            <input
              className="mini-money-input"
              type="number"
              min="0"
              step="0.5"
              value={transferReceived}
              onFocus={selectNumericInput}
              onChange={(event) => setTransferReceived(event.target.value)}
            />
          </label>
        </div>

        <div className={shortage > 0 ? "change-box warning" : "change-box"}>
          <span>{shortage > 0 ? "Falta" : "Cambio"}</span>
          <strong>{money(shortage > 0 ? shortage : change)}</strong>
        </div>

        <button className="pay-button" type="button" disabled={busy || cart.length === 0 || !hasOpenCash} onClick={() => completeSale()}>
          <CircleDollarSign size={24} />
          {hasOpenCash ? "Cobrar venta" : "Abre caja primero"}
          <kbd>F1/F2</kbd>
        </button>

        <div className="held-ticket-actions">
          <button className="ghost-button" type="button" disabled={cart.length === 0} onClick={holdCurrentTicket}>
            <Ticket size={18} />
            Dejar abierto
          </button>
          <button className="ghost-button" type="button" onClick={openHeldTickets}>
            Recuperar ({heldTickets.length})
          </button>
        </div>

        <div className="function-key-grid compact">
          {functionKeys.map((item) => (
            <button
              className={item.compactLabel ? "compact-label" : undefined}
              key={item.key}
              title={item.title}
              type="button"
              onClick={() => {
                runFunctionKeyAction(item.key);
              }}
            >
              <kbd>{item.key}</kbd>
              <span className="function-key-label">{item.label}</span>
            </button>
          ))}
        </div>

        {lastReceipt && (
          <div className="last-receipt">
            <span>Ultima venta</span>
            <strong>{lastReceipt.folio}</strong>
            <small>{money(lastReceipt.total)}</small>
            <button className="ghost-button mini" type="button" onClick={() => onReprintLast()}>
              <Ticket size={14} /> Reimprimir ticket
            </button>
          </div>
        )}
      </aside>
    </section>
  );
}
