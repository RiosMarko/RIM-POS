import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ToastStack, getNotificationTone } from "./ToastStack";

describe("ToastStack", () => {
  it("maps message tone", () => {
    expect(getNotificationTone("Producto guardado")).toBe("success");
    expect(getNotificationTone("Stock insuficiente")).toBe("danger");
    expect(getNotificationTone("Corte X listo")).toBe("success");
  });

  it("treats the sale toast as success (it never includes hardware errors)", () => {
    expect(getNotificationTone("Venta 24 realizada")).toBe("success");
  });

  it("dismisses notification by id", () => {
    const onDismiss = vi.fn();
    render(
      <ToastStack
        notifications={[{ id: 7, message: "Producto guardado", tone: "success" }]}
        onDismiss={onDismiss}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /cerrar notificacion/i }));

    expect(onDismiss).toHaveBeenCalledWith(7);
  });
});
