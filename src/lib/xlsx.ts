import { strFromU8, strToU8, unzipSync, zipSync } from "fflate";
import type { Product, ProductImportRow, TaxOption } from "../types";

const eleventaHeader = [
  "Código",
  "Producto",
  "P. Costo",
  "P. Venta",
  "P. Mayoreo",
  "Departamento",
  "Existencia",
  "Inv. Mínimo",
  "Inv. Máximo",
  "Tipo de Venta",
  "IVA",
  "IEPS",
];

const xmlHeader = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>';
const spreadsheetNs = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";

function xmlEscape(value: string) {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function normalizeHeader(value: string) {
  return value
    .toLowerCase()
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .replace(/[^a-z0-9]/g, "");
}

function columnName(index: number) {
  let value = index + 1;
  let name = "";
  while (value > 0) {
    const remainder = (value - 1) % 26;
    name = String.fromCharCode(65 + remainder) + name;
    value = Math.floor((value - 1) / 26);
  }
  return name;
}

function columnIndex(ref: string) {
  const letters = ref.match(/[A-Z]+/i)?.[0]?.toUpperCase() ?? "A";
  return [...letters].reduce((sum, letter) => sum * 26 + letter.charCodeAt(0) - 64, 0) - 1;
}

function parseMoney(value: string) {
  const normalized = value.replace(/\$/g, "").replace(/,/g, "").trim();
  const parsed = Number(normalized);
  return Number.isFinite(parsed) ? parsed : 0;
}

function parseRate(value: string) {
  const parsed = parseMoney(value);
  if (!Number.isFinite(parsed) || parsed <= 0) return 0;
  return parsed > 1 ? parsed / 100 : parsed;
}

function unitFromEleventa(value: string) {
  const normalized = value.toLowerCase();
  if (normalized.includes("granel") || normalized.includes("kilo") || normalized.includes("kg")) return "kg";
  if (normalized.includes("litro")) return "litro";
  return "pieza";
}

function cleanEleventaDepartment(value: string) {
  const trimmed = value.trim();
  if (!trimmed || trimmed === "- Sin Departamento -") return "Abarrotes";
  return trimmed;
}

function taxIdsForRates(taxOptions: TaxOption[], ivaRate: number, iepsRate: number) {
  const ids: number[] = [];
  const active = taxOptions.filter((tax) => tax.is_active);
  const findTax = (type: string, rate: number) =>
    active.find((tax) => tax.type === type && Math.abs(tax.rate - rate) < 0.0001);
  if (ivaRate > 0) {
    const tax = findTax("IVA", ivaRate);
    if (tax) ids.push(tax.id);
  } else {
    const tax = findTax("IVA", 0);
    if (tax) ids.push(tax.id);
  }
  if (iepsRate > 0) {
    const tax = findTax("IEPS", iepsRate);
    if (tax) ids.push(tax.id);
  }
  return ids;
}

function parseSharedStrings(xml: string) {
  const doc = new DOMParser().parseFromString(xml, "application/xml");
  return Array.from(doc.getElementsByTagNameNS(spreadsheetNs, "si")).map((node) =>
    Array.from(node.getElementsByTagNameNS(spreadsheetNs, "t"))
      .map((text) => text.textContent ?? "")
      .join(""),
  );
}

function cellText(cell: Element, sharedStrings: string[]) {
  const type = cell.getAttribute("t");
  if (type === "inlineStr") {
    return Array.from(cell.getElementsByTagNameNS(spreadsheetNs, "t"))
      .map((text) => text.textContent ?? "")
      .join("");
  }
  const value = cell.getElementsByTagNameNS(spreadsheetNs, "v")[0]?.textContent ?? "";
  if (type === "s") return sharedStrings[Number(value)] ?? "";
  return value;
}

export async function readXlsxRows(file: File) {
  const bytes = new Uint8Array(await file.arrayBuffer());
  const entries = unzipSync(bytes);
  const sheetEntry = entries["xl/worksheets/sheet1.xml"];
  if (!sheetEntry) throw new Error("XLSX sin hoja principal");
  const sharedStringsEntry = entries["xl/sharedStrings.xml"];
  const sharedStrings = sharedStringsEntry ? parseSharedStrings(strFromU8(sharedStringsEntry)) : [];
  const sheet = new DOMParser().parseFromString(strFromU8(sheetEntry), "application/xml");
  return Array.from(sheet.getElementsByTagNameNS(spreadsheetNs, "row")).map((row) => {
    const values: string[] = [];
    Array.from(row.getElementsByTagNameNS(spreadsheetNs, "c")).forEach((cell) => {
      values[columnIndex(cell.getAttribute("r") ?? "A")] = cellText(cell, sharedStrings);
    });
    return values.map((value) => value ?? "");
  });
}

export async function parseEleventaCatalogXlsx(file: File, taxOptions: TaxOption[]) {
  const rows = await readXlsxRows(file);
  if (rows.length === 0) throw new Error("XLSX vacio");
  const header = rows[0].map(normalizeHeader);
  const col = (name: string) => header.indexOf(normalizeHeader(name));
  const codeIndex = col("Código");
  const nameIndex = col("Producto");
  const costIndex = col("P. Costo");
  const priceIndex = col("P. Venta");
  const wholesaleIndex = col("P. Mayoreo");
  const departmentIndex = col("Departamento");
  const stockIndex = col("Existencia");
  const minStockIndex = col("Inv. Mínimo");
  const unitIndex = col("Tipo de Venta");
  const ivaIndex = col("IVA");
  const iepsIndex = col("IEPS");
  if (codeIndex < 0 || nameIndex < 0 || priceIndex < 0) {
    throw new Error("XLSX no parece catalogo eleventa");
  }

  return rows.slice(1).flatMap((row, index): ProductImportRow[] => {
    const rowNumber = index + 2;
    const name = (row[nameIndex] ?? "").trim().replace(/\s+/g, " ");
    if (!name) return [];
    const barcode = (row[codeIndex] ?? "").trim();
    const safeSku = barcode;
    const ivaRate = parseRate(row[ivaIndex] ?? "");
    const iepsRate = parseRate(row[iepsIndex] ?? "");
    const tax_ids = taxIdsForRates(taxOptions, ivaRate, iepsRate);
    return [{
      row_number: rowNumber,
      sku: safeSku.toUpperCase(),
      barcode,
      name,
      category: cleanEleventaDepartment(row[departmentIndex] ?? ""),
      unit: unitFromEleventa(row[unitIndex] ?? ""),
      price: parseMoney(row[priceIndex] ?? ""),
      wholesale_price: wholesaleIndex >= 0 ? parseMoney(row[wholesaleIndex] ?? "") || null : null,
      cost: parseMoney(row[costIndex] ?? ""),
      stock: parseMoney(row[stockIndex] ?? ""),
      min_stock: parseMoney(row[minStockIndex] ?? ""),
      tax_ids,
      tax_rate: ivaRate + iepsRate,
      active: true,
    }];
  });
}

function sheetXml(rows: Array<Array<string | number>>) {
  const body = rows.map((row, rowIndex) => {
    const cells = row.map((value, colIndex) => {
      const ref = `${columnName(colIndex)}${rowIndex + 1}`;
      if (typeof value === "number") return `<c r="${ref}"><v>${value}</v></c>`;
      return `<c r="${ref}" t="inlineStr"><is><t>${xmlEscape(value)}</t></is></c>`;
    }).join("");
    return `<row r="${rowIndex + 1}">${cells}</row>`;
  }).join("");
  return `${xmlHeader}<worksheet xmlns="${spreadsheetNs}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheetData>${body}</sheetData></worksheet>`;
}

function workbookXml(sheetName = "Catalogo") {
  return `${xmlHeader}<workbook xmlns="${spreadsheetNs}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="${xmlEscape(sheetName)}" sheetId="1" r:id="rId1"/></sheets></workbook>`;
}

function workbookRelsXml() {
  return `${xmlHeader}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>`;
}

function rootRelsXml() {
  return `${xmlHeader}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>`;
}

function contentTypesXml() {
  return `${xmlHeader}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>`;
}

export function eleventaRowsFromProducts(products: Product[], taxOptions: TaxOption[] = []) {
  return [
    eleventaHeader,
    ...products.map((product) => {
      const taxSummary = product.tax_ids.reduce(
        (summary, taxId) => {
          const tax = taxOptions.find((option) => option.id === taxId);
          if (tax?.type === "IVA") summary.iva += tax.rate;
          if (tax?.type === "IEPS") summary.ieps += tax.rate;
          return summary;
        },
        { iva: 0, ieps: 0 },
      );
      const ivaRate = taxSummary.iva || (product.tax_rate > 0 ? Math.min(product.tax_rate, 0.16) : 0);
      const iepsRate = taxSummary.ieps || Math.max(0, product.tax_rate - ivaRate);
      const iva = Math.round(ivaRate * 1000) / 10;
      const ieps = Math.round(iepsRate * 1000) / 10;
      return [
        product.barcode,
        product.name,
        product.cost,
        product.price,
        product.wholesale_price ?? 0,
        product.category || "- Sin Departamento -",
        product.stock,
        product.min_stock,
        0,
        product.unit === "pieza" ? "UNIDAD" : "GRANEL",
        iva,
        ieps || "",
      ];
    }),
  ];
}

export function downloadXlsx(filename: string, rows: Array<Array<string | number>>) {
  const zipped = zipSync({
    "[Content_Types].xml": strToU8(contentTypesXml()),
    "_rels/.rels": strToU8(rootRelsXml()),
    "xl/workbook.xml": strToU8(workbookXml()),
    "xl/_rels/workbook.xml.rels": strToU8(workbookRelsXml()),
    "xl/worksheets/sheet1.xml": strToU8(sheetXml(rows)),
  });
  downloadBytes(filename, zipped, XLSX_MIME);
}

export const XLSX_MIME = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";

export function downloadBytes(filename: string, bytes: Uint8Array, mime: string) {
  const blob = new Blob([bytes as unknown as BlobPart], { type: mime });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}

export type XlsxCellStyle = "title" | "section" | "label" | "money" | "plain" | "totalLabel" | "totalMoney";
export type XlsxCell = { value: string | number; style?: XlsxCellStyle };
export type XlsxRow = XlsxCell[];

export function cell(value: string | number, style?: XlsxCellStyle): XlsxCell {
  return { value, style };
}

const styledCellXf: Record<XlsxCellStyle, number> = {
  plain: 0,
  title: 1,
  section: 2,
  money: 3,
  label: 4,
  totalLabel: 5,
  totalMoney: 6,
};

function styledContentTypesXml() {
  return `${xmlHeader}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>`;
}

function styledWorkbookRelsXml() {
  return `${xmlHeader}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>`;
}

function stylesXml() {
  return `${xmlHeader}<styleSheet xmlns="${spreadsheetNs}">` +
    `<numFmts count="1"><numFmt numFmtId="164" formatCode="&quot;$&quot;#,##0.00"/></numFmts>` +
    `<fonts count="5">` +
    `<font><sz val="11"/><name val="Calibri"/></font>` +
    `<font><b/><sz val="13"/><color rgb="FFFFFFFF"/><name val="Calibri"/></font>` +
    `<font><b/><sz val="11"/><color rgb="FFFFFFFF"/><name val="Calibri"/></font>` +
    `<font><b/><sz val="11"/><name val="Calibri"/></font>` +
    `<font><b/><sz val="18"/><color rgb="FF1F6F4A"/><name val="Calibri"/></font>` +
    `</fonts>` +
    `<fills count="4">` +
    `<fill><patternFill patternType="none"/></fill>` +
    `<fill><patternFill patternType="gray125"/></fill>` +
    `<fill><patternFill patternType="solid"><fgColor rgb="FF1F6F4A"/><bgColor indexed="64"/></patternFill></fill>` +
    `<fill><patternFill patternType="solid"><fgColor rgb="FF3D8C63"/><bgColor indexed="64"/></patternFill></fill>` +
    `</fills>` +
    `<borders count="2">` +
    `<border><left/><right/><top/><bottom/><diagonal/></border>` +
    `<border><left style="thin"><color rgb="FFE0E0E0"/></left><right style="thin"><color rgb="FFE0E0E0"/></right><top style="thin"><color rgb="FFE0E0E0"/></top><bottom style="thin"><color rgb="FFE0E0E0"/></bottom></border>` +
    `</borders>` +
    `<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>` +
    `<cellXfs count="7">` +
    `<xf numFmtId="0" fontId="0" fillId="0" borderId="1" xfId="0" applyBorder="1"/>` +
    `<xf numFmtId="0" fontId="1" fillId="2" borderId="0" xfId="0" applyFont="1" applyFill="1"/>` +
    `<xf numFmtId="0" fontId="2" fillId="3" borderId="0" xfId="0" applyFont="1" applyFill="1"/>` +
    `<xf numFmtId="164" fontId="0" fillId="0" borderId="1" xfId="0" applyNumFmt="1" applyBorder="1"/>` +
    `<xf numFmtId="0" fontId="3" fillId="0" borderId="1" xfId="0" applyFont="1" applyBorder="1"/>` +
    `<xf numFmtId="0" fontId="4" fillId="0" borderId="0" xfId="0" applyFont="1"/>` +
    `<xf numFmtId="164" fontId="4" fillId="0" borderId="0" xfId="0" applyFont="1" applyNumFmt="1"/>` +
    `</cellXfs>` +
    `</styleSheet>`;
}

function styledSheetXml(rows: XlsxRow[], columnWidths: number[], dataBarRanges: string[]) {
  const rowHeight = (row: XlsxRow) => {
    if (row.some((item) => item.style === "title")) return ' ht="24" customHeight="1"';
    if (row.some((item) => item.style === "totalMoney" || item.style === "totalLabel")) return ' ht="28" customHeight="1"';
    if (row.some((item) => item.style === "section")) return ' ht="18" customHeight="1"';
    return "";
  };
  const cols = `<cols>${columnWidths.map((width, index) => `<col min="${index + 1}" max="${index + 1}" width="${width}" customWidth="1"/>`).join("")}</cols>`;
  const body = rows.map((row, rowIndex) => {
    const cells = row.map((item, colIndex) => {
      const ref = `${columnName(colIndex)}${rowIndex + 1}`;
      const styleAttr = ` s="${styledCellXf[item.style ?? "plain"]}"`;
      if (item.value === "") return `<c r="${ref}"${styleAttr}/>`;
      if (typeof item.value === "number") return `<c r="${ref}"${styleAttr}><v>${item.value}</v></c>`;
      return `<c r="${ref}"${styleAttr} t="inlineStr"><is><t>${xmlEscape(item.value)}</t></is></c>`;
    }).join("");
    return `<row r="${rowIndex + 1}"${rowHeight(row)}>${cells}</row>`;
  }).join("");
  const dataBars = dataBarRanges.map((range, index) => (
    `<conditionalFormatting sqref="${range}"><cfRule type="dataBar" priority="${index + 1}"><dataBar><cfvo type="min"/><cfvo type="max"/><color rgb="FF3D8C63"/></dataBar></cfRule></conditionalFormatting>`
  )).join("");
  return `${xmlHeader}<worksheet xmlns="${spreadsheetNs}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">${cols}<sheetData>${body}</sheetData>${dataBars}</worksheet>`;
}

export function buildStyledXlsxBytes(
  sheetName: string,
  rows: XlsxRow[],
  columnWidths: number[],
  dataBarRanges: string[] = [],
): Uint8Array {
  return zipSync({
    "[Content_Types].xml": strToU8(styledContentTypesXml()),
    "_rels/.rels": strToU8(rootRelsXml()),
    "xl/workbook.xml": strToU8(workbookXml(sheetName)),
    "xl/_rels/workbook.xml.rels": strToU8(styledWorkbookRelsXml()),
    "xl/styles.xml": strToU8(stylesXml()),
    "xl/worksheets/sheet1.xml": strToU8(styledSheetXml(rows, columnWidths, dataBarRanges)),
  });
}
