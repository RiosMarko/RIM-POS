import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const width = 512;
const height = 512;
const bg = [35, 139, 92, 255];
const bgDark = [25, 86, 70, 255];
const fg = [255, 255, 255, 255];

const letters = {
  P: [
    "111110",
    "110011",
    "110011",
    "111110",
    "110000",
    "110000",
    "110000",
  ],
  A: [
    "001100",
    "011110",
    "110011",
    "110011",
    "111111",
    "110011",
    "110011",
  ],
};

function crc32(buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc ^= byte;
    for (let i = 0; i < 8; i += 1) {
      crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const typeBuffer = Buffer.from(type);
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuffer, data])));
  return Buffer.concat([length, typeBuffer, data, crc]);
}

function putPixel(raw, x, y, color) {
  if (x < 0 || y < 0 || x >= width || y >= height) return;
  const offset = y * (width * 4 + 1) + 1 + x * 4;
  raw[offset] = color[0];
  raw[offset + 1] = color[1];
  raw[offset + 2] = color[2];
  raw[offset + 3] = color[3];
}

function drawRect(raw, x, y, w, h, color) {
  for (let yy = y; yy < y + h; yy += 1) {
    for (let xx = x; xx < x + w; xx += 1) {
      putPixel(raw, xx, yy, color);
    }
  }
}

function drawLetter(raw, glyph, startX, startY, scale) {
  const rows = letters[glyph];
  rows.forEach((row, rowIndex) => {
    [...row].forEach((cell, colIndex) => {
      if (cell === "1") {
        drawRect(raw, startX + colIndex * scale, startY + rowIndex * scale, scale - 3, scale - 3, fg);
      }
    });
  });
}

const raw = Buffer.alloc(height * (width * 4 + 1));
for (let y = 0; y < height; y += 1) {
  raw[y * (width * 4 + 1)] = 0;
  for (let x = 0; x < width; x += 1) {
    const radius = Math.hypot(x - width / 2, y - height / 2);
    const color = radius > 218 ? bgDark : bg;
    putPixel(raw, x, y, color);
  }
}

drawLetter(raw, "P", 92, 132, 34);
drawLetter(raw, "A", 280, 132, 34);

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(width, 0);
ihdr.writeUInt32BE(height, 4);
ihdr[8] = 8;
ihdr[9] = 6;
ihdr[10] = 0;
ihdr[11] = 0;
ihdr[12] = 0;

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw)),
  chunk("IEND", Buffer.alloc(0)),
]);

const out = resolve("src-tauri/icons/icon.png");
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, png);
console.log(out);

