import {
  Archive,
  Banknote,
  CalendarDays,
  CircleDollarSign,
  CreditCard as CreditCardIcon,
  FileText,
  PackageCheck,
  Search,
  ShoppingCart,
  TrendingDown,
  TrendingUp,
  WalletCards,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { LucideIcon } from "lucide-react";
import { downloadCsv } from "../../lib/csv";
import { money } from "../../lib/money";
import { getMonthlySalesReport, getProductSalesReport, listReportMovements } from "../../lib/posApi";
import type { MonthlySalesReport, ProductSalesReport, ReportMovement } from "../../types";

type ReportTab = "today" | "movements" | "products" | "cuts" | "monthly";
type DatePreset = "today" | "week" | "month" | "all";

const tabs: Array<{ key: ReportTab; label: string }> = [
  { key: "today", label: "Hoy" },
  { key: "movements", label: "Movimientos" },
  { key: "products", label: "Productos" },
  { key: "cuts", label: "Cortes" },
  { key: "monthly", label: "Mensual" },
];

const presetLabels: Array<{ key: DatePreset; label: string }> = [
  { key: "today", label: "Hoy" },
  { key: "week", label: "Semana" },
  { key: "month", label: "Mes" },
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

function shortDateLabel(date: Date) {
  return date.toLocaleDateString("es-MX", { day: "2-digit", month: "2-digit" });
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
  const [products, setProducts] = useState<ProductSalesReport[]>([]);
  const [movements, setMovements] = useState<ReportMovement[]>([]);
  const [monthly, setMonthly] = useState<MonthlySalesReport[]>([]);
  const [activeTab, setActiveTab] = useState<ReportTab>("today");
  const [activePreset, setActivePreset] = useState<DatePreset | null>("today");
  const [filterKind, setFilterKind] = useState<"all" | ReportMovement["kind"]>("all");
  const [filterText, setFilterText] = useState("");
  const [fromDate, setFromDate] = useState(todayKey);
  const [toDate, setToDate] = useState(todayKey);

  const refresh = useCallback(async () => {
    const [nextProducts, nextMovements, nextMonthly] = await Promise.all([
      getProductSalesReport({ fromDate: fromDate || undefined, toDate: toDate || undefined }),
      listReportMovements(),
      getMonthlySalesReport(),
    ]);
    setProducts(nextProducts);
    setMovements(nextMovements);
    setMonthly(nextMonthly);
  }, [fromDate, toDate]);

  useEffect(() => {
    refresh().catch((error) => showToast(String(error)));
  }, [refresh, showToast]);

  const applyPreset = (preset: DatePreset) => {
    setActivePreset(preset);
    const now = new Date();
    if (preset === "today") {
      const today = dateKey(now);
      setFromDate(today);
      setToDate(today);
      return;
    }
    if (preset === "week") {
      const start = new Date(now);
      start.setDate(now.getDate() - 6);
      setFromDate(dateKey(start));
      setToDate(dateKey(now));
      return;
    }
    if (preset === "month") {
      const start = new Date(now.getFullYear(), now.getMonth(), 1);
      const end = endOfMonth(now);
      setFromDate(dateKey(start));
      setToDate(dateKey(end));
      return;
    }
    setFromDate("");
    setToDate("");
  };

  const periodLabel = useMemo(() => {
    if (activePreset) return presetLabels.find((preset) => preset.key === activePreset)?.label ?? "Periodo";
    if (fromDate && toDate && fromDate === toDate) return fromDate;
    if (fromDate || toDate) return `${fromDate || "Inicio"} - ${toDate || "Hoy"}`;
    return "Todo";
  }, [activePreset, fromDate, toDate]);

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
  const filteredTotal = useMemo(
    () => filteredMovements.reduce((sum, movement) => sum + movement.amount, 0),
    [filteredMovements],
  );

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
        label: bucketSize === 1
          ? bucketStart.toLocaleDateString("es-MX", { weekday: "short" })
          : `${shortDateLabel(bucketStart)}-${shortDateLabel(cappedEnd)}`,
        total: 0,
      };
    });
    saleMovements.forEach((movement) => {
      const day = movement.created_at.slice(0, 10);
      const bucket = buckets.find((item) => day >= item.startKey && day <= item.endKey);
      if (bucket) bucket.total += movement.amount;
    });
    const max = Math.max(1, ...buckets.map((day) => day.total));
    return buckets.map((day) => ({ ...day, percent: Math.round((day.total / max) * 100) }));
  }, [activePreset, fromDate, saleMovements, toDate]);

  const paymentSummary = [
    { label: "Efectivo", value: saleMovements.reduce((sum, movement) => sum + (movement.cash_paid ?? 0), 0), className: "cash" },
    { label: "Tarjeta", value: saleMovements.reduce((sum, movement) => sum + (movement.card_paid ?? 0), 0), className: "card" },
    { label: "Credito", value: saleMovements.reduce((sum, movement) => sum + (movement.transfer_paid ?? 0), 0), className: "credit" },
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
      ["fecha", "tipo", "movimiento", "detalle", "caja", "usuario", "importe"],
      ...filteredMovements.map((movement) => [
        movement.created_at,
        movement.kind,
        movement.title,
        movement.detail,
        movement.cash_session_id ?? "",
        movement.actor_name ?? "Sistema",
        movement.amount,
      ]),
    ]);
    showToast("Reporte exportado");
  };

  return (
    <section className="admin-panel compact report-module">
      <div className="report-hero">
        <div>
          <h2>Reportes</h2>
          <p>Ventas, gastos, caja y productos en una vista mas facil de leer.</p>
        </div>
        <button className="primary-button" type="button" onClick={exportReport}>
          Exportar CSV
        </button>
      </div>

      <div className="report-stat-grid">
        <ReportStat icon={CircleDollarSign} label="Vendido periodo" value={money(salesTotal)} tone="success" />
        <ReportStat icon={TrendingDown} label="Gastos periodo" value={money(expensesTotal)} tone="danger" />
        <ReportStat icon={TrendingUp} label="Neto periodo" value={money(netTotal)} tone={netTotal >= 0 ? "info" : "danger"} />
        <ReportStat icon={FileText} label="Tickets" value={String(ticketCount)} />
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
        </div>
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
                  <h3>Ventas del periodo</h3>
                  <span>{periodLabel}</span>
                </div>
                <div
                  className="sales-bars"
                  aria-label="Ventas del periodo"
                  style={{ gridTemplateColumns: `repeat(${dailySales.length}, minmax(92px, 1fr))` }}
                >
                  {dailySales.map((day) => (
                    <div className="sales-bar" key={day.key}>
                      <strong>{money(day.total)}</strong>
                      <div><span style={{ height: `${Math.max(4, day.percent)}%` }} /></div>
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
                    <span>{product.quantity} piezas</span>
                  </div>
                  <strong>{money(product.total)}</strong>
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
                        {new Date(movement.created_at).toLocaleString("es-MX", { dateStyle: "short", timeStyle: "short" })}
                        {" · "}
                        {movement.cash_session_id ? `Caja ${movement.cash_session_id}` : "General"}
                        {" · "}
                        {movement.actor_name || "Sistema"}
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
            <small>Ventas {money(salesTotal)} menos gastos {money(expensesTotal)}</small>
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
