import { BarChart3, Boxes, CircleDollarSign, Search } from "lucide-react";
import { KeyboardEvent as ReactKeyboardEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Metric } from "../../components/display/SummaryCards";
import { downloadCsv } from "../../lib/csv";
import { money, roundMoney } from "../../lib/money";
import { adjustInventory, listInventoryMovements } from "../../lib/posApi";
import type { InventoryMovement, Product } from "../../types";
import { InventoryAdjustmentModal } from "./InventoryModals";

export function InventoryView({
  products,
  refreshProducts,
  showToast,
}: {
  products: Product[];
  refreshProducts: (query?: string) => Promise<void>;
  showToast: (message: string) => void;
}) {
  const [adjustmentProduct, setAdjustmentProduct] = useState<Product | null>(null);
  const [movements, setMovements] = useState<InventoryMovement[]>([]);
  const [inventoryQuery, setInventoryQuery] = useState("");
  const inventorySearchRef = useRef<HTMLInputElement>(null);
  const zeroStock = useMemo(() => products.filter((product) => product.stock <= 0), [products]);
  const inventoryValue = useMemo(() => products.reduce((sum, product) => sum + product.stock * product.cost, 0), [products]);
  const totalUnits = useMemo(() => products.reduce((sum, product) => sum + product.stock, 0), [products]);
  const normalizedInventoryQuery = useMemo(() => inventoryQuery.trim().toLowerCase(), [inventoryQuery]);
  const visibleProducts = useMemo(() => products.filter((product) => {
    const query = normalizedInventoryQuery;
    if (!query) return true;
    return (
      product.name.toLowerCase().includes(query) ||
      product.barcode.toLowerCase().includes(query) ||
      product.sku.toLowerCase().includes(query) ||
      product.category.toLowerCase().includes(query)
    );
  }), [normalizedInventoryQuery, products]);
  const latestMovementByProduct = useMemo(() => {
    const latest = new Map<number, InventoryMovement>();
    movements.forEach((movement) => {
      if (!latest.has(movement.product_id)) latest.set(movement.product_id, movement);
    });
    return latest;
  }, [movements]);

  const refreshKardex = useCallback(async () => {
    setMovements(await listInventoryMovements(undefined));
  }, []);

  useEffect(() => {
    refreshKardex().catch((error) => showToast(String(error)));
  }, [refreshKardex, showToast]);

  useEffect(() => {
    window.setTimeout(() => inventorySearchRef.current?.focus(), 40);
  }, []);

  const movementLabel = (movement?: InventoryMovement) => {
    if (!movement) return "Sin movimiento";
    const quantity = `${movement.quantity > 0 ? "+" : ""}${movement.quantity}`;
    return `${quantity} · ${movement.reason}`;
  };

  const saveAdjustment = async (quantity: number, reason: string) => {
    if (!adjustmentProduct) return;
    try {
      await adjustInventory({ product_id: adjustmentProduct.id, quantity, reason });
      await refreshProducts("");
      await refreshKardex();
      setAdjustmentProduct(null);
      showToast("Inventario ajustado");
    } catch (error) {
      showToast(String(error));
    }
  };

  const openAdjustment = (product: Product) => {
    if (product.stock <= 0) showToast(`${product.name} esta en 0`);
    setAdjustmentProduct(product);
  };

  const submitInventorySearch = (event: ReactKeyboardEvent<HTMLInputElement>) => {
    if (event.key !== "Enter") return;
    event.preventDefault();
    const query = inventoryQuery.trim().toLowerCase();
    if (!query) return;
    const exactMatch = products.find((product) =>
      product.barcode.toLowerCase() === query ||
      product.sku.toLowerCase() === query,
    );
    const targetProduct = exactMatch ?? (visibleProducts.length === 1 ? visibleProducts[0] : null);
    if (!targetProduct) {
      showToast("Producto no encontrado en inventario");
      return;
    }
    openAdjustment(targetProduct);
  };

  const exportInventory = () => {
    downloadCsv(`inventario-rim-pos-${new Date().toISOString().slice(0, 10)}.csv`, [
      ["producto", "sku", "codigo", "departamento", "existencia", "unidad", "costo", "valor_costo", "ultimo_movimiento"],
      ...visibleProducts.map((product) => [
        product.name,
        product.sku,
        product.barcode,
        product.category,
        product.stock,
        product.unit,
        product.cost,
        roundMoney(product.stock * product.cost),
        movementLabel(latestMovementByProduct.get(product.id)),
      ]),
    ]);
    showToast("Inventario exportado");
  };

  return (
    <section className="admin-panel inventory-module">
      <div className="module-toolbar">
        <div>
          <h2>Control de inventario</h2>
          <p>Existencias, ajustes, entradas, salidas y reporte.</p>
        </div>
        <div className="toolbar-actions">
          <button className="ghost-button" type="button" onClick={exportInventory}>Exportar reporte</button>
        </div>
      </div>
      <div className="inventory-summary">
        <Metric icon={Boxes} label="Unidades" value={String(totalUnits)} />
        <Metric icon={CircleDollarSign} label="Valor costo" value={money(inventoryValue)} />
        <Metric icon={BarChart3} label="En 0" value={String(zeroStock.length)} />
      </div>
      <form className="catalog-search" onSubmit={(event) => event.preventDefault()}>
        <Search size={18} />
        <input
          value={inventoryQuery}
          onChange={(event) => setInventoryQuery(event.target.value)}
          onKeyDown={submitInventorySearch}
          ref={inventorySearchRef}
          placeholder="Buscar en inventario por producto, codigo, SKU o departamento"
        />
      </form>
      <div className="inventory-layout">
        <div className="data-table">
          <div className="table-head inventory-row">
            <span>Producto</span>
            <span>Existencia</span>
            <span>Ultimo movimiento</span>
            <span>Ajuste</span>
          </div>
          {products.length === 0 ? (
            <div className="table-empty">Sin productos para mostrar</div>
          ) : visibleProducts.map((product) => (
            <div className="inventory-row" key={product.id}>
              <strong>{product.name}</strong>
              <span className={product.stock <= 0 ? "stock-low" : ""}>{product.stock} {product.unit}</span>
              <span>{movementLabel(latestMovementByProduct.get(product.id))}</span>
              <button
                className="ghost-button row-action"
                type="button"
                onClick={() => openAdjustment(product)}
              >
                Ajustar
              </button>
            </div>
          ))}
        </div>
        <aside className="inventory-side">
          <h3>Kardex</h3>
          <button type="button" onClick={() => refreshKardex().catch((error) => showToast(String(error)))}>Actualizar kardex</button>
          <div className="kardex-list">
            {movements.slice(0, 8).map((movement) => (
              <div className="kardex-row" key={movement.id}>
                <strong>{movement.quantity > 0 ? "+" : ""}{movement.quantity}</strong>
                <span>{movement.reason}</span>
                <small>{movement.product_name}</small>
              </div>
            ))}
          </div>
        </aside>
      </div>
      {adjustmentProduct && (
        <InventoryAdjustmentModal
          product={adjustmentProduct}
          onClose={() => setAdjustmentProduct(null)}
          onSave={saveAdjustment}
        />
      )}
    </section>
  );
}
