import { describe, expect, it } from "vitest";
import { csvEscape, parseCsvLine } from "./csv";

describe("csv helpers", () => {
  it("escapes quotes and wraps cells", () => {
    expect(csvEscape('A "quote"')).toBe('"A ""quote"""');
  });

  it("neutralizes spreadsheet formulas", () => {
    expect(csvEscape("=SUM(A1:A2)")).toBe('"\'=SUM(A1:A2)"');
    expect(csvEscape("+cmd")).toBe('"\'+cmd"');
    expect(csvEscape("-10")).toBe('"\'-10"');
    expect(csvEscape("@user")).toBe('"\'@user"');
  });

  it("parses quoted commas and escaped quotes", () => {
    expect(parseCsvLine('"A,B","C ""D""",E')).toEqual(["A,B", 'C "D"', "E"]);
  });
});
