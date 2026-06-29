import { PackagePlus, Search, Trash2 } from "lucide-react";
import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ConfirmDraft } from "../../components/modals/CommonModals";
import { downloadCsv, parseCsvLine } from "../../lib/csv";
import { money } from "../../lib/money";
import { deleteProduct, getSetting, listTaxes, upsertProduct } from "../../lib/posApi";
import type { Product, TaxOption } from "../../types";

const emptyProductForm = {
  sku: "",
  barcode: "",
  name: "",
  category: "Abarrotes",
  unit: "pieza",
  price: 0,
  cost: 0,
  stock: 0,
  min_stock: 0,
  tax_rate: 0,
  tax_ids: [] as number[],
  active: true,
};

const fallbackTaxOptions: TaxOption[] = [
  { id: 3, name: "Exento 0%", rate: 0, type: "IVA", country: "MX", is_active: true },
  { id: 2, name: "IVA 8%", rate: 0.08, type: "IVA", country: "MX", is_active: true },
  { id: 1, name: "IVA 16%", rate: 0.16, type: "IVA", country: "MX", is_active: true },
  { id: 4, name: "IEPS 8%", rate: 0.08, type: "IEPS", country: "MX", is_active: true },
  { id: 5, name: "IEPS 26.5%", rate: 0.265, type: "IEPS", country: "MX", is_active: true },
];

const normalizeBarcode = (value: string) => value.replace(/[^0-9A-Za-z]/g, "").trim();

const suggestCategory = (name: string, fallback = "Abarrotes") => {
  const value = name.toLowerCase().normalize("NFD").replace(/\p{Diacritic}/gu, "");
  if (/(refresco|agua|jugo|suero|bebida)/.test(value)) return "Bebidas";
  if (/(papa|cacahuate|botana|chicharron)/.test(value)) return "Botanas";
  if (/(leche|crema|queso|yogur)/.test(value)) return "Lacteos";
  if (/(jabon|detergente|cloro|suavizante|fibra)/.test(value)) return "Limpieza";
  if (/(papel|servilleta|shampoo|pasta dental)/.test(value)) return "Higiene";
  if (/(galleta|pan|roles)/.test(value)) return "Panaderia";
  if (/(dulce|chicle|chocolate|paleta)/.test(value)) return "Dulces";
  return fallback || "Abarrotes";
};

