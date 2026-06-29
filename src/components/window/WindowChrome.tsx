import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";
import rimPosLogo from "../../assets/rim-pos-icon.png";

export function WindowTitlebar({
  clock,
  roleLabel,
  sessionName,
}: {
  clock: Date;
  roleLabel: string;
  sessionName?: string;
}) {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    const appWindow = getCurrentWindow();
    appWindow.isMaximized().then(setMaximized).catch(() => undefined);
    let cleanup: (() => void) | undefined;
    appWindow.onResized(() => {
      appWindow.isMaximized().then(setMaximized).catch(() => undefined);
    }).then((unlisten) => {
      cleanup = unlisten;
    }).catch(() => undefined);
    return () => cleanup?.();
  }, []);

  const minimizeWindow = () => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    getCurrentWindow().minimize().catch(() => undefined);
  };

  const toggleMaximizeWindow = async () => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    const appWindow = getCurrentWindow();
    const isMaximized = await appWindow.isMaximized();
    if (isMaximized) {
      await appWindow.unmaximize();
      setMaximized(false);
      return;
    }
    await appWindow.maximize();
    setMaximized(true);
  };

  const closeWindow = () => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    getCurrentWindow().close().catch(() => undefined);
  };

  return (
    <header
      className="window-titlebar"
      data-tauri-drag-region
      aria-label="Barra de ventana"
      onDoubleClick={(event) => {
        if ((event.target as HTMLElement).closest("button")) return;
        void toggleMaximizeWindow();
      }}
    >
      <div className="window-titlebar-brand" data-tauri-drag-region>
        <img src={rimPosLogo} alt="" data-tauri-drag-region />
      </div>
      <div className="window-titlebar-status" data-tauri-drag-region>
        {sessionName && <span data-tauri-drag-region>{roleLabel}: {sessionName}</span>}
        <strong data-tauri-drag-region>{clock.toLocaleTimeString("es-MX", { hour: "2-digit", minute: "2-digit" })}</strong>
      </div>
      <div className="window-controls" aria-label="Controles de ventana">
        <button type="button" aria-label="Minimizar" onClick={minimizeWindow}>
          <Minus size={14} strokeWidth={2.6} />
        </button>
        <button type="button" aria-label={maximized ? "Restaurar" : "Maximizar"} onClick={toggleMaximizeWindow}>
          <Square size={12} strokeWidth={2.4} />
        </button>
        <button className="close" type="button" aria-label="Cerrar" onClick={closeWindow}>
          <X size={15} strokeWidth={2.6} />
        </button>
      </div>
    </header>
  );
}

export function WindowTransitionCover({ phase }: { phase: "cover" | "reveal" }) {
  return (
    <div className={`window-transition-cover ${phase}`} aria-hidden="true">
      <img src={rimPosLogo} alt="" />
    </div>
  );
}
