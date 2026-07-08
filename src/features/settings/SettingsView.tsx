import { Archive, CreditCard, Percent, Printer, Scale, Trash2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Setting } from "../../components/display/SummaryCards";
import { loadCardTerminals, saveCardTerminals } from "../../lib/cardTerminals";
import { selectNumericInput } from "../../lib/numberInput";
import { getSettings, listHardwareDevices, openDrawer, printTicket, readScale, setSettings } from "../../lib/posApi";
import type { HardwareDevice } from "../../types";

export function SettingsView({
  showToast,
  onTaxModeChange,
}: {
  showToast: (message: string) => void;
  onTaxModeChange: (mode: { enabled: boolean; pricesIncludeTax: boolean; roundTotalUp?: boolean }) => void;
}) {
  const [printer, setPrinter] = useState("mock-printer-80mm");
  const [cutPrinter, setCutPrinter] = useState("");
  const [scale, setScale] = useState("mock-scale-serial");
  const [scaleBaudRate, setScaleBaudRate] = useState(9600);
  const [drawer, setDrawer] = useState("mock-drawer-escpos");
  const [workstationId, setWorkstationId] = useState("CAJA-1");
  const [taxEnabled, setTaxEnabled] = useState(true);
  const [taxCountry, setTaxCountry] = useState("MX");
  const [taxDefaultRate, setTaxDefaultRate] = useState(0.16);
  const [taxPricesIncludeTax, setTaxPricesIncludeTax] = useState(true);
  const [taxShowBreakdown, setTaxShowBreakdown] = useState(true);
  const [roundTotalUp, setRoundTotalUp] = useState(true);
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
  const [ticketEscpos, setTicketEscpos] = useState(true);
  const [ticketPreviewDraft, setTicketPreviewDraft] = useState("");
  const [ticketPreviewDirty, setTicketPreviewDirty] = useState(false);
  const [cardTerminals, setCardTerminals] = useState<string[]>([]);
  const [cardTerminalDraft, setCardTerminalDraft] = useState("");
  const [devices, setDevices] = useState<HardwareDevice[]>([]);
  const [detecting, setDetecting] = useState(false);
  const [scanNetwork, setScanNetwork] = useState(false);

  const settingsKeys = useMemo(() => [
    "printer",
    "cut_printer",
    "workstation_id",
    "scale",
    "scale_baud_rate",
    "drawer",
    "tax_enabled",
    "tax_country",
    "tax_default_rate",
    "tax_prices_include_tax",
    "tax_show_breakdown",
    "tax_auto_apply_new_products",
    "ticket_store_name",
    "ticket_header",
    "ticket_footer",
    "ticket_width",
    "ticket_show_logo",
    "ticket_show_date",
    "ticket_show_cashier",
    "ticket_show_barcode",
    "ticket_show_item_count",
    "ticket_start_lines",
    "ticket_extra_lines",
    "ticket_copies",
    "ticket_escpos",
    "total_round_up",
  ], []);

  const printers = useMemo(() => devices.filter((device) => device.device_type === "printer"), [devices]);
  const serialDevices = useMemo(() => devices.filter((device) => device.device_type === "serial"), [devices]);
  const unconfiguredDevices = useMemo(() => devices.filter((device) => device.device_type === "unconfigured"), [devices]);
  const printerDevices = useMemo(() => {
    const byId = new Map<string, HardwareDevice>();
    [...printers, ...serialDevices].forEach((device) => byId.set(device.id, device));
    return Array.from(byId.values());
  }, [printers, serialDevices]);

  const syncDetectedDevices = async (
    result: HardwareDevice[],
    current: { printer: string; scale: string; drawer: string },
  ) => {
    const defaultPrinter = result.find((device) => device.device_type === "printer" && device.is_default) ?? result.find((device) => device.device_type === "printer");
    const defaultSerial = result.find((device) => device.device_type === "serial");
    const printerExists = result.some((device) => device.id === current.printer && (device.device_type === "printer" || device.device_type === "serial"));
    const scaleExists = result.some((device) => device.id === current.scale && device.device_type === "serial");
    const drawerExists = result.some((device) => device.id === current.drawer && (device.device_type === "printer" || device.device_type === "serial"));
    // Nunca borrar un dispositivo ya guardado por no detectarlo (ej. impresora WiFi).
    // Solo autollenar con el detectado por defecto cuando no hay nada configurado.
    const nextPrinter = printerExists ? current.printer : current.printer || defaultPrinter?.id || "";
    const nextScale = scaleExists ? current.scale : current.scale || defaultSerial?.id || "";
    const nextDrawer = drawerExists
      ? current.drawer
      : current.drawer || defaultSerial?.id || defaultPrinter?.id || "";

    if (nextPrinter !== current.printer) {
      setPrinter(nextPrinter);
    }
    if (nextScale !== current.scale) {
      setScale(nextScale);
    }
    if (nextDrawer !== current.drawer) {
      setDrawer(nextDrawer);
    }
    const updates: Record<string, string> = {};
    if (nextPrinter !== current.printer) {
      updates.printer = nextPrinter;
    }
    if (nextScale !== current.scale) {
      updates.scale = nextScale;
    }
    if (nextDrawer !== current.drawer) {
      updates.drawer = nextDrawer;
    }
    if (Object.keys(updates).length > 0) {
      await setSettings(updates);
    }
  };

  const detectDevices = useCallback(async (options?: { silent?: boolean }) => {
    setDetecting(true);
    try {
      const result = await listHardwareDevices({ includeNetwork: scanNetwork });
      setDevices(result);
      await syncDetectedDevices(result, { printer, scale, drawer });
      if (!options?.silent) {
        const usable = result.filter((device) => device.device_type !== "unconfigured");
        showToast(usable.length ? `${usable.length} dispositivos detectados` : "Sin dispositivos reales detectados");
      }
    } catch (error) {
      if (!options?.silent) showToast(String(error));
    } finally {
      setDetecting(false);
    }
  }, [drawer, printer, scale, scanNetwork, showToast]);

  // Number("") is 0, so an empty stored value would break baud/width/copies.
  const numberSetting = (value: string | null | undefined, fallback: number) => {
    if (value === null || value === undefined || value.trim() === "") return fallback;
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : fallback;
  };

  useEffect(() => {
    setCardTerminals(loadCardTerminals());
    getSettings(settingsKeys)
      .then((settings) => {
        const nextPrinter = settings.printer;
        const nextCutPrinter = settings.cut_printer;
        const nextWorkstationId = settings.workstation_id;
        const nextScale = settings.scale;
        const nextScaleBaudRate = settings.scale_baud_rate;
        const nextDrawer = settings.drawer;
        const nextTaxEnabled = settings.tax_enabled;
        const nextTaxCountry = settings.tax_country;
        const nextTaxDefaultRate = settings.tax_default_rate;
        const nextTaxPricesIncludeTax = settings.tax_prices_include_tax;
        const nextTaxShowBreakdown = settings.tax_show_breakdown;
        const nextTaxAutoApply = settings.tax_auto_apply_new_products;
        const nextTicketStoreName = settings.ticket_store_name;
        const nextTicketHeader = settings.ticket_header;
        const nextTicketFooter = settings.ticket_footer;
        const nextTicketWidth = settings.ticket_width;
        const nextTicketShowLogo = settings.ticket_show_logo;
        const nextTicketShowDate = settings.ticket_show_date;
        const nextTicketShowCashier = settings.ticket_show_cashier;
        const nextTicketShowBarcode = settings.ticket_show_barcode;
        const nextTicketShowItemCount = settings.ticket_show_item_count;
        const nextTicketStartLines = settings.ticket_start_lines;
        const nextTicketExtraLines = settings.ticket_extra_lines;
        const nextTicketCopies = settings.ticket_copies;
        const nextHardware = {
          printer: nextPrinter || "mock-printer-80mm",
          scale: nextScale || "mock-scale-serial",
          drawer: nextDrawer || "mock-drawer-escpos",
        };
        setPrinter(nextHardware.printer);
        setCutPrinter(nextCutPrinter || "");
        setWorkstationId(nextWorkstationId || "CAJA-1");
        setScale(nextHardware.scale);
        setScaleBaudRate(numberSetting(nextScaleBaudRate, 9600));
        setDrawer(nextHardware.drawer);
        const enabled = nextTaxEnabled !== "false";
        setTaxEnabled(enabled);
        setTaxCountry(nextTaxCountry || "MX");
        setTaxDefaultRate(numberSetting(nextTaxDefaultRate, 0.16));
        setTaxPricesIncludeTax(nextTaxPricesIncludeTax !== "false");
        setTaxShowBreakdown(nextTaxShowBreakdown !== "false");
        setTaxAutoApply(nextTaxAutoApply !== "false");
        setTicketStoreName(nextTicketStoreName || "RIM-POS");
        setTicketHeader(nextTicketHeader || "Abarrotes y miscelanea");
        setTicketFooter(nextTicketFooter || "Gracias por su compra");
        setTicketWidth(numberSetting(nextTicketWidth, 32));
        setTicketShowLogo(nextTicketShowLogo !== "false");
        setTicketShowDate(nextTicketShowDate !== "false");
        setTicketShowCashier(nextTicketShowCashier !== "false");
        setTicketShowBarcode(nextTicketShowBarcode === "true");
        setTicketShowItemCount(nextTicketShowItemCount !== "false");
        setTicketStartLines(numberSetting(nextTicketStartLines, 0));
        setTicketExtraLines(numberSetting(nextTicketExtraLines, 3));
        setTicketCopies(numberSetting(nextTicketCopies, 1));
        setTicketEscpos(settings.ticket_escpos !== "false");
        const nextRoundTotalUp = settings.total_round_up !== "false";
        setRoundTotalUp(nextRoundTotalUp);
        onTaxModeChange({ enabled, pricesIncludeTax: nextTaxPricesIncludeTax !== "false", roundTotalUp: nextRoundTotalUp });
      })
      .catch((error) => showToast(String(error)));
  }, [onTaxModeChange, settingsKeys, showToast]);

  const addCardTerminal = () => {
    const name = cardTerminalDraft.trim();
    if (name.length < 2) {
      showToast("Nombre de terminal requerido");
      return;
    }
    const next = saveCardTerminals([...cardTerminals, name]);
    setCardTerminals(next);
    setCardTerminalDraft("");
    showToast("Terminal agregada");
  };

  const removeCardTerminal = (terminal: string) => {
    const next = saveCardTerminals(cardTerminals.filter((item) => item !== terminal));
    setCardTerminals(next);
    showToast("Terminal eliminada");
  };

  const clampTicketLines = (value: number) => Math.max(0, Math.min(8, Math.floor(value)));

  const deriveTicketPreviewSettings = (value: string) => {
    const lines = value.replace(/\r/g, "").split("\n");
    let first = 0;
    while (first < lines.length && lines[first].trim() === "") first += 1;
    let last = lines.length - 1;
    while (last >= first && lines[last].trim() === "") last -= 1;
    const startLines = clampTicketLines(first);
    const extraLines = clampTicketLines(lines.length - 1 - last);
    const folioIndex = lines.findIndex((lineText) => lineText.startsWith("Folio 4581"));
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
      await setSettings({
        printer,
        cut_printer: cutPrinter,
        workstation_id: workstationId.trim() || "CAJA-1",
        scale,
        scale_baud_rate: String(scaleBaudRate),
        drawer,
        tax_enabled: String(taxEnabled),
        tax_country: taxCountry,
        tax_default_rate: String(taxDefaultRate),
        tax_prices_include_tax: String(taxPricesIncludeTax),
        tax_show_breakdown: String(taxShowBreakdown),
        tax_auto_apply_new_products: String(taxAutoApply),
        ticket_store_name: ticketStoreName,
        ticket_header: previewSettings?.header ?? ticketHeader,
        ticket_footer: previewSettings?.footer ?? ticketFooter,
        ticket_width: String(ticketWidth),
        ticket_show_logo: String(ticketShowLogo),
        ticket_show_date: String(ticketShowDate),
        ticket_show_cashier: String(ticketShowCashier),
        ticket_show_barcode: String(ticketShowBarcode),
        ticket_show_item_count: String(ticketShowItemCount),
        ticket_start_lines: String(previewSettings?.startLines ?? ticketStartLines),
        ticket_extra_lines: String(previewSettings?.extraLines ?? ticketExtraLines),
        ticket_copies: String(ticketCopies),
        ticket_escpos: String(ticketEscpos),
        total_round_up: String(roundTotalUp),
      });
      if (previewSettings) {
        setTicketHeader(previewSettings.header);
        setTicketFooter(previewSettings.footer);
        setTicketStartLines(previewSettings.startLines);
        setTicketExtraLines(previewSettings.extraLines);
        setTicketPreviewDirty(false);
      }
      onTaxModeChange({ enabled: taxEnabled, pricesIncludeTax: taxPricesIncludeTax, roundTotalUp });
      showToast("Configuracion guardada");
    } catch (error) {
      showToast(String(error));
    }
  };

  const saveHardwareSettings = async () => {
    await setSettings({
      printer,
      cut_printer: cutPrinter,
      scale,
      scale_baud_rate: String(scaleBaudRate),
      drawer,
    });
  };

  const testPrinter = async () => {
    try {
      await saveHardwareSettings();
      const result = await printTicket(0);
      showToast(result.message);
    } catch (error) {
      showToast(String(error));
    }
  };

  const testScale = async () => {
    try {
      await saveHardwareSettings();
      const result = await readScale();
      if (result.baud_rate !== scaleBaudRate) {
        setScaleBaudRate(result.baud_rate);
        await setSettings({ scale_baud_rate: String(result.baud_rate) });
      }
      showToast(`${result.weight} ${result.unit} · ${result.baud_rate} baud`);
    } catch (error) {
      showToast(String(error));
    }
  };

  const testDrawer = async () => {
    try {
      await saveHardwareSettings();
      const result = await openDrawer();
      showToast(result.message);
    } catch (error) {
      showToast(String(error));
    }
  };

  const deviceLabel = (device: HardwareDevice) => `${device.name}${device.is_default ? " (predeterminado)" : ""}`;
  const selectedDeviceName = (id: string) => {
    if (!id || id.startsWith("mock-")) return "Sin detectar";
    return devices.find((device) => device.id === id)?.name ?? `${id} (no detectada)`;
  };
  const line = "-".repeat(Math.max(24, Math.min(48, ticketWidth)));
  const ticketPreview = useMemo(() => {
    const width = Math.max(24, Math.min(48, ticketWidth));
    const separator = "-".repeat(width);
    const lines = [
      ...Array.from({ length: Math.max(0, Math.min(8, ticketStartLines)) }, () => ""),
      ...(ticketShowLogo ? [ticketStoreName || "RIM-POS"] : []),
      ...(ticketHeader.trim() ? ticketHeader.split("\n") : []),
      "Folio 4581",
      ...(ticketShowDate ? ["2026-06-20 08:20"] : []),
      ...(ticketShowCashier ? ["Cajero: Admin"] : []),
      separator,
      "Refresco cola 600 ml",
      "  2.000 @ $18.00  $36.00",
      ...(ticketShowBarcode ? ["  750000000001"] : []),
      "",
      "Arroz 1 kg",
      "  1.000 @ $32.00  $32.00",
      ...(ticketShowBarcode ? ["  750000000002"] : []),
      "",
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
                    <option value={32}>58 mm / 32 caracteres</option>
                    <option value={40}>80 mm / 40 caracteres</option>
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
                  <label className="toggle-row">
                    <input type="checkbox" checked={roundTotalUp} onChange={(event) => setRoundTotalUp(event.target.checked)} />
                    Redondeo total a pesos enteros
                  </label>
                </div>
              </div>
              <aside className="ticket-preview-card" aria-label="Vista previa del ticket">
                <div className="ticket-preview-title">
                  <strong>Vista previa del ticket</strong>
                  <span>{line.length} caracteres</span>
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
                  aria-label="Editar vista previa del ticket"
                />
                <div className="ticket-card-note">
                  <span>En ventas con tarjeta se agrega al final (no editable):</span>
                  <pre className="ticket-preview card-voucher">{[
                    line,
                    "VENTA A CREDITO",
                    "FIRMA DEL CLIENTE",
                    "",
                    "",
                    "",
                    "_".repeat(Math.max(24, Math.min(48, ticketWidth))),
                    "TERMINAL BANORTE-1",
                  ].join("\n")}</pre>
                </div>
                <button className="ghost-button" type="button" onClick={testPrinter}>
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
                Impuesto predeterminado
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
                  <option value="true">Aplicar impuesto predeterminado</option>
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
                  {printer && !printerDevices.some((device) => device.id === printer) && (
                    <option value={printer}>{printer} (guardada, no detectada)</option>
                  )}
                  {printerDevices.map((device) => (
                    <option value={device.id} key={device.id}>
                      {device.device_type === "serial" ? `Directo por ${deviceLabel(device)}` : deviceLabel(device)}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Tipo de impresora ticket
                <select value={ticketEscpos ? "escpos" : "normal"} onChange={(event) => setTicketEscpos(event.target.value === "escpos")}>
                  <option value="escpos">Termica ESC/POS (58mm, corta papel)</option>
                  <option value="normal">Normal con driver (Brother, laser, tinta)</option>
                </select>
              </label>
              <label>
                Impresora corte
                <select value={cutPrinter} onChange={(event) => setCutPrinter(event.target.value)}>
                  <option value="">Usar la misma que ticket</option>
                  {cutPrinter && !printerDevices.some((device) => device.id === cutPrinter) && (
                    <option value={cutPrinter}>{cutPrinter} (guardada, no detectada)</option>
                  )}
                  {printerDevices.map((device) => (
                    <option value={device.id} key={`cut-${device.id}`}>
                      {device.device_type === "serial" ? `Directo por ${deviceLabel(device)}` : deviceLabel(device)}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Bascula
                <select value={scale} onChange={(event) => setScale(event.target.value)}>
                  <option value="">Seleccionar puerto</option>
                  {scale && !serialDevices.some((device) => device.id === scale) && (
                    <option value={scale}>{scale} (guardada, no detectada)</option>
                  )}
                  {serialDevices.map((device) => (
                    <option value={device.id} key={device.id}>{deviceLabel(device)}</option>
                  ))}
                </select>
              </label>
              <label>
                Baud bascula
                <select
                  value={scaleBaudRate}
                  onChange={(event) => {
                    setScaleBaudRate(Number(event.target.value));
                  }}
                >
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
                  {drawer && !printers.some((device) => device.id === drawer) && !serialDevices.some((device) => device.id === drawer) && (
                    <option value={drawer}>{drawer} (guardada, no detectada)</option>
                  )}
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
              <button className="ghost-button" type="button" onClick={() => void detectDevices()} disabled={detecting}>
                {detecting ? "Detectando" : "Detectar dispositivos"}
              </button>
              <button className="ghost-button" type="button" onClick={testPrinter}>Probar impresora</button>
              <button className="ghost-button" type="button" onClick={testScale}>Probar bascula</button>
              <button className="ghost-button" type="button" onClick={testDrawer}>Probar cajon</button>
            </div>
            <label className="toggle-row">
              <input
                type="checkbox"
                checked={scanNetwork}
                onChange={(event) => setScanNetwork(event.target.checked)}
              />
              Incluir impresoras de red (mas lento, escanea la red local)
            </label>
            {unconfiguredDevices.length > 0 && (
              <div className="muted-note unconfigured-device-list">
                <strong>Detectados sin configurar:</strong>
                {unconfiguredDevices.map((device) => (
                  <div key={`${device.device_type}-${device.id}`}>
                    <strong>{device.name}</strong>: {device.detail}
                  </div>
                ))}
              </div>
            )}
          </section>

          <section className="settings-section">
            <div className="settings-section-title">
              <div>
                <h2>Terminales</h2>
                <p>Terminales bancarias para pagos con tarjeta.</p>
              </div>
            </div>
            <div className="terminal-config">
              <label>
                Nombre de terminal
                <input
                  value={cardTerminalDraft}
                  onChange={(event) => setCardTerminalDraft(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.preventDefault();
                      addCardTerminal();
                    }
                  }}
                  placeholder="Ej. Clip mostrador"
                />
              </label>
              <button className="primary-button" type="button" onClick={addCardTerminal}>Agregar terminal</button>
            </div>
            <div className="terminal-list">
              {cardTerminals.length === 0 ? (
                <div className="muted-note">Sin terminales agregadas.</div>
              ) : cardTerminals.map((terminal) => (
                <div className="terminal-row" key={terminal}>
                  <div>
                    <strong>{terminal}</strong>
                    <span>Disponible en Caja al pagar con tarjeta</span>
                  </div>
                  <button className="icon-button danger" type="button" aria-label={`Eliminar ${terminal}`} onClick={() => removeCardTerminal(terminal)}>
                    <Trash2 size={16} />
                  </button>
                </div>
              ))}
            </div>
          </section>

        </div>

        <aside className="settings-side">
          <div className="settings-list">
            <Setting icon={Archive} label="Caja/estacion" value={workstationId || "CAJA-1"} />
            <Setting icon={Printer} label="Impresora ticket" value={selectedDeviceName(printer)} />
            <Setting icon={Printer} label="Impresora corte" value={cutPrinter ? selectedDeviceName(cutPrinter) : "Misma que ticket"} />
            <Setting icon={Scale} label="Bascula" value={selectedDeviceName(scale)} />
            <Setting icon={Archive} label="Cajon" value={selectedDeviceName(drawer)} />
            <Setting icon={CreditCard} label="Terminales" value={`${cardTerminals.length}`} />
            <Setting icon={Percent} label="Impuestos" value={taxEnabled ? `${Math.round(taxDefaultRate * 100)}%, ${taxPricesIncludeTax ? "incluidos" : "sumados"}` : "Desactivados"} />
          </div>
          <div className="device-list">
            {devices.length === 0 ? (
              <div className="muted-note">Sin dispositivos reales detectados.</div>
            ) : devices.map((device) => (
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