export function ProductsView({
  products,
  refreshProducts,
  showToast,
  requestConfirm,
}: {
  products: Product[];
  refreshProducts: (query?: string) => Promise<void>;
  showToast: (message: string) => void;
  requestConfirm: (draft: ConfirmDraft) => void;
}) {
  const [form, setForm] = useState<typeof emptyProductForm & { id?: number }>(emptyProductForm);
  const [taxEnabled, setTaxEnabled] = useState(true);
  const [taxDefaultRate, setTaxDefaultRate] = useState(0.16);
  const [, setTaxPricesIncludeTax] = useState(true);
  const [taxAutoApply, setTaxAutoApply] = useState(true);
  const [taxOptions, setTaxOptions] = useState<TaxOption[]>([]);
  const [busy, setBusy] = useState(false);
  const [catalogQuery, setCatalogQuery] = useState("");
  const [editorOpen, setEditorOpen] = useState(false);
  const importInputRef = useRef<HTMLInputElement>(null);

  const activeTaxOptions = useMemo(() => taxOptions.length ? taxOptions : fallbackTaxOptions, [taxOptions]);
  const formTaxRate = useMemo(() => form.tax_ids.reduce((sum, taxId) => {
    const tax = activeTaxOptions.find((option) => option.id === taxId);
    return sum + (tax?.rate ?? 0);
  }, 0), [activeTaxOptions, form.tax_ids]);
  const taxIdsForProduct = (product: Product) => {
    if (product.tax_ids?.length) return product.tax_ids;
    if (product.tax_rate <= 0) return [];
    const exact = activeTaxOptions.find((tax) => Math.abs(tax.rate - product.tax_rate) < 0.0001);
    if (exact) return [exact.id];
    for (let start = 0; start < activeTaxOptions.length; start += 1) {
      for (let end = start + 1; end < activeTaxOptions.length; end += 1) {
        const sum = activeTaxOptions[start].rate + activeTaxOptions[end].rate;
        if (Math.abs(sum - product.tax_rate) < 0.0001) return [activeTaxOptions[start].id, activeTaxOptions[end].id];
      }
    }
    return [];
  };
  const formatTaxPercent = (rate: number) => `${Math.round(rate * 1000) / 10}%`;
  const taxBreakdownForProduct = (product: Product) => {
    const ids = taxIdsForProduct(product);
    return ids.reduce(
      (summary, taxId) => {
        const tax = activeTaxOptions.find((option) => option.id === taxId);
        if (tax?.type === "IVA") summary.iva += tax.rate;
        if (tax?.type === "IEPS") summary.ieps += tax.rate;
        return summary;
      },
      { iva: 0, ieps: 0 },
    );
  };

  const newProductForm = useCallback(() => ({
    ...emptyProductForm,
    tax_ids: taxEnabled && taxAutoApply
      ? activeTaxOptions.filter((tax) => tax.rate === taxDefaultRate).slice(0, 1).map((tax) => tax.id)
      : [],
    tax_rate: taxEnabled && taxAutoApply ? taxDefaultRate : 0,
  }), [activeTaxOptions, taxAutoApply, taxDefaultRate, taxEnabled]);

  useEffect(() => {
    Promise.all([
      getSetting("tax_enabled"),
      getSetting("tax_default_rate"),
      getSetting("tax_prices_include_tax"),
      getSetting("tax_auto_apply_new_products"),
      listTaxes(),
    ])
      .then(([nextEnabled, nextRate, nextPricesIncludeTax, nextAutoApply, nextTaxOptions]) => {
        const enabled = nextEnabled !== "false";
        const rate = Number(nextRate ?? 0.16);
        const autoApply = nextAutoApply !== "false";
        setTaxOptions(nextTaxOptions.filter((tax) => tax.is_active));
        setTaxEnabled(enabled);
        setTaxDefaultRate(rate);
        setTaxPricesIncludeTax(nextPricesIncludeTax !== "false");
        setTaxAutoApply(autoApply);
        const defaultTaxIds = nextTaxOptions.filter((tax) => tax.rate === rate).slice(0, 1).map((tax) => tax.id);
        setForm((current) => current.name ? current : { ...current, tax_ids: enabled && autoApply ? defaultTaxIds : [], tax_rate: enabled && autoApply ? rate : 0 });
      })
      .catch((error) => showToast(String(error)));
  }, [showToast]);

  const save = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    try {
      await upsertProduct({ ...form, tax_ids: taxEnabled ? form.tax_ids : [], tax_rate: taxEnabled ? formTaxRate : 0 });
      setForm(newProductForm());
      setEditorOpen(false);
      await refreshProducts("");
      showToast("Producto guardado");
    } catch (error) {
      showToast(String(error));
    } finally {
      setBusy(false);
    }
  };

  const remove = async (product: Product) => {
    requestConfirm({
      title: "Desactivar producto",
      message: `${product.name} deja de salir en busqueda y venta.`,
      confirmLabel: "Desactivar",
      tone: "danger",
      onConfirm: async () => {
        try {
          await deleteProduct(product.id);
          await refreshProducts("");
          showToast("Producto desactivado");
        } catch (error) {
          showToast(String(error));
        }
      },
    });
  };

  const exportProducts = () => {
    const header = ["sku", "barcode", "name", "category", "unit", "price", "cost", "stock", "tax_ids", "tax_rate", "active"];
    const rows = products.map((product) => [
      product.sku,
      product.barcode,
      product.name,
      product.category,
      product.unit,
      product.price,
      product.cost,
      product.stock,
      product.tax_ids.join("|"),
      product.tax_rate,
      product.active,
    ]);
    downloadCsv(`productos-rim-pos-${new Date().toISOString().slice(0, 10)}.csv`, [header, ...rows]);
  };

  const importProducts = async (file: File) => {
    try {
      const text = await file.text();
      const [headerLine, ...lines] = text.split(/\r?\n/).filter((line) => line.trim());
      if (!headerLine) throw new Error("CSV vacio");
      const headers = parseCsvLine(headerLine).map((header) => header.trim());
      let imported = 0;
      for (const line of lines) {
        const values = parseCsvLine(line);
        const row = Object.fromEntries(headers.map((header, index) => [header, values[index] ?? ""]));
        if (!row.name) continue;
        const barcode = normalizeBarcode(row.barcode || row.sku || "");
        const sku = (row.sku || barcode || `SKU-${Date.now()}-${imported}`).trim().toUpperCase();
        const name = row.name.trim().replace(/\s+/g, " ");
        await upsertProduct({
          sku,
          barcode,
          name,
          category: suggestCategory(name, row.category || "Abarrotes"),
          unit: row.unit || "pieza",
          price: Number(row.price || 0),
          cost: Number(row.cost || 0),
          stock: Number(row.stock || 0),
          min_stock: Number(row.min_stock || 0),
          tax_ids: String(row.tax_ids || "").split("|").map(Number).filter((value) => Number.isFinite(value) && value > 0),
          tax_rate: Number(row.tax_rate || 0),
          active: row.active !== "false",
        });
        imported += 1;
      }
      await refreshProducts(catalogQuery);
      showToast(`${imported} productos importados`);
    } catch (error) {
      showToast(String(error));
    } finally {
      if (importInputRef.current) importInputRef.current.value = "";
    }
  };

  return (
    <section className="admin-panel product-module">
      <div className="module-toolbar">
        <div>
          <h2>Catalogo de productos</h2>
          <p>Alta, busqueda, precios, codigos, departamentos e importacion.</p>
        </div>
        <div className="toolbar-actions">
          <input
            ref={importInputRef}
            className="hidden-file-input"
            type="file"
            accept=".csv,text/csv"
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (file) importProducts(file);
            }}
          />
          <button className="ghost-button" type="button" onClick={() => importInputRef.current?.click()}>Importar CSV</button>
          <button className="ghost-button" type="button" onClick={exportProducts}>Exportar CSV</button>
          <button className="ghost-button" type="button" onClick={() => setEditorOpen((current) => !current)}>
            {editorOpen ? "Ocultar formulario" : "Mostrar formulario"}
          </button>
          <button className="primary-button" type="button" onClick={() => { setForm(newProductForm()); setEditorOpen(true); }}>
          <PackagePlus size={18} />
          Nuevo producto
        </button>
        </div>
      </div>
      <form className="catalog-search" onSubmit={(event) => { event.preventDefault(); refreshProducts(catalogQuery).catch((error) => showToast(String(error))); }}>
        <Search size={18} />
        <input
          value={catalogQuery}
          onChange={(event) => {
            setCatalogQuery(event.target.value);
            refreshProducts(event.target.value).catch((error) => showToast(String(error)));
          }}
          placeholder="Buscar producto por nombre, codigo, SKU o departamento"
        />
      </form>
      {editorOpen && (
      <form className="product-editor" onSubmit={save}>
        <label className="field-span-2">
          Nombre
          <input value={form.name} onChange={(event) => setForm({ ...form, name: event.target.value })} />
        </label>
        <label>
          Codigo
          <input value={form.barcode} onChange={(event) => setForm({ ...form, barcode: event.target.value })} />
        </label>
        <label>
          SKU
          <input value={form.sku} onChange={(event) => setForm({ ...form, sku: event.target.value })} />
        </label>
        <label>
          Departamento
          <input value={form.category} onChange={(event) => setForm({ ...form, category: event.target.value })} />
        </label>
        <label>
          Precio de venta
          <input type="number" step="0.01" value={form.price} onChange={(event) => setForm({ ...form, price: Number(event.target.value) })} />
        </label>
        <label>
          Precio de compra
          <input type="number" step="0.01" value={form.cost} onChange={(event) => setForm({ ...form, cost: Number(event.target.value) })} />
        </label>
        <label>
          Stock
          <input type="number" step="1" value={form.stock} onChange={(event) => setForm({ ...form, stock: Number(event.target.value) })} />
        </label>
        <div className="tax-picker" role="group" aria-label="Impuestos incluidos">
          <span className="tax-picker-title">Impuestos incluidos</span>
          {activeTaxOptions.map((tax) => (
            <label key={tax.id}>
              <input
                type="checkbox"
                checked={form.tax_ids.includes(tax.id)}
                disabled={!taxEnabled}
                onChange={(event) => {
                  const nextIds = event.target.checked
                    ? [...form.tax_ids, tax.id]
                    : form.tax_ids.filter((taxId) => taxId !== tax.id);
                  setForm({ ...form, tax_ids: nextIds, tax_rate: nextIds.reduce((sum, taxId) => sum + (activeTaxOptions.find((option) => option.id === taxId)?.rate ?? 0), 0) });
                }}
              />
              <span>{tax.name}</span>
            </label>
          ))}
          <strong>{Math.round(formTaxRate * 1000) / 10}% total</strong>
        </div>
        <label>
          Unidad
          <select value={form.unit} onChange={(event) => setForm({ ...form, unit: event.target.value })}>
            <option value="pieza">pieza</option>
            <option value="kg">kg</option>
            <option value="litro">litro</option>
          </select>
        </label>
        <button className="primary-button form-submit" type="submit" disabled={busy}>
          Guardar producto
        </button>
      </form>
      )}
      <div className="data-table">
        <div className="table-head catalog-row">
          <span>Producto</span>
          <span>Codigo</span>
          <span>Departamento</span>
          <span>Compra</span>
          <span>Venta</span>
          <span>IVA</span>
          <span>IEPS</span>
        </div>
        {products.length === 0 ? (
          <div className="table-empty">Sin productos activos</div>
        ) : products.map((product) => (
          <div className="catalog-row" key={product.id}>
            <strong>{product.name}</strong>
            <span>{product.barcode}</span>
            <span>{product.category}</span>
            <span>{money(product.cost)}</span>
            <span>{money(product.price)}</span>
            <span>{formatTaxPercent(taxBreakdownForProduct(product).iva)}</span>
            <span>{formatTaxPercent(taxBreakdownForProduct(product).ieps)}</span>
            <button className="ghost-button row-action" type="button" onClick={() => {
              setForm({
                ...product,
                tax_ids: taxIdsForProduct(product),
              });
              setEditorOpen(true);
            }}>
              Editar
            </button>
            <button className="icon-button danger" type="button" onClick={() => remove(product)} aria-label={`Desactivar ${product.name}`}>
              <Trash2 size={16} />
            </button>
          </div>
        ))}
      </div>
    </section>
  );
}
