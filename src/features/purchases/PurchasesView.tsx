import { Trash2 } from "lucide-react";
import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { money, roundMoney } from "../../lib/money";
import { createPurchase, listPurchases, listSuppliers, upsertSupplier } from "../../lib/posApi";
import type { Product, PurchaseReceipt, Supplier, UserSession } from "../../types";

type PurchaseDraftLine = {
  localId: number;
  productId: number;
  quantity: number;
  unitCost: number;
};

export function PurchasesView({
  session,
  products,
  refreshProducts,
  showToast,
}: {
  session: UserSession;
  products: Product[];
  refreshProducts: (query?: string) => Promise<void>;
  showToast: (message: string) => void;
}) {
  const [suppliers, setSuppliers] = useState<Supplier[]>([]);
  const [purchases, setPurchases] = useState<PurchaseReceipt[]>([]);
  const [supplierForm, setSupplierForm] = useState({ name: "", phone: "", contact: "" });
  const [supplierId, setSupplierId] = useState<number | "">("");
  const [productId, setProductId] = useState(products[0]?.id ?? 0);
  const [quantity, setQuantity] = useState(1);
  const [unitCost, setUnitCost] = useState(products[0]?.cost ?? 0);
  const [note, setNote] = useState("Compra proveedor");
  const [purchaseLines, setPurchaseLines] = useState<PurchaseDraftLine[]>([]);
  const [savingPurchase, setSavingPurchase] = useState(false);

  const refresh = useCallback(async () => {
    const [nextSuppliers, nextPurchases] = await Promise.all([listSuppliers(), listPurchases()]);
    setSuppliers(nextSuppliers);
    setPurchases(nextPurchases);
  }, []);

  useEffect(() => {
    refresh().catch((error) => showToast(String(error)));
  }, [refresh, showToast]);

  useEffect(() => {
    if (!productId && products[0]) {
      setProductId(products[0].id);
      setUnitCost(products[0].cost);
    }
  }, [productId, products]);

  const selectedProduct = useMemo(() => products.find((product) => product.id === productId), [productId, products]);
  const purchaseTotal = useMemo(
    () => purchaseLines.reduce((sum, line) => sum + line.quantity * line.unitCost, 0),
    [purchaseLines],
  );

  const saveSupplier = async (event: FormEvent) => {
    event.preventDefault();
    try {
      const supplier = await upsertSupplier(supplierForm);
      setSupplierForm({ name: "", phone: "", contact: "" });
      setSupplierId(supplier.id);
      await refresh();
      showToast("Proveedor guardado");
    } catch (error) {
      showToast(String(error));
    }
  };

  const addPurchaseLine = () => {
    if (!selectedProduct || quantity <= 0 || unitCost < 0) {
      showToast("Producto, cantidad y costo requeridos");
      return;
    }
    const existing = purchaseLines.find((line) => line.productId === productId);
    if (existing) {
      setPurchaseLines((current) => current.map((line) =>
        line.productId === productId
          ? { ...line, quantity: roundMoney(line.quantity + quantity), unitCost }
          : line,
      ));
    } else {
      setPurchaseLines((current) => [...current, { localId: Date.now(), productId, quantity, unitCost }]);
    }
    setQuantity(1);
    showToast("Producto agregado a compra");
  };

  const savePurchase = async (event: FormEvent) => {
    event.preventDefault();
    if (purchaseLines.length === 0) {
      showToast("Agrega productos a la compra");
      return;
    }
    setSavingPurchase(true);
    try {
      const batch = `Compra lote ${new Date().toLocaleString("es-MX")}`;
      const receipts = [];
      for (const line of purchaseLines) {
        receipts.push(await createPurchase({
          supplier_id: supplierId || null,
          product_id: line.productId,
          quantity: line.quantity,
          unit_cost: line.unitCost,
          user_id: session.id,
          note: `${note.trim() || "Compra proveedor"} · ${batch}`,
        }));
      }
      setPurchaseLines([]);
      await refreshProducts("");
      await refresh();
      showToast(`${receipts.length} partidas registradas`);
    } catch (error) {
      showToast(String(error));
    } finally {
      setSavingPurchase(false);
    }
  };

  return (
    <section className="admin-panel purchase-module">
      <div className="module-toolbar">
        <div>
          <h2>Compras y proveedores</h2>
          <p>Entrada de inventario, costo proveedor e historial.</p>
        </div>
        <div className="toolbar-actions">
          <button className="ghost-button" type="button" onClick={() => refresh().catch((error) => showToast(String(error)))}>
            Actualizar
          </button>
        </div>
      </div>
      <div className="purchase-layout">
        <form className="user-form side-form" onSubmit={saveSupplier}>
          <div>
            <h2>Proveedor</h2>
            <p>Alta rapida para compras.</p>
          </div>
          <label>Nombre<input value={supplierForm.name} onChange={(event) => setSupplierForm({ ...supplierForm, name: event.target.value })} /></label>
          <label>Telefono<input value={supplierForm.phone} onChange={(event) => setSupplierForm({ ...supplierForm, phone: event.target.value })} /></label>
          <label>Contacto<input value={supplierForm.contact} onChange={(event) => setSupplierForm({ ...supplierForm, contact: event.target.value })} /></label>
          <button className="primary-button" type="submit">Guardar proveedor</button>
        </form>

        <form className="user-form transaction-form purchase-entry-form" onSubmit={savePurchase}>
          <div>
            <h2>Nueva compra</h2>
            <p>Agrega varias partidas, revisa total y registra entrada.</p>
          </div>
          <label>
            Proveedor
            <select value={supplierId} onChange={(event) => setSupplierId(event.target.value ? Number(event.target.value) : "")}>
              <option value="">Sin proveedor</option>
              {suppliers.map((supplier) => (
                <option value={supplier.id} key={supplier.id}>{supplier.name}</option>
              ))}
            </select>
          </label>
          <label className="field-span-2">
            Producto
            <select value={productId} onChange={(event) => {
              const nextId = Number(event.target.value);
              setProductId(nextId);
              setUnitCost(products.find((product) => product.id === nextId)?.cost ?? 0);
            }}>
              {products.map((product) => (
                <option value={product.id} key={product.id}>{product.name}</option>
              ))}
            </select>
          </label>
          <label>Cantidad<input type="number" min="0.001" step="0.001" value={quantity} onChange={(event) => setQuantity(Number(event.target.value))} /></label>
          <label>Costo unitario<input type="number" min="0" step="0.01" value={unitCost} onChange={(event) => setUnitCost(Number(event.target.value))} /></label>
          <button className="ghost-button form-submit" type="button" disabled={!selectedProduct} onClick={addPurchaseLine}>
            Agregar partida
          </button>
          <label className="field-span-2">Nota<input value={note} onChange={(event) => setNote(event.target.value)} /></label>
          <div className="purchase-draft-list field-span-2">
            {purchaseLines.length === 0 ? (
              <div className="table-empty compact">Sin partidas agregadas</div>
            ) : purchaseLines.map((line) => {
              const product = products.find((candidate) => candidate.id === line.productId);
              return (
                <div className="purchase-draft-row" key={line.localId}>
                  <strong>{product?.name ?? "Producto"}</strong>
                  <input
                    type="number"
                    min="0.001"
                    step="0.001"
                    value={line.quantity}
                    aria-label={`Cantidad ${product?.name ?? ""}`}
                    onChange={(event) => setPurchaseLines((current) => current.map((candidate) => candidate.localId === line.localId ? { ...candidate, quantity: Number(event.target.value) } : candidate))}
                  />
                  <input
                    type="number"
                    min="0"
                    step="0.01"
                    value={line.unitCost}
                    aria-label={`Costo ${product?.name ?? ""}`}
                    onChange={(event) => setPurchaseLines((current) => current.map((candidate) => candidate.localId === line.localId ? { ...candidate, unitCost: Number(event.target.value) } : candidate))}
                  />
                  <span>{money(line.quantity * line.unitCost)}</span>
                  <button className="icon-button danger" type="button" aria-label={`Quitar ${product?.name ?? "partida"}`} onClick={() => setPurchaseLines((current) => current.filter((candidate) => candidate.localId !== line.localId))}>
                    <Trash2 size={16} />
                  </button>
                </div>
              );
            })}
          </div>
          <div className="purchase-total field-span-2">
            <span>Total compra</span>
            <strong>{money(purchaseTotal)}</strong>
          </div>
          <button className="primary-button form-submit" type="submit" disabled={savingPurchase || purchaseLines.length === 0}>
            Registrar compra
          </button>
        </form>
      </div>

      <div className="purchase-columns">
        <div className="data-table">
          <div className="table-head purchase-row">
            <span>Compra</span>
            <span>Proveedor</span>
            <span>Producto</span>
            <span>Cantidad</span>
            <span>Total</span>
          </div>
          {purchases.length === 0 ? (
            <div className="table-empty">Sin compras registradas</div>
          ) : purchases.map((purchase) => (
            <div className="purchase-row" key={purchase.id}>
              <strong>#{purchase.id}</strong>
              <span>{purchase.supplier_name || "Sin proveedor"}</span>
              <span>{purchase.product_name}</span>
              <span>{purchase.quantity}</span>
              <strong className="money-cell">{money(purchase.total)}</strong>
            </div>
          ))}
        </div>
        <aside className="inventory-side">
          <h3>Proveedores</h3>
          {suppliers.length === 0 ? <span className="muted-note">Sin proveedores</span> : suppliers.map((supplier) => (
            <div className="kardex-row" key={supplier.id}>
              <strong>{supplier.name}</strong>
              <span>{supplier.phone || "Sin telefono"}</span>
              <small>{supplier.contact || "Sin contacto"}</small>
            </div>
          ))}
        </aside>
      </div>
    </section>
  );
}
