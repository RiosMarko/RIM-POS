import { Archive, Percent, Printer, Scale } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Setting } from "../../components/display/SummaryCards";
import { selectNumericInput } from "../../lib/numberInput";
import { getSetting, listHardwareDevices, openDrawer, printTicket, readScale, setSetting } from "../../lib/posApi";
import type { HardwareDevice } from "../../types";

export function SettingsView({
  showToast,
  onTaxModeChange,
}: {
  showToast: (message: string) => void;
  onTaxModeChange: (mode: { enabled: boolean; pricesIncludeTax: boolean }) => void;
}) {
  const [printer, setPrinter] = useState("mock-printer-80mm");
  const [scale, setScale] = useState("mock-scale-serial");
  const [scaleBaudRate, setScaleBaudRate] = useState(9600);
  const [drawer, setDrawer] = useState("mock-drawer-escpos");
  const [workstationId, setWorkstationId] = useState("CAJA-1");
  const [taxEnabled, setTaxEnabled] = useState(true);
  const [taxCountry, setTaxCountry] = useState("MX");
  const [taxDefaultRate, setTaxDefaultRate] = useState(0.16);
  const [taxPricesIncludeTax, setTaxPricesIncludeTax] = useState(true);
  const [taxShowBreakdown, setTaxShowBreakdown] = useState(true);
  const [taxAutoApply, setTaxAutoApply] = useState(true);
  const [ticketStoreName, setTicketStoreName] = useState("RIM-POS");
  const [ticketHeader, setTicketHeader] = useState("Abarrotes y miscelanea");
  const [ticketFooter, setTicketFooter] = useState("Gracias por su compra");
  const [ticketWidth, setTicketWidth] = useState(32);
  const [ticketShowLogo, setTicketShowLogo] = useState(true);
  const [ticketShowDate, setTicketShowDate] = useState(true);
  const [ticketShowCashier, setTicketShowCashier] = useState(true);
  const [ticketShowBarcode, setTicketShowBarcode] = useState(false);
  const [ticketShowItemCount, setTicketShowItemCount] = useState(true);
  const [ticketStartLines, setTicketStartLines] = useState(0);
  const [ticketExtraLines, setTicketExtraLines] = useState(3);
  const [ticketCopies, setTicketCopies] = useState(1);
  const [ticketPreviewDraft, setTicketPreviewDraft] = useState("");
  const [ticketPreviewDirty, setTicketPreviewDirty] = useState(false);
  const [devices, setDevices] = useState<HardwareDevice[]>([]);
  const [detecting, setDetecting] = useState(false);

  const printers = useMemo(() => devices.filter((device) => device.device_type === "printer"), [devices]);
  const serialDevices = useMemo(() => devices.filter((device) => device.device_type === "serial"), [devices]);

  const detectDevices = useCallback(async () => {
    setDetecting(true);
    try {
      const result = await listHardwareDevices();
      setDevices(result);
      const defaultPrinter = result.find((device) => device.device_type === "printer" && device.is_default) ?? result.find((device) => device.device_type === "printer");
      const defaultSerial = result.find((device) => device.device_type === "serial");
      if (defaultPrinter && printer === "mock-printer-80mm") setPrinter(defaultPrinter.id);
      if (defaultSerial && scale === "mock-scale-serial") setScale(defaultSerial.id);
      if (defaultSerial && drawer === "mock-drawer-escpos") setDrawer(defaultSerial.id);
      showToast(`${result.length} dispositivos detectados`);
    } catch (error) {
      showToast(String(error));
    } finally {
      setDetecting(false);
    }
  }, [drawer, printer, scale, showToast]);

  useEffect(() => {
    Promise.all([
      getSetting("printer"),
      getSetting("workstation_id"),
      getSetting("scale"),
      getSetting("scale_baud_rate"),
      getSetting("drawer"),
      getSetting("tax_enabled"),
      getSetting("tax_country"),
      getSetting("tax_default_rate"),
      getSetting("tax_prices_include_tax"),
      getSetting("tax_show_breakdown"),
      getSetting("tax_auto_apply_new_products"),
      getSetting("ticket_store_name"),
      getSetting("ticket_header"),
      getSetting("ticket_footer"),
      getSetting("ticket_width"),
      getSetting("ticket_show_logo"),
      getSetting("ticket_show_date"),
      getSetting("ticket_show_cashier"),
      getSetting("ticket_show_barcode"),
      getSetting("ticket_show_item_count"),
      getSetting("ticket_start_lines"),
      getSetting("ticket_extra_lines"),
      getSetting("ticket_copies"),
      listHardwareDevices(),
    ])
      .then(([
        nextPrinter,
        nextWorkstationId,
        nextScale,
        nextScaleBaudRate,
        nextDrawer,
        nextTaxEnabled,
        nextTaxCountry,
        nextTaxDefaultRate,
        nextTaxPricesIncludeTax,
        nextTaxShowBreakdown,
        nextTaxAutoApply,
        nextTicketStoreName,
        nextTicketHeader,
        nextTicketFooter,
        nextTicketWidth,
        nextTicketShowLogo,
        nextTicketShowDate,
        nextTicketShowCashier,
        nextTicketShowBarcode,
        nextTicketShowItemCount,
        nextTicketStartLines,
        nextTicketExtraLines,
        nextTicketCopies,
        nextDevices,
      ]) => {
        setDevices(nextDevices);
        const defaultPrinter = nextDevices.find((device) => device.device_type === "printer" && device.is_default) ?? nextDevices.find((device) => device.device_type === "printer");
        const defaultSerial = nextDevices.find((device) => device.device_type === "serial");
        setPrinter(nextPrinter || defaultPrinter?.id || "mock-printer-80mm");
        setWorkstationId(nextWorkstationId || "CAJA-1");
        setScale(nextScale || defaultSerial?.id || "mock-scale-serial");
        setScaleBaudRate(Number(nextScaleBaudRate ?? 9600));
        setDrawer(nextDrawer || defaultPrinter?.id || defaultSerial?.id || "mock-drawer-escpos");
        const enabled = nextTaxEnabled !== "false";
        setTaxEnabled(enabled);
        setTaxCountry(nextTaxCountry || "MX");
        setTaxDefaultRate(Number(nextTaxDefaultRate ?? 0.16));
        setTaxPricesIncludeTax(nextTaxPricesIncludeTax !== "false");
        setTaxShowBreakdown(nextTaxShowBreakdown !== "false");
        setTaxAutoApply(nextTaxAutoApply !== "false");
        setTicketStoreName(nextTicketStoreName || "RIM-POS");
        setTicketHeader(nextTicketHeader || "Abarrotes y miscelanea");
        setTicketFooter(nextTicketFooter || "Gracias por su compra");
        setTicketWidth(Number(nextTicketWidth ?? 32));
        setTicketShowLogo(nextTicketShowLogo !== "false");
        setTicketShowDate(nextTicketShowDate !== "false");
        setTicketShowCashier(nextTicketShowCashier !== "false");
        setTicketShowBarcode(nextTicketShowBarcode === "true");
        setTicketShowItemCount(nextTicketShowItemCount !== "false");
        setTicketStartLines(Number(nextTicketStartLines ?? 0));
        setTicketExtraLines(Number(nextTicketExtraLines ?? 3));
        setTicketCopies(Number(nextTicketCopies ?? 1));
        onTaxModeChange({ enabled, pricesIncludeTax: nextTaxPricesIncludeTax !== "false" });
      })
      .catch((error) => showToast(String(error)));
  }, [onTaxModeChange, showToast]);

  const clampTicketLines = (value: number) => Math.max(0, Math.min(8, Math.floor(value)));

  const deriveTicketPreviewSettings = (value: string) => {
    const lines = value.replace(/\r/g, "").split("\n");
    let first = 0;
    while (first < lines.length && lines[first].trim() === "") first += 1;
    let last = lines.length - 1;
    while (last >= first && lines[last].trim() === "") last -= 1;
    const startLines = clampTicketLines(first);
    const extraLines = clampTicketLines(lines.length - 1 - last);
    const folioIndex = lines.findIndex((lineText) => lineText.startsWith("Folio 2026-06-001"));
    const headerStart = startLines + (ticketShowLogo ? 1 : 0);
    const nextHeader = folioIndex > headerStart ? lines.slice(headerStart, folioIndex).join("\n") : ticketHeader;
    let itemCountIndex = -1;
    let changeIndex = -1;
    lines.forEach((lineText, index) => {
      if (lineText.startsWith("Articulos:")) itemCountIndex = index;
      if (lineText.startsWith("CAMBIO")) changeIndex = index;
    });
    const footerMarker = itemCountIndex >= 0 ? itemCountIndex : changeIndex;
    let footerLines = footerMarker >= 0 ? lines.slice(footerMarker + 1, last + 1) : [];
    if (footerLines[0]?.trim() === "") footerLines = footerLines.slice(1);
    return {
      header: nextHeader,
      footer: footerLines.length ? footerLines.join("\n") : ticketFooter,
      startLines,
      extraLines,
    };
  };

  const applyTicketPreviewEdits = (value = ticketPreviewDraft) => {
    const next = deriveTicketPreviewSettings(value);
    setTicketHeader(next.header);
    setTicketFooter(next.footer);
    setTicketStartLines(next.startLines);
    setTicketExtraLines(next.extraLines);
    setTicketPreviewDirty(false);
  };

  const save = async () => {
    const previewSettings = ticketPreviewDirty ? deriveTicketPreviewSettings(ticketPreviewDraft) : null;
    try {
      await setSetting("printer", printer);
      await setSetting("workstation_id", workstationId.trim() || "CAJA-1");
      await setSetting("scale", scale);
      await setSetting("scale_baud_rate", String(scaleBaudRate));
      await setSetting("drawer", drawer);
      await setSetting("tax_enabled", String(taxEnabled));
      await setSetting("tax_country", taxCountry);
      await setSetting("tax_default_rate", String(taxDefaultRate));
      await setSetting("tax_prices_include_tax", String(taxPricesIncludeTax));
      await setSetting("tax_show_breakdown", String(taxShowBreakdown));
      await setSetting("tax_auto_apply_new_products", String(taxAutoApply));
      await setSetting("ticket_store_name", ticketStoreName);
      await setSetting("ticket_header", previewSettings?.header ?? ticketHeader);
      await setSetting("ticket_footer", previewSettings?.footer ?? ticketFooter);
      await setSetting("ticket_width", String(ticketWidth));
      await setSetting("ticket_show_logo", String(ticketShowLogo));
      await setSetting("ticket_show_date", String(ticketShowDate));
      await setSetting("ticket_show_cashier", String(ticketShowCashier));
      await setSetting("ticket_show_barcode", String(ticketShowBarcode));
      await setSetting("ticket_show_item_count", String(ticketShowItemCount));
      await setSetting("ticket_start_lines", String(previewSettings?.startLines ?? ticketStartLines));
      await setSetting("ticket_extra_lines", String(previewSettings?.extraLines ?? ticketExtraLines));
      await setSetting("ticket_copies", String(ticketCopies));
      if (previewSettings) {
        setTicketHeader(previewSettings.header);
        setTicketFooter(previewSettings.footer);
        setTicketStartLines(previewSettings.startLines);
        setTicketExtraLines(previewSettings.extraLines);
        setTicketPreviewDirty(false);
      }
      onTaxModeChange({ enabled: taxEnabled, pricesIncludeTax: taxPricesIncludeTax });
      showToast("Configuracion guardada");
    } catch (error) {
      showToast(String(error));
    }
  };

  const deviceLabel = (device: HardwareDevice) => `${device.name}${device.is_default ? " (predeterminado)" : ""}`;
  const selectedDeviceName = (id: string) => devices.find((device) => device.id === id)?.name ?? id;
  const line = "-".repeat(Math.max(24, Math.min(48, ticketWidth)));
  const ticketPreview = useMemo(() => {
    const width = Math.max(24, Math.min(48, ticketWidth));
    const separator = "-".repeat(width);
    const lines = [
      ...Array.from({ length: Math.max(0, Math.min(8, ticketStartLines)) }, () => ""),
      ...(ticketShowLogo ? [ticketStoreName || "RIM-POS"] : []),
      ...(ticketHeader.trim() ? ticketHeader.split("\n") : []),
      "Folio 2026-06-001",
      ...(ticketShowDate ? ["2026-06-20 08:20"] : []),
      ...(ticketShowCashier ? ["Cajero: Admin"] : []),
      separator,
      "Refresco cola 600 ml",
      ...(ticketShowBarcode ? ["  750000000001"] : []),
      "  2.000 @ $18.00  $36.00",
      "Arroz 1 kg",
      ...(ticketShowBarcode ? ["  750000000002"] : []),
      "  1.000 @ $32.00  $32.00",
      separator,
      ...(taxShowBreakdown ? ["SUBTOTAL        $58.62", "IMPUESTOS       $9.38"] : []),
      "*** TOTAL       $68.00",
      "PAGADO          $100.00",
      "CAMBIO          $32.00",
      ...(ticketShowItemCount ? ["Articulos: 3.000"] : []),
      "",
      ...(ticketFooter.trim() ? ticketFooter.split("\n") : []),
      ...Array.from({ length: Math.max(0, Math.min(8, ticketExtraLines)) }, () => ""),
    ];
    return lines.join("\n");
  }, [
    taxShowBreakdown,
    ticketExtraLines,
    ticketFooter,
    ticketHeader,
    ticketShowBarcode,
    ticketShowCashier,
    ticketShowDate,
    ticketShowItemCount,
    ticketShowLogo,
    ticketStartLines,
    ticketStoreName,
    ticketWidth,
  ]);

  useEffect(() => {
    if (!ticketPreviewDirty) setTicketPreviewDraft(ticketPreview);
  }, [ticketPreview, ticketPreviewDirty]);

  return (
    <section className="admin-panel settings-module">
      <div className="settings-layout">
        <div className="settings-main">
          <section className="settings-section">
            <div className="settings-section-title">
              <div>
                <h2>Ticket impreso</h2>
                <p>Texto, contenido visible, ancho y corte.</p>
              </div>
              <button className="primary-button" type="button" onClick={save}>Guardar configuracion</button>
            </div>
            <div className="ticket-layout">
              <div className="ticket-control-grid">
                <label className="field-span-2">
                  Nombre del negocio
                  <input value={ticketStoreName} onChange={(event) => setTicketStoreName(event.target.value)} />
                </label>
                <label>
                  Ancho
                  <select value={ticketWidth} onChange={(event) => setTicketWidth(Number(event.target.value))}>
                    <option value={32}>58 mm / 32 chars</option>
                    <option value={40}>80 mm / 40 chars</option>
                    <option value={48}>80 mm amplio</option>
                  </select>
                </label>
                <label>
                  Copias
                  <input type="number" min="1" max="4" value={ticketCopies} onFocus={selectNumericInput} onChange={(event) => setTicketCopies(Number(event.target.value))} />
                </label>
                <label className="field-span-2">
                  Encabezado
                  <textarea value={ticketHeader} onChange={(event) => setTicketHeader(event.target.value)} rows={3} />
                </label>
                <label className="field-span-2">
                  Pie de ticket
                  <textarea value={ticketFooter} onChange={(event) => setTicketFooter(event.target.value)} rows={3} />
                </label>
                <label>
                  Lineas iniciales
                  <input type="number" min="0" max="8" value={ticketStartLines === 0 ? "" : ticketStartLines} onFocus={selectNumericInput} onChange={(event) => setTicketStartLines(Number(event.target.value))} />
                </label>
                <label>
                  Lineas al final
                  <input type="number" min="0" max="8" value={ticketExtraLines === 0 ? "" : ticketExtraLines} onFocus={selectNumericInput} onChange={(event) => setTicketExtraLines(Number(event.target.value))} />
                </label>
                <div className="toggle-stack field-span-2">
                  <label className="toggle-row">
                    <input type="checkbox" checked={ticketShowLogo} onChange={(event) => setTicketShowLogo(event.target.checked)} />
                    Mostrar nombre arriba
                  </label>
                  <label className="toggle-row">
                    <input type="checkbox" checked={ticketShowDate} onChange={(event) => setTicketShowDate(event.target.checked)} />
                    Mostrar fecha y hora
                  </label>
                  <label className="toggle-row">
                    <input type="checkbox" checked={ticketShowCashier} onChange={(event) => setTicketShowCashier(event.target.checked)} />
                    Mostrar cajero
                  </label>
                  <label className="toggle-row">
                    <input type="checkbox" checked={ticketShowBarcode} onChange={(event) => setTicketShowBarcode(event.target.checked)} />
                    Mostrar codigo por producto
                  </label>
                  <label className="toggle-row">
                    <input type="checkbox" checked={ticketShowItemCount} onChange={(event) => setTicketShowItemCount(event.target.checked)} />
                    Mostrar total de articulos
                  </label>
                  <label className="toggle-row">
                    <input type="checkbox" checked={taxShowBreakdown} onChange={(event) => setTaxShowBreakdown(event.target.checked)} disabled={!taxEnabled} />
                    Desglosar impuestos
                  </label>
                </div>
              </div>
              <aside className="ticket-preview-card" aria-label="Preview del ticket">
                <div className="ticket-preview-title">
                  <strong>Preview del ticket</strong>
                  <span>{line.length} chars</span>
                </div>
                <textarea
                  className="ticket-preview"
                  value={ticketPreviewDraft}
                  rows={18}
                  onChange={(event) => {
                    setTicketPreviewDraft(event.target.value);
                    setTicketPreviewDirty(true);
                  }}
                  onBlur={() => {
                    if (ticketPreviewDirty) applyTicketPreviewEdits();
                  }}
                  aria-label="Editar preview del ticket"
                />
                <button className="ghost-button" type="button" onClick={() => printTicket(0).then((result) => showToast(result.message)).catch((error) => showToast(String(error)))}>
                  Probar impresora
                </button>
              </aside>
            </div>
          </section>

          <section className="settings-section">
            <div className="settings-section-title">
              <div>
                <h2>Impuestos</h2>
                <p>IVA, IEPS y captura de precios finales.</p>
              </div>
            </div>
            <div className="settings-grid">
              <label>
                Impuestos
                <select value={taxEnabled ? "true" : "false"} onChange={(event) => setTaxEnabled(event.target.value === "true")}>
                  <option value="true">Mis productos manejan impuestos</option>
                  <option value="false">Sin impuestos</option>
                </select>
              </label>
              <label>
                Pais fiscal
                <select value={taxCountry} onChange={(event) => setTaxCountry(event.target.value)}>
                  <option value="MX">Mexico SAT</option>
                  <option value="OTHER">Otro pais</option>
                </select>
              </label>
              <label>
                Impuesto default
                <select value={taxDefaultRate} onChange={(event) => setTaxDefaultRate(Number(event.target.value))} disabled={!taxEnabled}>
                  <option value={0}>0%</option>
                  <option value={0.08}>8%</option>
                  <option value={0.16}>IVA 16%</option>
                  <option value={0.265}>IEPS 26.5%</option>
                </select>
              </label>
              <label>
                Nuevos productos
                <select value={taxAutoApply ? "true" : "false"} onChange={(event) => setTaxAutoApply(event.target.value === "true")} disabled={!taxEnabled}>
                  <option value="true">Aplicar impuesto default</option>
                  <option value="false">Asignar manualmente</option>
                </select>
              </label>
              <label>
                Precios
                <select value={taxPricesIncludeTax ? "included" : "added"} onChange={(event) => setTaxPricesIncludeTax(event.target.value === "included")} disabled={!taxEnabled}>
                  <option value="included">Precio ya incluye impuestos</option>
                  <option value="added">Sumar impuestos al cobrar</option>
                </select>
              </label>
            </div>
          </section>

          <section className="settings-section">
            <div className="settings-section-title">
              <div>
                <h2>Hardware</h2>
                <p>Impresora, bascula y cajon.</p>
              </div>
            </div>
            <div className="settings-grid">
              <label>
                Nombre de caja
                <input value={workstationId} onChange={(event) => setWorkstationId(event.target.value)} placeholder="CAJA-1" />
              </label>
              <label>
                Impresora ticket
                <select value={printer} onChange={(event) => setPrinter(event.target.value)}>
                  <option value="">Seleccionar impresora</option>
                  {printers.map((device) => (
                    <option value={device.id} key={device.id}>{deviceLabel(device)}</option>
                  ))}
                </select>
              </label>
              <label>
                Bascula
                <select value={scale} onChange={(event) => setScale(event.target.value)}>
                  <option value="">Seleccionar puerto</option>
                  {serialDevices.map((device) => (
                    <option value={device.id} key={device.id}>{deviceLabel(device)}</option>
                  ))}
                </select>
              </label>
              <label>
                Baud bascula
                <select value={scaleBaudRate} onChange={(event) => setScaleBaudRate(Number(event.target.value))}>
                  <option value={2400}>2400</option>
                  <option value={4800}>4800</option>
                  <option value={9600}>9600</option>
                  <option value={19200}>19200</option>
                  <option value={38400}>38400</option>
                </select>
              </label>
              <label>
                Cajon
                <select value={drawer} onChange={(event) => setDrawer(event.target.value)}>
                  <option value="">Seleccionar dispositivo</option>
                  {printers.map((device) => (
                    <option value={device.id} key={`printer-${device.id}`}>Pulso por {device.name}</option>
                  ))}
                  {serialDevices.map((device) => (
                    <option value={device.id} key={`serial-${device.id}`}>{deviceLabel(device)}</option>
                  ))}
                </select>
              </label>
            </div>
            <div className="cash-actions">
              <button className="ghost-button" type="button" onClick={detectDevices} disabled={detecting}>
                {detecting ? "Detectando" : "Detectar dispositivos"}
              </button>
              <button className="ghost-button" type="button" onClick={() => readScale().then((result) => showToast(`${result.weight} ${result.unit}`)).catch((error) => showToast(String(error)))}>Probar bascula</button>
              <button className="ghost-button" type="button" onClick={() => openDrawer().then((result) => showToast(result.message)).catch((error) => showToast(String(error)))}>Probar cajon</button>
            </div>
          </section>

        </div>

        <aside className="settings-side">
          <div className="settings-list">
            <Setting icon={Archive} label="Caja/estacion" value={workstationId || "CAJA-1"} />
            <Setting icon={Printer} label="Impresora ticket" value={selectedDeviceName(printer)} />
            <Setting icon={Scale} label="Bascula" value={selectedDeviceName(scale)} />
            <Setting icon={Archive} label="Cajon" value={selectedDeviceName(drawer)} />
            <Setting icon={Percent} label="Impuestos" value={taxEnabled ? `${Math.round(taxDefaultRate * 100)}%, ${taxPricesIncludeTax ? "incluidos" : "sumados"}` : "Desactivados"} />
          </div>
          <div className="device-list">
            {devices.map((device) => (
              <div className="device-row" key={`${device.device_type}-${device.id}`}>
                <strong>{device.name}</strong>
                <span>{device.device_type} · {device.connection}</span>
                <small>{device.detail}</small>
              </div>
            ))}
          </div>
        </aside>
      </div>
    </section>
  );
}
