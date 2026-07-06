import { Trash2 } from "lucide-react";
import { FormEvent, useCallback, useEffect, useState } from "react";
import type { ConfirmDraft } from "../../components/modals/CommonModals";
import { loadShoppingList, saveShoppingList, type ShoppingListItem } from "../../lib/shoppingList";
import { deleteSupplier, listSuppliers, upsertSupplier } from "../../lib/posApi";
import type { Supplier } from "../../types";

export function PurchasesView({
  showToast,
  requestConfirm,
}: {
  showToast: (message: string) => void;
  requestConfirm: (draft: ConfirmDraft) => void;
}) {
  const [suppliers, setSuppliers] = useState<Supplier[]>([]);
  const [supplierForm, setSupplierForm] = useState({ name: "", phone: "", contact: "", id: undefined as number | undefined });
  const [shoppingList, setShoppingList] = useState<ShoppingListItem[]>([]);
  const [shoppingDraft, setShoppingDraft] = useState("");
  const editingSupplier = Boolean(supplierForm.id);
  const resetSupplierForm = () => setSupplierForm({ name: "", phone: "", contact: "", id: undefined });

  const refresh = useCallback(async () => {
    setSuppliers(await listSuppliers());
  }, []);

  useEffect(() => {
    refresh().catch((error) => showToast(String(error)));
  }, [refresh, showToast]);

  useEffect(() => {
    setShoppingList(loadShoppingList());
  }, []);

  const saveSupplier = async (event: FormEvent) => {
    event.preventDefault();
    try {
      await upsertSupplier(supplierForm);
      resetSupplierForm();
      await refresh();
      showToast("Proveedor guardado");
    } catch (error) {
      showToast(String(error));
    }
  };

  const editSupplier = (supplier: Supplier) => {
    setSupplierForm({ id: supplier.id, name: supplier.name, phone: supplier.phone ?? "", contact: supplier.contact ?? "" });
  };

  const removeSupplier = (supplier: Supplier) => {
    requestConfirm({
      title: "Borrar proveedor",
      message: `${supplier.name} deja de aparecer en compras.`,
      confirmLabel: "Borrar proveedor",
      tone: "danger",
      onConfirm: async () => {
        try {
          await deleteSupplier(supplier.id);
          if (supplierForm.id === supplier.id) resetSupplierForm();
          await refresh();
          showToast("Proveedor borrado");
        } catch (error) {
          showToast(String(error));
        }
      },
    });
  };

  const addShoppingItem = () => {
    const text = shoppingDraft.trim();
    if (!text) return;
    setShoppingList(saveShoppingList([...shoppingList, { id: Date.now(), text }]));
    setShoppingDraft("");
  };

  const removeShoppingItem = (id: number) => {
    setShoppingList(saveShoppingList(shoppingList.filter((item) => item.id !== id)));
  };

  return (
    <section className="admin-panel purchase-module">
      <div className="module-toolbar">
        <div>
          <h2>Compras y proveedores</h2>
          <p>Datos de proveedores y lista de lo que falta comprar.</p>
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
          <div className="form-button-row">
            {editingSupplier && <button className="ghost-button" type="button" onClick={resetSupplierForm}>Nuevo proveedor</button>}
            <button className="primary-button" type="submit">{editingSupplier ? "Actualizar proveedor" : "Guardar proveedor"}</button>
          </div>
        </form>

        <div className="user-form transaction-form">
          <div>
            <h2>Lista de compras</h2>
            <p>Anota que falta comprar y borralo cuando ya lo traigas.</p>
          </div>
          <div className="terminal-config">
            <label>
              Producto a comprar
              <input
                value={shoppingDraft}
                onChange={(event) => setShoppingDraft(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    addShoppingItem();
                  }
                }}
                placeholder="Ej. Coca 600ml x10"
              />
            </label>
            <button className="primary-button" type="button" onClick={addShoppingItem}>Agregar</button>
          </div>
          <div className="terminal-list">
            {shoppingList.length === 0 ? (
              <div className="muted-note">Sin pendientes por comprar.</div>
            ) : shoppingList.map((item) => (
              <div className="terminal-row" key={item.id}>
                <div>
                  <strong>{item.text}</strong>
                </div>
                <button className="icon-button danger" type="button" aria-label={`Quitar ${item.text}`} onClick={() => removeShoppingItem(item.id)}>
                  <Trash2 size={16} />
                </button>
              </div>
            ))}
          </div>
        </div>
      </div>

      <aside className="inventory-side">
        <h3>Proveedores</h3>
        {suppliers.length === 0 ? (
          <span className="muted-note">Sin proveedores</span>
        ) : (
          <div className="supplier-grid">
            {suppliers.map((supplier) => (
              <div className="kardex-row" key={supplier.id}>
                <strong>{supplier.contact ? `${supplier.name} - ${supplier.contact}` : supplier.name}</strong>
                <span>{supplier.phone ? `Tel. ${supplier.phone}` : "Sin telefono"}</span>
                <div className="form-button-row">
                  <button className="ghost-button" type="button" onClick={() => editSupplier(supplier)}>Editar</button>
                  <button className="icon-button danger" type="button" aria-label={`Borrar ${supplier.name}`} onClick={() => removeSupplier(supplier)}>
                    <Trash2 size={16} />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </aside>
    </section>
  );
}
