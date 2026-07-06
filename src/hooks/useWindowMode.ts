import { currentMonitor, getCurrentWindow, LogicalPosition, LogicalSize } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState } from "react";
import type { UserSession } from "../types";

const LOGIN_WINDOW_SIZE = new LogicalSize(760, 440);
const POS_WINDOW_SIZE = new LogicalSize(1366, 768);
const POS_MIN_WINDOW_SIZE = new LogicalSize(1100, 680);

const wait = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));
const nextFrame = () => new Promise((resolve) => window.requestAnimationFrame(resolve));

async function setAppWindowMode(mode: "login" | "pos") {
  if (!("__TAURI_INTERNALS__" in window)) return;
  const appWindow = getCurrentWindow();
  if (mode === "login") {
    await appWindow.setResizable(true);
    await appWindow.setMaximizable(true);
    await appWindow.setMaxSize(null);
    await appWindow.setMinSize(LOGIN_WINDOW_SIZE);
    await appWindow.unmaximize();
    await wait(80);
    await appWindow.setSize(LOGIN_WINDOW_SIZE);
    await appWindow.center();
    await wait(80);
    await appWindow.setSize(LOGIN_WINDOW_SIZE);
    await appWindow.center();
    return;
  }
  await appWindow.setResizable(true);
  await appWindow.setMaximizable(true);
  await appWindow.setMaxSize(null);
  await appWindow.setMinSize(POS_MIN_WINDOW_SIZE);
  const monitor = await currentMonitor();
  if (monitor) {
    const position = monitor.workArea.position.toLogical(monitor.scaleFactor);
    const size = monitor.workArea.size.toLogical(monitor.scaleFactor);
    await appWindow.setPosition(new LogicalPosition(position.x, position.y));
    await appWindow.setSize(new LogicalSize(size.width, size.height));
  } else {
    await appWindow.setSize(POS_WINDOW_SIZE);
  }
  await appWindow.maximize();
}

export function useWindowMode(session: UserSession | null) {
  const [loginMaximized, setLoginMaximized] = useState(false);
  const [windowTransition, setWindowTransition] = useState<"idle" | "cover" | "reveal">("idle");
  const windowModeRef = useRef<"login" | "pos">("login");

  const logWindowError = useCallback((error: unknown) => {
    console.warn("Window mode update failed", error);
  }, []);

  useEffect(() => {
    windowModeRef.current = "login";
    setLoginMaximized(false);
    setAppWindowMode("login").catch(logWindowError);
  }, [logWindowError]);

  const coverWindowTransition = useCallback(async () => {
    setWindowTransition("cover");
    await wait(130);
  }, []);

  const revealWindowTransition = useCallback(async () => {
    await nextFrame();
    setWindowTransition("reveal");
    window.setTimeout(() => setWindowTransition("idle"), 190);
  }, []);

  useEffect(() => {
    if (session || !("__TAURI_INTERNALS__" in window)) return;
    let resizeTimer: number | undefined;
    let disposed = false;
    const appWindow = getCurrentWindow();
    const syncLoginWindow = async () => {
      if (windowModeRef.current !== "login") return;
      const maximized = await appWindow.isMaximized();
      setLoginMaximized(maximized);
      if (!maximized) {
        await appWindow.setSize(LOGIN_WINDOW_SIZE);
      }
    };
    syncLoginWindow().catch(logWindowError);
    let cleanup: (() => void) | undefined;
    appWindow
      .onResized(() => {
        window.clearTimeout(resizeTimer);
        resizeTimer = window.setTimeout(() => {
          if (disposed || windowModeRef.current !== "login") return;
          syncLoginWindow().catch(logWindowError);
        }, 80);
      })
      .then((unlisten) => {
        if (disposed) {
          Promise.resolve(unlisten()).catch(logWindowError);
          return;
        }
        cleanup = unlisten;
      })
      .catch(logWindowError);
    return () => {
      disposed = true;
      window.clearTimeout(resizeTimer);
      if (cleanup) Promise.resolve(cleanup()).catch(logWindowError);
    };
  }, [logWindowError, session]);

  const enterPosMode = useCallback(
    async (beforeReveal: () => void | Promise<void>) => {
      await coverWindowTransition();
      windowModeRef.current = "pos";
      setLoginMaximized(true);
      try {
        await setAppWindowMode("pos");
      } catch (error) {
        logWindowError(error);
      }
      await beforeReveal();
      await revealWindowTransition();
    },
    [coverWindowTransition, logWindowError, revealWindowTransition],
  );

  const enterLoginMode = useCallback(
    async (beforeReveal: () => void | Promise<void>) => {
      await coverWindowTransition();
      setLoginMaximized(false);
      windowModeRef.current = "login";
      try {
        await setAppWindowMode("login");
      } catch (error) {
        logWindowError(error);
      }
      await beforeReveal();
      await revealWindowTransition();
    },
    [coverWindowTransition, logWindowError, revealWindowTransition],
  );

  return { loginMaximized, windowTransition, enterPosMode, enterLoginMode };
}
