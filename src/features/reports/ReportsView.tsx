import {
  Archive,
  Banknote,
  CalendarDays,
  CircleDollarSign,
  CreditCard as CreditCardIcon,
  FileText,
  PackageCheck,
  Percent,
  ReceiptText,
  Search,
  ShoppingCart,
  TrendingDown,
  TrendingUp,
  WalletCards,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { LucideIcon } from "lucide-react";
import { downloadCsv } from "../../lib/csv";
import { formatDateMx, formatDateTimeMx } from "../../lib/date";
import { money } from "../../lib/money";
import { getMonthlySalesReport, getProductSalesReport, getTaxBreakdown, getUnsoldProductsReport, listReportMovements } from "../../lib/posApi";
import type { MonthlySalesReport, ProductSalesReport, ReportMovement, TaxBreakdown } from "../../types";

type ReportTab = "today" | "movements" | "products" | "cuts" | "monthly";
type DatePreset = "week" | "month" | "lastMonth" | "year" | "all";

const tabs: Array<{ key: ReportTab; label: string }> = [
  { key: "today", label: "Hoy" },
  { key: "movements", label: "Movimientos" },
  { key: "products", label: "Productos" },
  { key: "cuts", label: "Cortes" },
  { key: "monthly", label: "Mensual" },
];

const presetLabels: Array<{ key: DatePreset; label: string }> = [
  { key: "week", label: "Semana Actual" },
  { key: "month", label: "Mes Actual" },
  { key: "lastMonth", label: "Mes Anterior" },
  { key: "year", label: "Año actual" },
  { key: "all", label: "Todo" },
];

function dateKey(date: Date) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function parseDateKey(value: string) {
  if (!value) return null;
  const [year, month, day] = value.split("-").map(Number);
  if (!year || !month || !day) return null;
  return new Date(year, month - 1, day);
}

function addDays(date: Date, days: number) {
  const next = new Date(date);
  next.setDate(next.getDate() + days);
  return next;
}

function endOfMonth(date: Date) {
  return new Date(date.getFullYear(), date.getMonth() + 1, 0);
}

function monthRange(monthKey: string) {
  const [year, month] = monthKey.split("-").map(Number);
  if (!year || !month) return null;
  const start = new Date(year, month - 1, 1);
  return { from: dateKey(start), to: dateKey(endOfMonth(start)) };
}

function shortDateLabel(date: Date) {
  return formatDateMx(date);
}

function reportDateLabel(value: string) {
  const date = parseDateKey(value);
  if (!date) return "Inicio";
  return formatDateMx(date);
}

function movementTone(movement: ReportMovement) {
  if (movement.kind === "sale") return movement.amount >= 0 ? "sale" : "danger";
  if (movement.kind === "purchase" || movement.amount < 0) return "expense";
  if (movement.kind === "cash") return movement.amount === 0 ? "neutral" : movement.amount > 0 ? "cash-in" : "expense";
  if (movement.kind === "cut") return "cut";
  if (movement.kind === "inventory") return "inventory";
  return "neutral";
}

function movementLabel(kind: ReportMovement["kind"]) {
  if (kind === "sale") return "Venta";
  if (kind === "purchase") return "Compra";
  if (kind === "cash") return "Caja";
  if (kind === "inventory") return "Inventario";
  if (kind === "credit") return "Credito";
  return "Corte";
}

function movementIcon(kind: ReportMovement["kind"]): LucideIcon {
  if (kind === "sale") return ShoppingCart;
  if (kind === "purchase") return PackageCheck;
  if (kind === "cash") return Banknote;
  if (kind === "inventory") return Archive;
  if (kind === "credit") return WalletCards;
  return CalendarDays;
}

function ReportStat({
  icon: Icon,
  label,
  value,
  tone = "default",
}: {
  icon: LucideIcon;
  label: string;
  value: string;
  tone?: "default" | "success" | "danger" | "warning" | "info";
}) {
  return (
    <div className={`report-stat ${tone}`}>
      <Icon size={21} />
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

export function ReportsView({ showToast }: { showToast: (message: string) => void }) {
  const todayKey = dateKey(new Date());
  const monthStartKey = dateKey(new Date(new Date().getFullYear(), new Date().getMonth(), 1));
  const [products, setProducts] = useState<ProductSalesReport[]>([]);
  const [unsoldProducts, setUnsoldProducts] = useState<ProductSalesReport[]>([]);
  const [movements, setMovements] = useState<ReportMovement[]>([]);
  const [monthly, setMonthly] = useState<MonthlySalesReport[]>([]);
  const [taxBreakdown, setTaxBreakdown] = useState<TaxBreakdown[]>([]);
  const [activeTab, setActiveTab] = useState<ReportTab>("today");
  const [activePreset, setActivePreset] = useState<DatePreset | null>("month");
  const [filterKind, setFilterKind] = useState<"all" | ReportMovement["kind"]>("all");
  const [filterText, setFilterText] = useState("");
  const [fromDate, setFromDate] = useState(monthStartKey);
  const [toDate, setToDate] = useState(todayKey);
  const [selectedMonth, setSelectedMonth] = useState(todayKey.slice(0, 7));

  const refresh = useCallback(async () => {
    const range = { fromDate: fromDate || undefined, toDate: toDate || undefined };
    const [nextProducts, nextUnsoldProducts, nextMovements, nextMonthly, nextTaxBreakdown] = await Promise.all([
      getProductSalesReport(range),
      getUnsoldProductsReport(range),
      listReportMovements(range),
      getMonthlySalesReport(),
      getTaxBreakdown(range),
    ]);
    setProducts(nextProducts);
    setUnsoldProducts(nextUnsoldProducts);
    setMovements(nextMovements);
    setMonthly(nextMonthly);
    setTaxBreakdown(nextTaxBreakdown);
  }, [fromDate, toDate]);

  useEffect(() => {
    refresh().catch((error) => showToast(String(error)));
  }, [refresh, showToast]);

  const applyPreset = (preset: DatePreset) => {
    setActivePreset(preset);
    const now = new Date();
    if (preset === "week") {
      const start = new Date(now);
      const day = now.getDay() || 7;
      start.setDate(now.getDate() - day + 1);
      setFromDate(dateKey(start));
      setToDate(dateKey(now));
      return;
    }
    if (preset === "month") {
      const start = new Date(now.getFullYear(), now.getMonth(), 1);
      const end = endOfMonth(now);
      setFromDate(dateKey(start));
      setToDate(dateKey(end));
      setSelectedMonth(dateKey(start).slice(0, 7));
      return;
    }
    if (preset === "lastMonth") {
      const start = new Date(now.getFullYear(), now.getMonth() - 1, 1);
      const end = endOfMonth(start);
      setFromDate(dateKey(start));
      setToDate(dateKey(end));
      setSelectedMonth(dateKey(start).slice(0, 7));
      return;
    }
    if (preset === "year") {
      const start = new Date(now.getFullYear(), 0, 1);
      const end = new Date(now.getFullYear(), 11, 31);
      setFromDate(dateKey(start));
      setToDate(dateKey(end));
      return;
    }
    setFromDate("");
    setToDate("");
  };

  const applyMonth = (monthKey: string) => {
    setSelectedMonth(monthKey);
    const range = monthRange(monthKey);
    if (!range) return;
    setActivePreset(null);
    setFromDate(range.from);
    setToDate(range.to);
  };

  const periodLabel = useMemo(() => {
    if (activePreset) return presetLabels.find((preset) => preset.key === activePreset)?.label ?? "Periodo";
    if (fromDate && toDate && fromDate === toDate) return fromDate;
    if (fromDate || toDate) return `${fromDate || "Inicio"} - ${toDate || "Hoy"}`;
    return "Todo";
  }, [activePreset, fromDate, toDate]);
  const reportTitle = useMemo(() => (
    `Resumen de ventas del ${reportDateLabel(fromDate)} al ${reportDateLabel(toDate || todayKey)}`
  ), [fromDate, todayKey, toDate]);

  const normalizedFilterText = useMemo(() => filterText.trim().toLowerCase(), [filterText]);
  const periodMovements = useMemo(() => movements.filter((movement) => {
    const day = movement.created_at.slice(0, 10);
    if (fromDate && day < fromDate) return false;
    if (toDate && day > toDate) return false;
    return true;
  }), [fromDate, movements, toDate]);
  const filteredMovements = useMemo(() => periodMovements.filter((movement) => {
    if (filterKind !== "all" && movement.kind !== filterKind) return false;
    if (!normalizedFilterText) return true;
    return `${movement.title} ${movement.detail} ${movement.actor_name ?? ""}`.toLowerCase().includes(normalizedFilterText);
  }), [filterKind, normalizedFilterText, periodMovements]);

  const saleMovements = useMemo(() => periodMovements.filter((movement) => movement.kind === "sale" && movement.amount > 0), [periodMovements]);
  const expenseMovements = useMemo(() => periodMovements.filter((movement) => (
    movement.amount < 0 && (movement.kind === "cash" || movement.kind === "purchase")
  )), [periodMovements]);
  const cutMovements = useMemo(() => filteredMovements.filter((movement) => movement.kind === "cut"), [filteredMovements]);
  const salesTotal = useMemo(() => saleMovements.reduce((sum, movement) => sum + movement.amount, 0), [saleMovements]);
  const expensesTotal = useMemo(() => expenseMovements.reduce((sum, movement) => sum + Math.abs(movement.amount), 0), [expenseMovements]);
  const netTotal = salesTotal - expensesTotal;
  const ticketCount = saleMovements.length;
  const averageTicket = ticketCount > 0 ? salesTotal / ticketCount : 0;
  const grossProfit = useMemo(() => saleMovements.reduce((sum, movement) => sum + (movement.gross_profit ?? 0), 0), [saleMovements]);
  const marginPercent = salesTotal > 0 ? (grossProfit / salesTotal) * 100 : 0;
  const taxTotal = useMemo(() => saleMovements.reduce((sum, movement) => sum + (movement.tax_total ?? 0), 0), [saleMovements]);
  const filteredTotal = useMemo(
    () => filteredMovements.reduce((sum, movement) => sum + movement.amount, 0),
    [filteredMovements],
  );

  const departmentSales = useMemo(() => {
    const totalsByDepartment = new Map<string, { total: number; profit: number; quantity: number }>();
    products.forEach((product) => {
      const key = product.category || "- Sin Departamento -";
      const current = totalsByDepartment.get(key) ?? { total: 0, profit: 0, quantity: 0 };
      current.total += product.total;
      current.profit += product.gross_profit ?? 0;
      current.quantity += product.quantity;
      totalsByDepartment.set(key, current);
    });
    return Array.from(totalsByDepartment, ([department, value]) => ({ department, ...value }))
      .sort((left, right) => right.total - left.total);
  }, [products]);

  const terminalSales = useMemo(() => {
    const totalsByTerminal = new Map<string, number>();
    saleMovements.forEach((movement) => {
      const amount = movement.card_paid ?? 0;
      if (amount <= 0) return;
      const key = movement.card_terminal?.trim() || "Sin terminal";
      totalsByTerminal.set(key, (totalsByTerminal.get(key) ?? 0) + amount);
    });
    return Array.from(totalsByTerminal, ([terminal, total]) => ({ terminal, total }))
      .sort((left, right) => right.total - left.total);
  }, [saleMovements]);

  const dailySales = useMemo(() => {
    const explicitStart = parseDateKey(fromDate);
    const explicitEnd = parseDateKey(toDate);
    const saleDates = saleMovements
      .map((movement) => parseDateKey(movement.created_at.slice(0, 10)))
      .filter((date): date is Date => Boolean(date))
      .sort((left, right) => left.getTime() - right.getTime());
    const end = explicitEnd ?? saleDates[saleDates.length - 1] ?? new Date();
    const requestedStart = explicitStart ?? saleDates[0] ?? addDays(end, -6);
    const rangeDays = Math.max(1, Math.round((end.getTime() - requestedStart.getTime()) / 86400000) + 1);
    const bucketSize = rangeDays > 92 ? 30 : rangeDays > 14 ? 7 : 1;
    const monthPresetBuckets = activePreset === "month" && bucketSize === 7;
    const bucketCount = monthPresetBuckets ? 4 : Math.ceil(rangeDays / bucketSize);
    const buckets = Array.from({ length: bucketCount }, (_, index) => {
      const bucketStart = addDays(requestedStart, index * bucketSize);
      const bucketEnd = monthPresetBuckets && index === bucketCount - 1 ? end : addDays(bucketStart, bucketSize - 1);
      const cappedEnd = bucketEnd > end ? end : bucketEnd;
      const startKey = dateKey(bucketStart);
      const endKey = dateKey(cappedEnd);
      return {
        key: `${startKey}:${endKey}`,
        startKey,
        endKey,
        label: bucketSize === 1 ? shortDateLabel(bucketStart) : `${shortDateLabel(bucketStart)}-${shortDateLabel(cappedEnd)}`,
        total: 0,
        profit: 0,
      };
    });
    saleMovements.forEach((movement) => {
      const day = movement.created_at.slice(0, 10);
      const bucket = buckets.find((item) => day >= item.startKey && day <= item.endKey);
      if (bucket) {
        bucket.total += movement.amount;
        bucket.profit += movement.gross_profit ?? 0;
      }
    });
    const max = Math.max(1, ...buckets.flatMap((day) => [day.total, day.profit]));
    return buckets.map((day) => ({
      ...day,
      percent: Math.round((day.total / max) * 100),
      profitPercent: Math.round((day.profit / max) * 100),
    }));
  }, [activePreset, fromDate, saleMovements, toDate]);

  const paymentSummary = [
    { label: "Efectivo", value: saleMovements.reduce((sum, movement) => sum + (movement.cash_paid ?? 0), 0), className: "cash" },
    { label: "Tarjeta", value: saleMovements.reduce((sum, movement) => sum + (movement.card_paid ?? 0), 0), className: "card" },
    { label: "Transferencia", value: saleMovements.reduce((sum, movement) => sum + (movement.transfer_paid ?? 0), 0), className: "credit" },
  ];
  const paymentTotal = Math.max(1, paymentSummary.reduce((sum, item) => sum + item.value, 0));

  const visibleMovements = activeTab === "today"
    ? filteredMovements.slice(0, 12)
    : activeTab === "cuts"
      ? cutMovements
      : filteredMovements;
  const visibleMonthly = useMemo(() => monthly.filter((row) => {
    const month = row.month;
    if (fromDate && month < fromDate.slice(0, 7)) return false;
    if (toDate && month > toDate.slice(0, 7)) return false;
    return true;
  }), [fromDate, monthly, toDate]);

  const exportReport = () => {
    downloadCsv(`reporte-rim-pos-${new Date().toISOString().slice(0, 10)}.csv`, [
      ["fecha", "tipo", "movimiento", "detalle", "caja", "usuario", "importe", "ganancia", "impuestos", "terminal"],
      ...filteredMovements.map((movement) => [
        movement.created_at,
        movement.kind,
        movement.title,
        movement.detail,
        movement.cash_session_id ?? "",
        movement.actor_name ?? "Sistema",
        movement.amount,
        movement.gross_profit ?? 0,
        movement.tax_total ?? 0,
        movement.card_terminal ?? "",
      ]),
    ]);
    showToast("Reporte exportado");
  };

  return (
    <section className="admin-panel compact report-module">
      <div className="report-hero">
        <div>
          <h2>{reportTitle}</h2>
          <p>Ventas, ganancia, pagos, departamentos, impuestos y terminales.</p>
        </div>
        <button className="primary-button" type="button" onClick={exportReport}>
          Exportar CSV
        </button>
      </div>

      <div className="report-stat-grid">
        <ReportStat icon={CircleDollarSign} label="Ventas Totales" value={money(salesTotal)} tone="success" />
        <ReportStat icon={FileText} label="Numero de Ventas" value={String(ticketCount)} />
        <ReportStat icon={ReceiptText} label="Venta Promedio" value={money(averageTicket)} />
        <ReportStat icon={TrendingUp} label="Ganancia" value={money(grossProfit)} tone={grossProfit >= 0 ? "info" : "danger"} />
        <ReportStat icon={Percent} label="Margen promedio" value={`${marginPercent.toFixed(2)}%`} />
        <ReportStat icon={TrendingDown} label="Gastos periodo" value={money(expensesTotal)} tone="danger" />
        <ReportStat icon={Banknote} label="Efectivo" value={money(paymentSummary[0].value)} tone="warning" />
        <ReportStat icon={CreditCardIcon} label="Tarjeta" value={money(paymentSummary[1].value)} />
      </div>

      <div className="report-tabs" role="tablist" aria-label="Secciones de reportes">
        {tabs.map((tab) => (
          <button
            className={activeTab === tab.key ? "active" : undefined}
            type="button"
            role="tab"
            aria-selected={activeTab === tab.key}
            key={tab.key}
            onClick={() => setActiveTab(tab.key)}
          >
            {tab.label}
          </button>
        ))}
      </div>

      <div className="report-filter-bar refined">
        <div className="report-presets" aria-label="Rango rapido">
          {presetLabels.map((preset) => (
            <button className={activePreset === preset.key ? "active" : undefined} type="button" key={preset.key} onClick={() => applyPreset(preset.key)}>
              {preset.label}
            </button>
          ))}
          <button className={activePreset === null ? "active" : undefined} type="button" onClick={() => setActivePreset(null)}>
            Periodo...
          </button>
        </div>
        <label>
          Mes
          <input type="month" value={selectedMonth} onChange={(event) => applyMonth(event.target.value)} />
        </label>
        <label>
          Desde
          <input type="date" value={fromDate} onChange={(event) => {
            setActivePreset(null);
            setFromDate(event.target.value);
          }} />
        </label>
        <label>
          Hasta
          <input type="date" value={toDate} onChange={(event) => {
            setActivePreset(null);
            setToDate(event.target.value);
          }} />
        </label>
        <label>
          Tipo
          <select value={filterKind} onChange={(event) => setFilterKind(event.target.value as "all" | ReportMovement["kind"])}>
            <option value="all">Todo</option>
            <option value="sale">Ventas</option>
            <option value="purchase">Compras</option>
            <option value="cash">Caja</option>
            <option value="inventory">Inventario</option>
            <option value="credit">Credito clientes</option>
            <option value="cut">Cortes</option>
          </select>
        </label>
        <label className="report-search">
          Buscar
          <div>
            <Search size={16} />
            <input value={filterText} onChange={(event) => setFilterText(event.target.value)} placeholder="Folio, usuario, producto, motivo" />
          </div>
        </label>
        <div className="report-filter-total">
          <span>{filteredMovements.length} movimientos</span>
          <strong>{money(filteredTotal)}</strong>
        </div>
      </div>

      <div className="report-layout upgraded">
        <div className="report-main">
          {activeTab === "today" && (
            <div className="report-dashboard">
              <div className="report-panel">
                <div className="report-panel-title">
                  <h3>Ventas y ganancia</h3>
                  <span>{periodLabel}</span>
                </div>
                <div className="chart-legend">
                  <span><i className="legend-sales" />Ventas</span>
                  <span><i className="legend-profit" />Ganancia</span>
                </div>
                <div
                  className="sales-bars"
                  aria-label="Ventas del periodo"
                  style={{ gridTemplateColumns: `repeat(${dailySales.length}, minmax(92px, 1fr))` }}
                >
                  {dailySales.map((day) => (
                    <div className="sales-bar" key={day.key}>
                      <strong>{money(day.total)}</strong>
                      <div>
                        <span className="sales-bar-total" style={{ height: `${Math.max(4, day.percent)}%` }} />
                        <span className="sales-bar-profit" style={{ height: `${Math.max(3, day.profitPercent)}%` }} />
                      </div>
                      <small>{day.label}</small>
                    </div>
                  ))}
                </div>
              </div>
              <div className="report-panel payment-report-panel">
                <div className="report-panel-title">
                  <h3>Metodo de pago</h3>
                  <span>{periodLabel}</span>
                </div>
                <div className="payment-mix">
                  {paymentSummary.map((item) => (
                    <div className="payment-row" key={item.label}>
                      <div>
                        <span>{item.label}</span>
                        <strong>{money(item.value)}</strong>
                      </div>
                      <div className="payment-track">
                        <span className={item.className} style={{ width: `${Math.max(2, Math.round((item.value / paymentTotal) * 100))}%` }} />
                      </div>
                    </div>
                  ))}
                </div>
              </div>
              <div className="report-panel report-list-panel">
                <div className="report-panel-title">
                  <h3>Ventas por dia</h3>
                  <span>{periodLabel}</span>
                </div>
                {dailySales.length === 0 ? (
                  <div className="table-empty">Sin ventas diarias</div>
                ) : dailySales.map((day) => (
                  <div className="report-breakdown-row" key={`day-${day.key}`}>
                    <div>
                      <strong>{day.label}</strong>
                      <span>Ganancia {money(day.profit)}</span>
                    </div>
                    <strong>{money(day.total)}</strong>
                  </div>
                ))}
              </div>
              <div className="report-panel report-list-panel">
                <div className="report-panel-title">
                  <h3>Ventas por Departamento</h3>
                  <span>{money(salesTotal)}</span>
                </div>
                {departmentSales.length === 0 ? (
                  <div className="table-empty">Sin ventas por departamento</div>
                ) : departmentSales.slice(0, 8).map((department) => (
                  <div className="report-breakdown-row" key={department.department}>
                    <div>
                      <strong>{department.department}</strong>
                      <span>{department.quantity.toFixed(3)} piezas · ganancia {money(department.profit)}</span>
                    </div>
                    <strong>{money(department.total)}</strong>
                  </div>
                ))}
              </div>
              <div className="report-panel report-list-panel">
                <div className="report-panel-title">
                  <h3>Terminales usadas</h3>
                  <span>Tarjeta</span>
                </div>
                {terminalSales.length === 0 ? (
                  <div className="table-empty">Sin pagos con terminal</div>
                ) : terminalSales.map((terminal) => (
                  <div className="report-breakdown-row" key={terminal.terminal}>
                    <div>
                      <strong>{terminal.terminal}</strong>
                      <span>Pagos con tarjeta</span>
                    </div>
                    <strong>{money(terminal.total)}</strong>
                  </div>
                ))}
              </div>
              <div className="report-panel report-list-panel">
                <div className="report-panel-title">
                  <h3>Impuestos</h3>
                  <span>{periodLabel}</span>
                </div>
                <div className="tax-summary-grid">
                  <div>
                    <span>Impuestos cobrados</span>
                    <strong>{money(taxTotal)}</strong>
                  </div>
                  <div>
                    <span>Ventas gravadas</span>
                    <strong>{money(Math.max(0, salesTotal - taxTotal))}</strong>
                  </div>
                </div>
                <div className="tax-breakdown-list">
                  {taxBreakdown.length === 0 ? (
                    <div className="muted-note">Sin desglose fiscal</div>
                  ) : taxBreakdown.map((tax) => (
                    <div className="report-breakdown-row" key={tax.tax_rate}>
                      <div>
                        <strong>{tax.tax_rate > 0 ? `IVA/IEPS ${(tax.tax_rate * 100).toFixed(2)}%` : "Tasa 0%"}</strong>
                        <span>Ventas gravadas {money(tax.taxable_sales)}</span>
                      </div>
                      <strong>{money(tax.tax_collected)}</strong>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          )}

          {activeTab === "products" ? (
            <div className="report-panel product-report-panel">
              <div className="report-panel-title">
                <h3>Productos vendidos</h3>
                <span>Ordenados por venta</span>
              </div>
              {products.length === 0 ? (
                <div className="table-empty">Sin ventas por producto</div>
              ) : products.map((product) => (
                <div className="product-report-row" key={product.product_id}>
                  <div>
                    <strong>{product.product_name}</strong>
                    <span>{product.category || "- Sin Departamento -"} · {product.quantity.toFixed(3)} piezas · ganancia {money(product.gross_profit ?? 0)}</span>
                  </div>
                  <strong>{money(product.total)}</strong>
                </div>
              ))}
              <div className="report-panel-title">
                <h3>Productos sin venta</h3>
                <span>En este periodo</span>
              </div>
              {unsoldProducts.length === 0 ? (
                <div className="table-empty">Todos tuvieron venta en el periodo</div>
              ) : unsoldProducts.map((product) => (
                <div className="product-report-row" key={`unsold-${product.product_id}`}>
                  <div>
                    <strong>{product.product_name}</strong>
                    <span>{product.category || "- Sin Departamento -"}</span>
                  </div>
                  <strong>{money(0)}</strong>
                </div>
              ))}
            </div>
          ) : activeTab === "monthly" ? (
            <div className="report-panel product-report-panel">
              <div className="report-panel-title">
                <h3>Reporte mensual</h3>
                <span>Cierres Z por mes</span>
              </div>
              {visibleMonthly.length === 0 ? (
                <div className="table-empty">Sin cortes Z cerrados</div>
              ) : visibleMonthly.map((row) => (
                <div className="product-report-row" key={row.month}>
                  <div>
                    <strong>{row.month}</strong>
                    <span>{row.total_tickets} tickets, {row.canceled_tickets} cancelados</span>
                  </div>
                  <div className="monthly-values">
                    <strong>{money(row.total_amount)}</strong>
                    <span>prom. {money(row.average_ticket)}</span>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="report-panel movement-panel">
              <div className="report-panel-title">
                <h3>{activeTab === "cuts" ? "Cortes de caja" : activeTab === "today" ? "Movimientos del periodo" : "Movimientos"}</h3>
                <span>{periodLabel}</span>
              </div>
              {visibleMovements.length === 0 ? (
                <div className="table-empty">Sin movimientos registrados</div>
              ) : visibleMovements.map((movement) => {
                const Icon = movementIcon(movement.kind);
                return (
                  <div className={`movement-card ${movementTone(movement)}`} key={movement.id}>
                    <Icon size={18} />
                    <div>
                      <strong>{movement.title}</strong>
                      <span>{movement.detail}</span>
                      <small>
                        {formatDateTimeMx(movement.created_at)}
                        {" · "}
                        {movement.cash_session_id ? `Caja ${movement.cash_session_id}` : "General"}
                        {" · "}
                        {movement.actor_name || "Sistema"}
                        {movement.card_terminal ? ` · Terminal ${movement.card_terminal}` : ""}
                      </small>
                    </div>
                    <span className="movement-kind">{movementLabel(movement.kind)}</span>
                    <strong className={movement.amount < 0 ? "amount-out" : "amount-in"}>{money(movement.amount)}</strong>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        <aside className="report-side">
          <div className="report-side-card net-card">
            <span>Periodo filtrado</span>
            <strong>{money(netTotal)}</strong>
            <small>Ventas {money(salesTotal)} · ganancia {money(grossProfit)} · impuestos {money(taxTotal)}</small>
          </div>

          <div className="report-side-card">
            <h3>Departamentos</h3>
            {departmentSales.length === 0 ? (
              <div className="muted-note">Sin departamentos en rango</div>
            ) : departmentSales.slice(0, 5).map((department) => (
              <div className="side-mini-row" key={department.department}>
                <span>{department.department}</span>
                <strong>{money(department.total)}</strong>
              </div>
            ))}
          </div>

          <div className="report-side-card">
            <h3>Ultimos gastos</h3>
            {expenseMovements.length === 0 ? (
              <div className="muted-note">Sin gastos en rango</div>
            ) : expenseMovements.slice(0, 5).map((movement) => (
              <div className="side-mini-row" key={movement.id}>
                <span>{movement.title}</span>
                <strong>{money(Math.abs(movement.amount))}</strong>
              </div>
            ))}
          </div>

          <div className="report-side-card">
            <h3>Top productos</h3>
            {products.length === 0 ? (
              <div className="muted-note">Sin productos vendidos</div>
            ) : products.slice(0, 5).map((product) => (
              <div className="side-mini-row" key={product.product_id}>
                <span>{product.product_name}</span>
                <strong>{money(product.total)}</strong>
              </div>
            ))}
          </div>

          <div className="report-side-card">
            <h3>Terminales</h3>
            {terminalSales.length === 0 ? (
              <div className="muted-note">Sin pagos con tarjeta</div>
            ) : terminalSales.slice(0, 5).map((terminal) => (
              <div className="side-mini-row" key={terminal.terminal}>
                <span>{terminal.terminal}</span>
                <strong>{money(terminal.total)}</strong>
              </div>
            ))}
          </div>

          <div className="report-side-card">
            <h3>Cortes recientes</h3>
            {cutMovements.length === 0 ? (
              <div className="muted-note">Sin cortes en rango</div>
            ) : cutMovements.slice(0, 4).map((movement) => (
              <div className="side-mini-row" key={movement.id}>
                <span>{movement.title}</span>
                <strong>{money(movement.amount)}</strong>
              </div>
            ))}
          </div>
        </aside>
      </div>
    </section>
  );
}
