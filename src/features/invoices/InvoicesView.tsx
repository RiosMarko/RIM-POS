import { FileText } from "lucide-react";
import { FormEvent, useCallback, useEffect, useState } from "react";
import { money } from "../../lib/money";
import { createInvoiceDraft, getSetting, listCustomers, listInvoices, listSales, setSetting } from "../../lib/posApi";
import type { Customer, InvoiceDraft, SaleListItem } from "../../types";

export function InvoicesView({ showToast }: { showToast: (message: string) => void }) {
  const [sales, setSales] = useState<SaleListItem[]>([]);
  const [customers, setCustomers] = useState<Customer[]>([]);
  const [invoices, setInvoices] = useState<InvoiceDraft[]>([]);
  const [saleId, setSaleId] = useState(0);
  const [customerId, setCustomerId] = useState<number | "">("");
  const [fiscalForm, setFiscalForm] = useState({
    rfc: "",
    fiscal_regime: "",
    fiscal_postal_code: "",
    default_cfdi_use: "G03",
    invoice_series: "A",
  });

  const refresh = useCallback(async () => {
    const [nextSales, nextCustomers, nextInvoices] = await Promise.all([listSales(), listCustomers(), listInvoices()]);
    setSales(nextSales.filter((sale) => sale.status === "paid"));
    setCustomers(nextCustomers);
    setInvoices(nextInvoices);
    if (!saleId && nextSales[0]) setSaleId(nextSales[0].id);
  }, [saleId]);

  useEffect(() => {
    refresh().catch((error) => showToast(String(error)));
    Promise.all([
      getSetting("company_rfc"),
      getSetting("company_fiscal_regime"),
      getSetting("company_fiscal_postal_code"),
      getSetting("default_cfdi_use"),
      getSetting("invoice_series"),
    ])
      .then(([rfc, fiscalRegime, fiscalPostalCode, defaultCfdiUse, invoiceSeries]) => {
        setFiscalForm({
          rfc: rfc ?? "",
          fiscal_regime: fiscalRegime ?? "",
          fiscal_postal_code: fiscalPostalCode ?? "",
          default_cfdi_use: defaultCfdiUse ?? "G03",
          invoice_series: invoiceSeries ?? "A",
        });
      })
      .catch((error) => showToast(String(error)));
  }, [refresh, showToast]);

  const saveFiscal = async (event: FormEvent) => {
    event.preventDefault();
    try {
      await setSetting("company_rfc", fiscalForm.rfc);
      await setSetting("company_fiscal_regime", fiscalForm.fiscal_regime);
      await setSetting("company_fiscal_postal_code", fiscalForm.fiscal_postal_code);
      await setSetting("default_cfdi_use", fiscalForm.default_cfdi_use);
      await setSetting("invoice_series", fiscalForm.invoice_series);
      showToast("Datos fiscales guardados");
    } catch (error) {
      showToast(String(error));
    }
  };

  const prepareInvoice = async () => {
    if (!saleId) {
      showToast("Selecciona venta pagada");
      return;
    }
    try {
      const invoice = await createInvoiceDraft(saleId, customerId || null);
      await refresh();
      showToast(invoice.pac_message);
    } catch (error) {
      showToast(String(error));
    }
  };

  return (
    <section className="admin-panel invoice-module">
      <div className="module-toolbar">
        <div>
          <h2>Facturacion CFDI</h2>
          <p>Borradores fiscales ligados a ventas. Timbrado requiere PAC real.</p>
        </div>
        <button className="ghost-button" type="button" onClick={() => refresh().catch((error) => showToast(String(error)))}>
          Actualizar
        </button>
      </div>

      <div className="invoice-layout">
        <form className="user-form fiscal-form" onSubmit={saveFiscal}>
          <div>
            <h2>Empresa fiscal</h2>
            <p>Datos base para CFDI 4.0.</p>
          </div>
          <label>RFC<input value={fiscalForm.rfc} onChange={(event) => setFiscalForm({ ...fiscalForm, rfc: event.target.value.toUpperCase() })} /></label>
          <label className="field-span-2">Regimen fiscal<input value={fiscalForm.fiscal_regime} onChange={(event) => setFiscalForm({ ...fiscalForm, fiscal_regime: event.target.value })} /></label>
          <label>Codigo postal<input value={fiscalForm.fiscal_postal_code} onChange={(event) => setFiscalForm({ ...fiscalForm, fiscal_postal_code: event.target.value })} /></label>
          <label>Uso CFDI default<input value={fiscalForm.default_cfdi_use} onChange={(event) => setFiscalForm({ ...fiscalForm, default_cfdi_use: event.target.value.toUpperCase() })} /></label>
          <label>Serie<input value={fiscalForm.invoice_series} onChange={(event) => setFiscalForm({ ...fiscalForm, invoice_series: event.target.value.toUpperCase() })} /></label>
          <button className="primary-button form-submit" type="submit">Guardar datos fiscales</button>
        </form>

        <div className="user-form invoice-action-panel">
          <div>
            <h2>Facturar venta</h2>
            <p>Usa totales de venta; no recalcula por separado.</p>
          </div>
          <label>
            Venta pagada
            <select value={saleId} onChange={(event) => setSaleId(Number(event.target.value))}>
              <option value={0}>Seleccionar venta</option>
              {sales.map((sale) => (
                <option value={sale.id} key={sale.id}>{sale.folio} - {money(sale.total)}</option>
              ))}
            </select>
          </label>
          <label>
            Cliente fiscal
            <select value={customerId} onChange={(event) => setCustomerId(event.target.value ? Number(event.target.value) : "")}>
              <option value="">Publico general / factura global</option>
              {customers.map((customer) => (
                <option value={customer.id} key={customer.id}>{customer.name} {customer.rfc ? `- ${customer.rfc}` : ""}</option>
              ))}
            </select>
          </label>
          <button className="primary-button" type="button" onClick={prepareInvoice}>
            <FileText size={18} />
            Preparar CFDI
          </button>
          <div className="notice-box">
            CFDI no timbra sin PAC autorizado. Configura proveedor y credenciales reales antes de produccion.
          </div>
        </div>
      </div>

      <div className="data-table">
        <div className="table-head invoice-row">
          <span>Folio</span>
          <span>Venta</span>
          <span>Cliente</span>
          <span>Estado</span>
          <span>Total</span>
        </div>
        {invoices.length === 0 ? (
          <div className="table-empty">Sin borradores CFDI</div>
        ) : invoices.map((invoice) => (
          <div className="invoice-row" key={invoice.id}>
            <strong>{invoice.folio}</strong>
            <span>{invoice.sale_id ? `Venta ${invoice.sale_id}` : "Global"}</span>
            <span>{invoice.customer_name || "Publico general"}</span>
            <span>{invoice.status === "draft" ? "Borrador" : invoice.status}</span>
            <strong className="money-cell">{money(invoice.total)}</strong>
          </div>
        ))}
      </div>
    </section>
  );
}
