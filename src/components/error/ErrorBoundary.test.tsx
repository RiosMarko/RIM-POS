import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ErrorBoundary } from "./ErrorBoundary";

describe("ErrorBoundary", () => {
  it("renders healthy children", () => {
    render(
      <ErrorBoundary resetKey="sale">
        <div>Vista sana</div>
      </ErrorBoundary>,
    );

    expect(screen.getByText("Vista sana")).toBeInTheDocument();
  });

  it("derives fallback state from error", () => {
    expect(ErrorBoundary.getDerivedStateFromError(new Error("Vista rota"))).toEqual({
      error: expect.objectContaining({ message: "Vista rota" }),
    });
  });
});
