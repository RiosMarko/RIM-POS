import { AlertTriangle, CheckCircle2, PackagePlus, Search, Trash2 } from "lucide-react";
import { FormEvent, memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ConfirmDraft } from "../../components/modals/CommonModals";
import { AdminGate } from "../auth/AuthScreens";
import { parseCsvLine } from "../../lib/csv";
import { money } from "../../lib/money";
import { selectNumericInput } from "../../lib/numberInput";
import { bulkImportProducts, deleteProduct, getSetting, listTaxes, upsertProduct, validateProductImport } from "../../lib/posApi";
import type { ProductSearchOptions } from "../../lib/posApi";
import { downloadXlsx, eleventaRowsFromProducts, parseEleventaCatalogXlsx } from "../../lib/xlsx";
import type { Product, ProductImportIssue, ProductImportResult, ProductImportRow, TaxOption } from "../../types";

const emptyProductForm = {
  sku: "",
  barcode: "",
  name: "",
  category: "Abarrotes",
  unit: "pieza",
  price: 0,
  wholesale_price: null as number | null,
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

const PRODUCT_PAGE_SIZE = 50;
const PRODUCT_LOAD_MORE_SIZE = 100;
const PRODUCT_VIEW_ALL_LIMIT = 50_000;
const PRODUCT_SEARCH_DEBOUNCE_MS = 180;

const normalizeBarcode = (value: string) => value.replace(/[^0-9A-Za-z]/g, "").trim();

const emptyBulkEdit = {
  category: "",
  unit: "",
  updateTaxes: false,
  tax_ids: [] as number[],
};

type ImportPreview = {
  fileName: string;
  rows: ProductImportRow[];
  issues: ProductImportIssue[];
};

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

const formatTaxPercent = (rate: number) => `${Math.round(rate * 1000) / 10}%`;

type ProductRowProps = {
  product: Product;
  selected: boolean;
  ivaLabel: string;
  iepsLabel: string;
  onToggle: (productId: number) => void;
  onEdit: (product: Product) => void;
  onRemove: (product: Product) => void;
};

const ProductRow = memo(function ProductRow({
  product,
  selected,
  ivaLabel,
  iepsLabel,
  onToggle,
  onEdit,
  onRemove,
}: ProductRowProps) {
  return (
    <div className={`catalog-row ${selected ? "selected" : ""}`}>
      <label className="row-checkbox" aria-label={`Seleccionar ${product.name}`}>
        <input
          type="checkbox"
          checked={selected}
          onChange={() => onToggle(product.id)}
        />
      </label>
      <strong>{product.name}</strong>
      <span>{product.barcode}</span>
      <span>{product.category}</span>
      <span>{money(product.cost)}</span>
      <span>{money(product.price)}</span>
      <span>{product.wholesale_price ? money(product.wholesale_price) : "-"}</span>
      <span>{ivaLabel}</span>
      <span>{iepsLabel}</span>
      <button className="ghost-button row-action" type="button" onClick={() => onEdit(product)}>
        Editar
      </button>
      <button className="icon-button danger" type="button" onClick={() => onRemove(product)} aria-label={`Desactivar ${product.name}`}>
        <Trash2 size={16} />
      </button>
    </div>
  );
});

export function ProductsView({
  products,
  refreshProducts,
  showToast,
  requestConfirm,
}: {
  products: Product[];
  refreshProducts: (query?: string, options?: ProductSearchOptions) => Promise<Product[]>;
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
  const [catalogLoading, setCatalogLoading] = useState(false);
  const [catalogQuery, setCatalogQuery] = useState("");
  const [catalogLimit, setCatalogLimit] = useState(PRODUCT_PAGE_SIZE);
  const [selectedProductIds, setSelectedProductIds] = useState<Set<number>>(new Set());
  const [bulkEdit, setBulkEdit] = useState(emptyBulkEdit);
  const [editorOpen, setEditorOpen] = useState(false);
  const [importPreview, setImportPreview] = useState<ImportPreview | null>(null);
  const [lastImportResult, setLastImportResult] = useState<ProductImportResult | null>(null);
  const [deleteAdminDraft, setDeleteAdminDraft] = useState<Product | null>(null);
  const [bulkDeleteAdminDraft, setBulkDeleteAdminDraft] = useState<Product[] | null>(null);
  const importInputRef = useRef<HTMLInputElement>(null);
  const catalogLoadingRef = useRef(false);
  const catalogSearchTimerRef = useRef<number | null>(null);
  const pageProducts = useMemo(() => products.slice(0, catalogLimit), [catalogLimit, products]);
  const catalogPageEnd = pageProducts.length;
  const hasMoreProducts = products.length > catalogLimit;
  const selectedProducts = useMemo(() => pageProducts.filter((product) => selectedProductIds.has(product.id)), [pageProducts, selectedProductIds]);
  const allPageSelected = useMemo(
    () => pageProducts.length > 0 && pageProducts.every((product) => selectedProductIds.has(product.id)),
    [pageProducts, selectedProductIds],
  );

  const loadCatalog = useCallback(async (query: string, limit = catalogLimit) => {
    catalogLoadingRef.current = true;
    setCatalogLoading(true);
    try {
      return await refreshProducts(query, { limit: limit + 1, offset: 0 });
    } finally {
      catalogLoadingRef.current = false;
      setCatalogLoading(false);
    }
  }, [catalogLimit, refreshProducts]);

  const queueCatalogLoad = useCallback((query: string) => {
    if (catalogSearchTimerRef.current) window.clearTimeout(catalogSearchTimerRef.current);
    catalogSearchTimerRef.current = window.setTimeout(() => {
      catalogSearchTimerRef.current = null;
      loadCatalog(query, PRODUCT_PAGE_SIZE).catch((error) => showToast(String(error)));
    }, PRODUCT_SEARCH_DEBOUNCE_MS);
  }, [loadCatalog, showToast]);

  const activeTaxOptions = useMemo(() => taxOptions.length ? taxOptions : fallbackTaxOptions, [taxOptions]);
  const formTaxRate = useMemo(() => form.tax_ids.reduce((sum, taxId) => {
    const tax = activeTaxOptions.find((option) => option.id === taxId);
    return sum + (tax?.rate ?? 0);
  }, 0), [activeTaxOptions, form.tax_ids]);
  const taxIdsForProduct = useCallback((product: Product) => {
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
  }, [activeTaxOptions]);
  const activeTaxById = useMemo(
    () => new Map(activeTaxOptions.map((tax) => [tax.id, tax])),
    [activeTaxOptions],
  );
  const taxLabelsByProductId = useMemo(() => {
    const labels = new Map<number, { iva: string; ieps: string }>();
    for (const product of pageProducts) {
      let iva = 0;
      let ieps = 0;
      for (const taxId of taxIdsForProduct(product)) {
        const tax = activeTaxById.get(taxId);
        if (tax?.type === "IVA") iva += tax.rate;
        if (tax?.type === "IEPS") ieps += tax.rate;
      }
      labels.set(product.id, { iva: formatTaxPercent(iva), ieps: formatTaxPercent(ieps) });
    }
    return labels;
  }, [activeTaxById, pageProducts, taxIdsForProduct]);

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

  useEffect(() => {
    const visibleIds = new Set(pageProducts.map((product) => product.id));
    setSelectedProductIds((current) => new Set(Array.from(current).filter((id) => visibleIds.has(id))));
  }, [pageProducts]);

  useEffect(() => () => {
    if (catalogSearchTimerRef.current) window.clearTimeout(catalogSearchTimerRef.current);
  }, []);

  const save = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    try {
      const barcode = normalizeBarcode(form.barcode);
      await upsertProduct({ ...form, sku: barcode, barcode, tax_ids: taxEnabled ? form.tax_ids : [], tax_rate: taxEnabled ? formTaxRate : 0 });
      setForm(newProductForm());
      setEditorOpen(false);
      setCatalogLimit(PRODUCT_PAGE_SIZE);
      await loadCatalog("", PRODUCT_PAGE_SIZE);
      showToast("Producto guardado");
    } catch (error) {
      showToast(String(error));
    } finally {
      setBusy(false);
    }
  };

  const deleteProductAsAdmin = useCallback(async (product: Product, actorId: number) => {
    try {
      await deleteProduct(product.id, actorId);
      setCatalogLimit(PRODUCT_PAGE_SIZE);
      await loadCatalog("", PRODUCT_PAGE_SIZE);
      showToast("Producto desactivado");
    } catch (error) {
      showToast(String(error));
    }
  }, [loadCatalog, showToast]);

  const deleteProductsAsAdmin = useCallback(async (targetProducts: Product[], actorId: number) => {
    setBusy(true);
    try {
      await Promise.all(targetProducts.map((product) => deleteProduct(product.id, actorId)));
      setSelectedProductIds(new Set());
      await loadCatalog(catalogQuery);
      showToast(`${targetProducts.length} productos borrados`);
    } catch (error) {
      showToast(String(error));
    } finally {
      setBusy(false);
    }
  }, [catalogQuery, loadCatalog, showToast]);

  const remove = useCallback(async (product: Product) => {
    requestConfirm({
      title: "Desactivar producto",
      message: `${product.name} deja de salir en busqueda y venta.`,
      confirmLabel: "Desactivar",
      tone: "danger",
      onConfirm: async () => setDeleteAdminDraft(product),
    });
  }, [requestConfirm]);

  const toggleProductSelection = useCallback((productId: number) => {
    setSelectedProductIds((current) => {
      const next = new Set(current);
      if (next.has(productId)) next.delete(productId);
      else next.add(productId);
      return next;
    });
  }, []);

  const editProduct = useCallback((product: Product) => {
    setForm({
      ...product,
      wholesale_price: product.wholesale_price ?? null,
      tax_ids: taxIdsForProduct(product),
    });
    setEditorOpen(true);
  }, [taxIdsForProduct]);

  const toggleSelectPage = () => {
    setSelectedProductIds((current) => {
      if (allPageSelected) return new Set();
      const next = new Set(current);
      pageProducts.forEach((product) => next.add(product.id));
      return next;
    });
  };

  const applyBulkEdit = async () => {
    if (selectedProducts.length === 0) {
      showToast("Selecciona productos");
      return;
    }
    const nextCategory = bulkEdit.category.trim();
    const nextUnit = bulkEdit.unit.trim();
    const nextTaxRate = bulkEdit.tax_ids.reduce((sum, taxId) => {
      const tax = activeTaxOptions.find((option) => option.id === taxId);
      return sum + (tax?.rate ?? 0);
    }, 0);
    const hasChanges = Boolean(nextCategory || nextUnit || bulkEdit.updateTaxes);
    if (!hasChanges) {
      showToast("Elige al menos un cambio");
      return;
    }
    setBusy(true);
    try {
      await Promise.all(selectedProducts.map((product) => upsertProduct({
        ...product,
        category: nextCategory || product.category,
        unit: nextUnit || product.unit,
        tax_ids: bulkEdit.updateTaxes ? bulkEdit.tax_ids : taxIdsForProduct(product),
        tax_rate: bulkEdit.updateTaxes ? nextTaxRate : product.tax_rate,
      })));
      setBulkEdit(emptyBulkEdit);
      setSelectedProductIds(new Set());
      await loadCatalog(catalogQuery);
      showToast(`${selectedProducts.length} productos actualizados`);
    } catch (error) {
      showToast(String(error));
    } finally {
      setBusy(false);
    }
  };

  const removeSelected = () => {
    if (selectedProducts.length === 0) {
      showToast("Selecciona productos");
      return;
    }
    requestConfirm({
      title: "Borrar productos",
      message: `Vas a borrar ${selectedProducts.length} productos. Ya no saldran en busqueda ni venta.`,
      confirmLabel: `Borrar ${selectedProducts.length}`,
      tone: "danger",
      onConfirm: async () => setBulkDeleteAdminDraft(selectedProducts),
    });
  };

  const exportProducts = () => {
    downloadXlsx(`catalogo-rim-pos-${new Date().toISOString().slice(0, 10)}.xlsx`, eleventaRowsFromProducts(pageProducts, activeTaxOptions));
  };

  const parseCsvImportFile = async (file: File) => {
    const text = await file.text();
    const [headerLine, ...lines] = text.split(/\r?\n/).filter((line) => line.trim());
    if (!headerLine) throw new Error("CSV vacio");
    const headers = parseCsvLine(headerLine).map((header) => header.trim());
    const rows = lines.flatMap((line, index): ProductImportRow[] => {
      const values = parseCsvLine(line);
      const row = Object.fromEntries(headers.map((header, index) => [header, values[index] ?? ""]));
      const rowNumber = index + 2;
      if (!row.name) return [];
      const barcode = normalizeBarcode(row.barcode || row["Código"] || row.codigo || "");
      const name = row.name.trim().replace(/\s+/g, " ");
      return [{
        row_number: rowNumber,
        sku: barcode,
        barcode,
        name,
        category: suggestCategory(name, row.category || "Abarrotes"),
        unit: row.unit || "pieza",
        price: Number(row.price || 0),
        wholesale_price: row.wholesale_price ? Number(row.wholesale_price) : null,
        cost: Number(row.cost || 0),
        stock: Number(row.stock || 0),
        min_stock: Number(row.min_stock || 0),
        tax_ids: String(row.tax_ids || "").split("|").map(Number).filter((value) => Number.isFinite(value) && value > 0),
        tax_rate: Number(row.tax_rate || 0),
        active: row.active !== "false",
      }];
    });
    return { rows, issues: [] as ProductImportIssue[] };
  };

  const parseImportFile = async (file: File) => {
    try {
      const isXlsx = file.name.toLowerCase().endsWith(".xlsx");
      const parsed = isXlsx
        ? { rows: await parseEleventaCatalogXlsx(file, activeTaxOptions), issues: [] }
        : await parseCsvImportFile(file);
      const nextIssues = [...parsed.issues];
      const seenBarcodes = new Set<string>();
      parsed.rows.forEach((row) => {
        const issue = (message: string) => nextIssues.push({ row_number: row.row_number, sku: "", barcode: row.barcode, message });
        if (row.barcode.trim().length < 1) issue("Codigo requerido");
        if (row.name.trim().length < 2) issue("Nombre requerido");
        if ([row.price, row.wholesale_price ?? 0, row.cost, row.stock, row.min_stock, row.tax_rate].some((value) => !Number.isFinite(value) || value < 0)) {
          issue("Importe o existencia invalida");
        }
        const barcodeKey = row.barcode.trim();
        if (seenBarcodes.has(barcodeKey)) issue("Codigo duplicado en archivo");
        seenBarcodes.add(barcodeKey);
      });
      if (parsed.rows.length === 0) {
        nextIssues.push({ row_number: 0, sku: "", barcode: "", message: "Archivo sin productos" });
      }
      if (nextIssues.length === 0) {
        const dbValidation = await validateProductImport(parsed.rows);
        nextIssues.push(...dbValidation.issues);
      }
      setLastImportResult(null);
      setImportPreview({ fileName: file.name, rows: parsed.rows, issues: nextIssues });
      showToast(`${parsed.rows.length} filas listas para revisar`);
    } catch (error) {
      showToast(String(error));
    } finally {
      if (importInputRef.current) importInputRef.current.value = "";
    }
  };

  const commitImport = async () => {
    if (!importPreview) return;
    if (importPreview.issues.length > 0) {
      showToast("Corrige errores antes de importar");
      return;
    }
    setBusy(true);
    try {
      const result = await bulkImportProducts(importPreview.rows);
      setLastImportResult(result);
      if (result.committed) {
        setImportPreview(null);
        await loadCatalog(catalogQuery);
        showToast(`${result.imported} productos importados`);
      } else {
        setImportPreview({ ...importPreview, issues: result.issues });
        showToast("Importacion detenida, no se guardo ningun producto");
      }
    } catch (error) {
      showToast(String(error));
    } finally {
      setBusy(false);
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
            accept=".csv,text/csv,.xlsx,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (file) parseImportFile(file);
            }}
          />
          <button className="ghost-button" type="button" onClick={() => importInputRef.current?.click()}>Importar catalogo</button>
          <button className="ghost-button" type="button" onClick={exportProducts}>Exportar Excel</button>
          <button className="ghost-button" type="button" onClick={() => setEditorOpen((current) => !current)}>
            {editorOpen ? "Ocultar formulario" : "Mostrar formulario"}
          </button>
          <button className="primary-button" type="button" onClick={() => { setForm(newProductForm()); setEditorOpen(true); }}>
          <PackagePlus size={18} />
          Nuevo producto
        </button>
        </div>
      </div>
      <form className="catalog-search" onSubmit={(event) => {
        event.preventDefault();
        if (catalogSearchTimerRef.current) window.clearTimeout(catalogSearchTimerRef.current);
        setCatalogLimit(PRODUCT_PAGE_SIZE);
        loadCatalog(catalogQuery, PRODUCT_PAGE_SIZE).catch((error) => showToast(String(error)));
      }}>
        <Search size={18} />
        <input
          value={catalogQuery}
          onChange={(event) => {
            const nextQuery = event.target.value;
            setCatalogQuery(nextQuery);
            setCatalogLimit(PRODUCT_PAGE_SIZE);
            queueCatalogLoad(nextQuery);
          }}
          placeholder="Buscar producto por nombre, codigo o departamento"
        />
      </form>
      {importPreview && (
        <section className="import-review" aria-label="Revision de importacion CSV">
          <div className="import-review-header">
            <div>
              <h3>{importPreview.fileName}</h3>
              <p>
                {importPreview.rows.length} filas detectadas. La importacion guarda todo o nada.
              </p>
            </div>
            <div className={importPreview.issues.length ? "import-status warning" : "import-status success"}>
              {importPreview.issues.length ? <AlertTriangle size={18} /> : <CheckCircle2 size={18} />}
              <span>
                {importPreview.issues.length ? `${importPreview.issues.length} errores` : "Listo para importar"}
              </span>
            </div>
          </div>
          {importPreview.issues.length > 0 ? (
            <div className="import-issues" role="status">
              {importPreview.issues.slice(0, 8).map((issue) => (
                <div className="import-issue-row" key={`${issue.row_number}-${issue.message}-${issue.barcode}`}>
                  <strong>Fila {issue.row_number}</strong>
                  <span>{issue.barcode || "sin codigo"}</span>
                  <em>{issue.message}</em>
                </div>
              ))}
              {importPreview.issues.length > 8 && (
                <div className="import-issue-row muted">
                  <strong>+{importPreview.issues.length - 8}</strong>
                  <span>errores mas</span>
                  <em>Corrige CSV y vuelve a cargarlo</em>
                </div>
              )}
            </div>
          ) : (
            <div className="import-preview-grid" role="status">
              <div>
                <span>Nuevas/actualizadas</span>
                <strong>{importPreview.rows.length}</strong>
              </div>
              <div>
                <span>Primera fila</span>
                <strong>{importPreview.rows[0]?.name ?? "Sin productos"}</strong>
              </div>
            </div>
          )}
          <div className="import-review-actions">
            <button className="ghost-button" type="button" onClick={() => setImportPreview(null)}>
              Cancelar importacion
            </button>
            <button className="primary-button" type="button" disabled={busy || importPreview.issues.length > 0} onClick={commitImport}>
              Importar productos
            </button>
          </div>
        </section>
      )}
      {lastImportResult && !lastImportResult.committed && (
        <section className="import-review import-review-error" aria-label="Errores de importacion">
          <div className="import-review-header">
            <div>
              <h3>Importacion detenida</h3>
              <p>No se guardo ningun producto. Corrige CSV y vuelve a intentarlo.</p>
            </div>
            <div className="import-status warning">
              <AlertTriangle size={18} />
              <span>{lastImportResult.failed} errores</span>
            </div>
          </div>
        </section>
      )}
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
          Departamento
          <input value={form.category} onChange={(event) => setForm({ ...form, category: event.target.value })} />
        </label>
        <label>
          Precio de venta
          <input type="number" step="0.01" value={form.price === 0 ? "" : form.price} onFocus={selectNumericInput} onChange={(event) => setForm({ ...form, price: Number(event.target.value) })} />
        </label>
        <label>
          Precio Mayoreo
          <input type="number" step="0.01" value={form.wholesale_price ? form.wholesale_price : ""} onFocus={selectNumericInput} onChange={(event) => setForm({ ...form, wholesale_price: event.target.value === "" ? null : Number(event.target.value) })} />
        </label>
        <label>
          Precio de compra
          <input type="number" step="0.01" value={form.cost === 0 ? "" : form.cost} onFocus={selectNumericInput} onChange={(event) => setForm({ ...form, cost: Number(event.target.value) })} />
        </label>
        <label>
          Existencia
          <input type="number" step="1" value={form.stock === 0 ? "" : form.stock} onFocus={selectNumericInput} onChange={(event) => setForm({ ...form, stock: Number(event.target.value) })} />
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
      {selectedProducts.length > 0 && (
      <section className="bulk-product-bar" aria-label="Acciones masivas de productos">
        <div className="bulk-product-summary">
          <strong>{selectedProducts.length} seleccionados</strong>
          <span>{pageProducts.length} visibles</span>
        </div>
        <div className="bulk-product-fields">
          <label>
            Departamento
            <input value={bulkEdit.category} onChange={(event) => setBulkEdit({ ...bulkEdit, category: event.target.value })} placeholder="Sin cambio" />
          </label>
          <label>
            Unidad
            <select value={bulkEdit.unit} onChange={(event) => setBulkEdit({ ...bulkEdit, unit: event.target.value })}>
              <option value="">Sin cambio</option>
              <option value="pieza">pieza</option>
              <option value="kg">kg</option>
              <option value="litro">litro</option>
            </select>
          </label>
          <label className="bulk-tax-toggle">
            <input
              type="checkbox"
              checked={bulkEdit.updateTaxes}
              onChange={(event) => setBulkEdit({ ...bulkEdit, updateTaxes: event.target.checked })}
            />
            Cambiar impuestos
          </label>
          {bulkEdit.updateTaxes && (
            <div className="bulk-tax-options" role="group" aria-label="Impuestos masivos">
              {activeTaxOptions.map((tax) => (
                <label key={tax.id}>
                  <input
                    type="checkbox"
                    checked={bulkEdit.tax_ids.includes(tax.id)}
                    onChange={(event) => {
                      const nextIds = event.target.checked
                        ? [...bulkEdit.tax_ids, tax.id]
                        : bulkEdit.tax_ids.filter((taxId) => taxId !== tax.id);
                      setBulkEdit({ ...bulkEdit, tax_ids: nextIds });
                    }}
                  />
                  <span>{tax.name}</span>
                </label>
              ))}
              <strong>{formatTaxPercent(bulkEdit.tax_ids.reduce((sum, taxId) => sum + (activeTaxOptions.find((option) => option.id === taxId)?.rate ?? 0), 0))} total</strong>
            </div>
          )}
        </div>
        <div className="bulk-product-actions">
          <button className="ghost-button" type="button" disabled={busy || selectedProducts.length === 0} onClick={applyBulkEdit}>
            Aplicar cambios
          </button>
          <button className="danger-button" type="button" disabled={busy || selectedProducts.length === 0} onClick={removeSelected}>
            Borrar
          </button>
        </div>
      </section>
      )}
      <div className="data-table">
        <div className="table-head catalog-row">
          <label className="row-checkbox header-checkbox" aria-label="Seleccionar todos los productos visibles">
            <input
              type="checkbox"
              checked={allPageSelected}
              disabled={pageProducts.length === 0}
              onChange={toggleSelectPage}
            />
          </label>
          <span>Producto</span>
          <span>Codigo</span>
          <span>Departamento</span>
          <span>Compra</span>
          <span>Venta</span>
          <span>Mayoreo</span>
          <span>IVA</span>
          <span>IEPS</span>
        </div>
        {pageProducts.length === 0 ? (
          <div className="table-empty">Sin productos activos</div>
        ) : pageProducts.map((product) => {
          const labels = taxLabelsByProductId.get(product.id) ?? { iva: "0%", ieps: "0%" };
          return (
            <ProductRow
              key={product.id}
              product={product}
              selected={selectedProductIds.has(product.id)}
              ivaLabel={labels.iva}
              iepsLabel={labels.ieps}
              onToggle={toggleProductSelection}
              onEdit={editProduct}
              onRemove={remove}
            />
          );
        })}
      </div>
      <div className="table-pagination">
        <span>{pageProducts.length === 0 ? "Sin resultados" : `${catalogPageEnd} productos visibles por codigo${catalogLoading ? "..." : ""}`}</span>
        <div>
          <button
            className="ghost-button"
            type="button"
            disabled={!hasMoreProducts || catalogLoading}
            onClick={() => {
              const nextLimit = catalogLimit + PRODUCT_LOAD_MORE_SIZE;
              loadCatalog(catalogQuery, nextLimit)
                .then((result) => setCatalogLimit(Math.min(nextLimit, Math.max(result.length, PRODUCT_PAGE_SIZE))))
                .catch((error) => showToast(String(error)));
            }}
          >
            Ver mas
          </button>
          <button
            className="ghost-button"
            type="button"
            disabled={!hasMoreProducts || catalogLoading}
            onClick={() => {
              loadCatalog(catalogQuery, PRODUCT_VIEW_ALL_LIMIT)
                .then((result) => setCatalogLimit(Math.max(result.length, PRODUCT_PAGE_SIZE)))
                .catch((error) => showToast(String(error)));
            }}
          >
            Ver todo
          </button>
        </div>
      </div>
      {deleteAdminDraft && (
        <AdminGate
          targetLabel="borrar producto"
          onCancel={() => setDeleteAdminDraft(null)}
          onSuccess={(adminSession) => {
            const product = deleteAdminDraft;
            setDeleteAdminDraft(null);
            deleteProductAsAdmin(product, adminSession.id);
          }}
          showToast={showToast}
        />
      )}
      {bulkDeleteAdminDraft && (
        <AdminGate
          targetLabel="borrar productos"
          onCancel={() => setBulkDeleteAdminDraft(null)}
          onSuccess={(adminSession) => {
            const targetProducts = bulkDeleteAdminDraft;
            setBulkDeleteAdminDraft(null);
            deleteProductsAsAdmin(targetProducts, adminSession.id);
          }}
          showToast={showToast}
        />
      )}
    </section>
  );
}
