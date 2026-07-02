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

function workbookXml() {
  return `${xmlHeader}<workbook xmlns="${spreadsheetNs}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Catalogo" sheetId="1" r:id="rId1"/></sheets></workbook>`;
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
        product.unit === "kg" ? "GRANEL" : "UNIDAD",
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
  const blob = new Blob([zipped], {
    type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
  });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}
