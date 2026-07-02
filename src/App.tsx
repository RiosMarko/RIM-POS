import {
  Banknote,
  BarChart3,
  Barcode,
  Boxes,
  DatabaseBackup,
  PackagePlus,
  Settings,
  ShoppingCart,
  UserPlus,
  Users,
} from "lucide-react";
import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ToastStack } from "./components/feedback/ToastStack";
import { ErrorBoundary } from "./components/error/ErrorBoundary";
import { AppShell } from "./components/layout/AppShell";
import { ConfirmActionModal, NumberPromptModal, type ConfirmDraft, type NumberPromptDraft } from "./components/modals/CommonModals";
import { WindowTitlebar, WindowTransitionCover } from "./components/window/WindowChrome";
import { AdminView, preloadAdminViews } from "./features/admin/AdminView";
import { AdminGate, LoginScreen } from "./features/auth/AuthScreens";
import { ExpenseDialog } from "./features/cash/CashModals";
import {
  DeleteHeldTicketModal,
  HeldTicketsModal,
  RecoveryDraftModal,
  ShortcutHelp,
  TicketNameModal,
} from "./features/sales/SaleModals";
import { SaleView } from "./features/sales/SaleView";
import { useClock } from "./hooks/useClock";
import { useAdminNavigation } from "./hooks/useAdminNavigation";
import { usePosShortcuts } from "./hooks/usePosShortcuts";
import { useToasts } from "./hooks/useToasts";
import { useWindowMode } from "./hooks/useWindowMode";
import { loadCardTerminals } from "./lib/cardTerminals";
import { cartTotals, roundMoney } from "./lib/money";
import {
  createSale,
  createAutoBackupIfDue,
  deleteHeldTicket,
  getAppBootstrap,
  getDashboardSummary,
  getProductsByIds,
  listHeldTickets,
  login,
  openCashSession,
  openDrawer,
  createCashMovement,
  printTicket,
  clearActiveSaleDraft,
  getActiveSaleDraft,
  saveActiveSaleDraft,
  saveHeldTicket,
  searchProducts,
  needsInitialSetup,
  setApiActor,
  type ProductSearchOptions,
} from "./lib/posApi";
import type { NavItem, ViewKey } from "./navigation";
import type {
  ActiveSaleDraft,
  CartLine,
  DashboardSummary,
  HeldTicket,
  Product,
  SaleReceipt,
  UserSession,
} from "./types";

const navItems: Array<NavItem<typeof ShoppingCart>> = [
  { key: "sale", label: "Caja", icon: ShoppingCart },
  { key: "products", label: "Productos", icon: Barcode, adminOnly: true, permission: "products" },
  { key: "inventory", label: "Inventario", icon: Boxes, adminOnly: true, permission: "inventory" },
  { key: "customers", label: "Clientes", icon: Users, adminOnly: true, permission: "customers" },
  { key: "reports", label: "Reportes", icon: BarChart3, adminOnly: true, permission: "reports" },
  { key: "cash", label: "Corte", icon: Banknote },
  { key: "purchases", label: "Compras", icon: PackagePlus, adminOnly: true, permission: "purchases" },
  { key: "users", label: "Usuarios", icon: UserPlus, adminOnly: true },
  { key: "settings", label: "Config", icon: Settings, adminOnly: true },
  { key: "administration", label: "Administracion", icon: DatabaseBackup, adminOnly: true },
];

const ACTIVE_DRAFT_SAVE_DELAY_MS = 3000;

function runWhenIdle(action: () => void) {
  const idleWindow = window as Window & {
    requestIdleCallback?: (callback: () => void) => number;
    cancelIdleCallback?: (handle: number) => void;
  };
  if (idleWindow.requestIdleCallback) {
    const handle = idleWindow.requestIdleCallback(action);
    return () => idleWindow.cancelIdleCallback?.(handle);
  }
  const handle = window.setTimeout(action, 500);
  return () => window.clearTimeout(handle);
}

function App() {
  const [session, setSession] = useState<UserSession | null>(null);
  const [setupRequired, setSetupRequired] = useState(false);
  const { notifications, showToast, dismissToast } = useToasts();
  const clock = useClock();
  const { loginMaximized, windowTransition, enterPosMode, enterLoginMode } = useWindowMode(session);
  const [query, setQuery] = useState("");
  const [saleProducts, setSaleProducts] = useState<Product[]>([]);
  const [adminProducts, setAdminProducts] = useState<Product[]>([]);
  const [cart, setCart] = useState<CartLine[]>([]);
  const [cashReceived, setCashReceived] = useState("");
  const [cardReceived, setCardReceived] = useState("");
  const [transferReceived, setTransferReceived] = useState("");
  const [cardTerminals, setCardTerminals] = useState<string[]>([]);
  const [selectedCardTerminal, setSelectedCardTerminal] = useState("");
  const [lastReceipt, setLastReceipt] = useState<SaleReceipt | null>(null);
  const [heldTickets, setHeldTickets] = useState<HeldTicket[]>([]);
  const [activeHeldTicketId, setActiveHeldTicketId] = useState<number | null>(null);
  const [summary, setSummary] = useState<DashboardSummary | null>(null);
  const autoBackupCheckedRef = useRef(false);
  const [busy, setBusy] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [heldTicketsOpen, setHeldTicketsOpen] = useState(false);
  const [ticketNameDraft, setTicketNameDraft] = useState<{ id?: number; name: string } | null>(null);
  const [ticketDeleteDraft, setTicketDeleteDraft] = useState<HeldTicket | null>(null);
  const [expenseOpen, setExpenseOpen] = useState(false);
  const [recoveryDraft, setRecoveryDraft] = useState<ActiveSaleDraft | null>(null);
  const [confirmDraft, setConfirmDraft] = useState<ConfirmDraft | null>(null);
  const [numberPromptDraft, setNumberPromptDraft] = useState<NumberPromptDraft | null>(null);
  const [selectedCartProductId, setSelectedCartProductId] = useState<number | null>(null);
  const [taxEnabled, setTaxEnabled] = useState(true);
  const [pricesIncludeTax, setPricesIncludeTax] = useState(true);
  const searchRef = useRef<HTMLInputElement>(null);
  const cashRef = useRef<HTMLInputElement>(null);
  const recoveryCheckedSessionRef = useRef<number | null>(null);
  const lastSavedActiveDraftRef = useRef("");
  const saleProductSearchRequestRef = useRef(0);
  const adminProductSearchRequestRef = useRef(0);

  const isAdmin = session?.role === "admin";
  const {
    currentView,
    authorizedAdminView,
    pendingAdminView,
    requestView,
    cancelPendingAdminView,
    grantPendingAdminView,
    resetNavigation,
  } = useAdminNavigation({ session, isAdmin, navItems });
  const totals = useMemo(() => cartTotals(cart, pricesIncludeTax, taxEnabled), [cart, pricesIncludeTax, taxEnabled]);
  const cashPaid = Number(cashReceived) || 0;
  const cardPaid = Number(cardReceived) || 0;
  const transferPaid = Number(transferReceived) || 0;
  const paid = cashPaid + cardPaid + transferPaid;
  const nonCashPaid = cardPaid + transferPaid;
  const cashNeeded = roundMoney(Math.max(0, totals.total - nonCashPaid));
  const change = roundMoney(Math.max(0, cashPaid - cashNeeded));
  const shortage = roundMoney(Math.max(0, totals.total - paid));

  const handleLoginSuccess = useCallback(
    async (nextSession: UserSession) => {
      await enterPosMode(() => {
        setApiActor(nextSession);
        setSession(nextSession);
        setSetupRequired(false);
      });
    },
    [enterPosMode],
  );

  useEffect(() => {
    if (session) return;
    needsInitialSetup()
      .then(setSetupRequired)
      .catch((error) => showToast(String(error)));
  }, [session, showToast]);

  const refreshSummary = useCallback(async () => {
    const next = await getDashboardSummary();
    setSummary(next);
  }, []);

  const refreshSaleProducts = useCallback(
    async (nextQuery = query) => {
      const requestId = saleProductSearchRequestRef.current + 1;
      saleProductSearchRequestRef.current = requestId;
      const result = await searchProducts(nextQuery);
      if (requestId !== saleProductSearchRequestRef.current) return;
      setSaleProducts(result);
    },
    [query],
  );

  const refreshAdminProducts = useCallback(async (nextQuery = "", options: ProductSearchOptions = { limit: 51, offset: 0 }) => {
    const requestId = adminProductSearchRequestRef.current + 1;
    adminProductSearchRequestRef.current = requestId;
    const result = await searchProducts(nextQuery, options);
    if (requestId !== adminProductSearchRequestRef.current) return;
    setAdminProducts(result);
  }, []);

  const refreshHeldTickets = useCallback(async () => {
    const result = await listHeldTickets();
    setHeldTickets((current) => {
      if (current.length === 0) return result;
      const byId = new Map(result.map((ticket) => [ticket.id, ticket]));
      const kept = current.flatMap((ticket) => {
        const updated = byId.get(ticket.id);
        if (!updated) return [];
        byId.delete(ticket.id);
        return [updated];
      });
      return [...kept, ...result.filter((ticket) => byId.has(ticket.id))];
    });
  }, []);

  const refreshCardTerminals = useCallback(() => {
    const next = loadCardTerminals();
    setCardTerminals(next);
    setSelectedCardTerminal((current) => (current && next.includes(current) ? current : next[0] ?? ""));
  }, []);

  useEffect(() => {
    if (!session) return;
    refreshCardTerminals();
    const restoreMessage = window.localStorage.getItem("rim-pos-post-restore-message");
    if (restoreMessage) {
      window.localStorage.removeItem("rim-pos-post-restore-message");
      window.setTimeout(() => showToast(restoreMessage), 250);
    }
    getAppBootstrap()
      .then((bootstrap) => {
        setSummary(bootstrap.summary);
        setSaleProducts(bootstrap.products);
        setAdminProducts(bootstrap.products);
        setHeldTickets(bootstrap.held_tickets);
        setTaxEnabled(bootstrap.tax_enabled);
        setPricesIncludeTax(bootstrap.tax_prices_include_tax);
      })
      .catch((error) => showToast(String(error)));
    const cancelIdle = runWhenIdle(() => {
      preloadAdminViews();
      if (!autoBackupCheckedRef.current) {
        autoBackupCheckedRef.current = true;
        createAutoBackupIfDue()
          .then((result) => {
            if (result) showToast("Backup automatico creado");
          })
          .catch((error) => showToast(`Backup automatico fallo: ${String(error)}`));
      }
    });
    window.setTimeout(() => searchRef.current?.focus(), 50);
    return cancelIdle;
  }, [refreshCardTerminals, session, showToast]);

  useEffect(() => {
    if (!session) return;
    const refresh = () => refreshCardTerminals();
    window.addEventListener("focus", refresh);
    window.addEventListener("rim-pos-card-terminals-updated", refresh);
    return () => {
      window.removeEventListener("focus", refresh);
      window.removeEventListener("rim-pos-card-terminals-updated", refresh);
    };
  }, [refreshCardTerminals, session]);

  useEffect(() => {
    if (!session || !summary) return;
    if (recoveryCheckedSessionRef.current === session.id) return;
    recoveryCheckedSessionRef.current = session.id;
    lastSavedActiveDraftRef.current = "";
    getActiveSaleDraft(session.id, summary.open_cash_session?.id ?? null)
      .then((draft) => {
        if (draft?.items.length) {
          setRecoveryDraft(draft);
        }
      })
      .catch((error) => showToast(String(error)));
  }, [session, showToast, summary]);

  useEffect(() => {
    if (currentView === "sale") {
      refreshCardTerminals();
      window.setTimeout(() => searchRef.current?.focus(), 40);
    }
  }, [currentView, refreshCardTerminals]);

  useEffect(() => {
    if (!session) return;
    if (!["products", "inventory", "purchases"].includes(currentView)) return;
    refreshAdminProducts("", { limit: 51, offset: 0 }).catch((error) => showToast(String(error)));
  }, [currentView, refreshAdminProducts, session, showToast]);

  const addProduct = useCallback((product: Product, quantity = 1) => {
    if (quantity <= 0) return;
    const existing = cart.find((line) => line.product.id === product.id);
    const nextQuantity = (existing?.quantity ?? 0) + quantity;
    if (product.stock <= 0) {
      showToast(`${product.name} sin existencia`);
      return;
    }
    if (nextQuantity > product.stock) {
      showToast(`${product.name} solo tiene ${product.stock} ${product.unit}`);
      return;
    }
    setCart((current) => {
      if (existing) {
        return current.map((line) =>
          line.product.id === product.id ? { ...line, product, quantity: nextQuantity } : line,
        );
      }
      return [...current, { product, quantity, discount: 0 }];
    });
    setSelectedCartProductId(product.id);
    setQuery("");
    searchRef.current?.focus();
  }, [cart, showToast]);

  const updateLine = useCallback((productId: number, patch: Partial<Pick<CartLine, "quantity" | "discount">>) => {
    setCart((current) =>
      current
        .map((line) => {
          if (line.product.id !== productId) return line;
          const nextQuantity = patch.quantity ?? line.quantity;
          if (nextQuantity > line.product.stock) {
            showToast(`${line.product.name} solo tiene ${line.product.stock} ${line.product.unit}`);
            return line;
          }
          return { ...line, ...patch };
        })
        .filter((line) => line.quantity > 0),
    );
  }, [showToast]);

  const applyWholesaleToSelectedLine = useCallback(() => {
    const line = selectedCartProductId
      ? cart.find((item) => item.product.id === selectedCartProductId)
      : cart[cart.length - 1];
    if (!line) {
      showToast("No hay producto para mayoreo");
      return;
    }
    const wholesalePrice = line.product.wholesale_price ?? 0;
    if (wholesalePrice <= 0 || wholesalePrice >= line.product.price) {
      showToast(`${line.product.name} sin precio de mayoreo menor`);
      return;
    }
    const discount = roundMoney((line.product.price - wholesalePrice) * line.quantity);
    updateLine(line.product.id, { discount });
    showToast(`Mayoreo aplicado: ${line.product.name}`);
  }, [cart, selectedCartProductId, showToast, updateLine]);

  useEffect(() => {
    setSelectedCartProductId((current) => {
      if (cart.length === 0) return null;
      if (current && cart.some((line) => line.product.id === current)) return current;
      return cart[cart.length - 1].product.id;
    });
  }, [cart]);

  const submitSearch = async (event?: FormEvent) => {
    event?.preventDefault();
    const requestId = saleProductSearchRequestRef.current + 1;
    saleProductSearchRequestRef.current = requestId;
    const result = await searchProducts(query);
    if (requestId !== saleProductSearchRequestRef.current) return;
    setSaleProducts(result);
    if (query.trim() && result.length === 1) addProduct(result[0]);
  };

  const completeSale = async (options: { printTicket?: boolean } = {}) => {
    if (!session) return;
    if (cart.length === 0) {
      showToast("Agrega articulos");
      return;
    }
    if (!summary?.open_cash_session) {
      showToast("Abre caja antes de vender");
      return;
    }
    if (paid < totals.total) {
      cashRef.current?.focus();
      showToast("Pago insuficiente");
      return;
    }
    if (nonCashPaid > totals.total) {
      showToast("Tarjeta/credito no puede exceder total");
      return;
    }
    if (cardPaid > 0 && !selectedCardTerminal) {
      showToast("Selecciona terminal de tarjeta");
      return;
    }
    setBusy(true);
    try {
      const currentProducts = await getProductsByIds(cart.map((line) => line.product.id));
      const byId = new Map(currentProducts.map((product) => [product.id, product]));
      const unavailableLine = cart.find((line) => !byId.has(line.product.id));
      if (unavailableLine) {
        showToast(`${unavailableLine.product.name} no disponible`);
        return;
      }
      const syncedCart = cart.map((line) => ({ ...line, product: byId.get(line.product.id) ?? line.product }));
      const missingStock = syncedCart.find((line) => line.product.stock <= 0 || line.quantity > line.product.stock);
      if (missingStock) {
        setCart(syncedCart);
        showToast(`${missingStock.product.name} sin existencia suficiente`);
        return;
      }
      const receipt = await createSale({
        cashier_id: session.id,
        customer_id: null,
        items: syncedCart.map((line) => ({
          product_id: line.product.id,
          quantity: line.quantity,
          unit_price: line.product.price,
          discount: line.discount,
        })),
        payments: [
          ...(cashPaid > 0 ? [{ method: "cash" as const, amount: cashPaid }] : []),
          ...(cardPaid > 0 ? [{ method: "card" as const, amount: cardPaid, reference: selectedCardTerminal }] : []),
          ...(transferPaid > 0 ? [{ method: "transfer" as const, amount: transferPaid }] : []),
        ],
      });
      setLastReceipt(receipt);
      setCart([]);
      setSelectedCartProductId(null);
      setCashReceived("");
      setCardReceived("");
      setTransferReceived("");
      if (activeHeldTicketId) {
        await deleteHeldTicket(activeHeldTicketId);
        setActiveHeldTicketId(null);
        await refreshHeldTickets();
      }
      await clearActiveDraftForSession();
      const hardwareWarnings: string[] = [];
      if (options.printTicket !== false) {
        try {
          await printTicket(receipt.sale_id);
        } catch (error) {
          hardwareWarnings.push(`Ticket no impreso: ${String(error)}`);
        }
      }
      try {
        await openDrawer();
      } catch (error) {
        hardwareWarnings.push(`Cajon no abrio: ${String(error)}`);
      }
      await refreshSummary();
      showToast(hardwareWarnings.length ? `Venta ${receipt.folio} cobrada. ${hardwareWarnings.join(" ")}` : `Venta ${receipt.folio} cobrada`);
      searchRef.current?.focus();
    } catch (error) {
      showToast(String(error));
    } finally {
      setBusy(false);
    }
  };

  const recordExpense = async (provider: string, amount: number) => {
    if (!session) return;
    const cashSessionId = summary?.open_cash_session?.id;
    if (!cashSessionId) {
      showToast("Abre caja primero");
      return;
    }
    try {
      await createCashMovement({
        session_id: cashSessionId,
        movement_type: "out",
        amount,
        reason: `Gasto: ${provider}`,
        actor_id: session.id,
      });
      setExpenseOpen(false);
      await refreshSummary();
      showToast("Gasto registrado");
    } catch (error) {
      showToast(String(error));
    }
  };

  const openDrawerAndRecord = async () => {
    if (!session) return;
    const cashSessionId = summary?.open_cash_session?.id;
    if (!cashSessionId) {
      showToast("Abre caja primero para registrar cajon");
      return;
    }
    try {
      const result = await openDrawer();
      await createCashMovement({
        session_id: cashSessionId,
        movement_type: "drawer",
        amount: 0,
        reason: "Apertura manual de cajon",
        actor_id: session.id,
      });
      await refreshSummary();
      showToast(result.message);
    } catch (error) {
      showToast(String(error));
    }
  };

  const cartToHeldItems = useCallback(
    () =>
      cart.map((line) => ({
        product_id: line.product.id,
        quantity: line.quantity,
        unit_price: line.product.price,
        discount: line.discount,
        tax_rate: line.product.tax_rate,
      })),
    [cart],
  );

  const clearActiveDraftForSession = useCallback(async () => {
    if (!session) return;
    lastSavedActiveDraftRef.current = "";
    await clearActiveSaleDraft(session.id);
  }, [session]);

  useEffect(() => {
    if (!session) return;
    if (recoveryCheckedSessionRef.current !== session.id) return;
    if (recoveryDraft) return;
    if (activeHeldTicketId) return;
    if (cart.length === 0) {
      if (!lastSavedActiveDraftRef.current) return;
      const timer = window.setTimeout(() => {
        clearActiveSaleDraft(session.id)
          .then(() => {
            lastSavedActiveDraftRef.current = "";
          })
          .catch((error) => showToast(`Borrador no limpiado: ${String(error)}`));
      }, 300);
      return () => window.clearTimeout(timer);
    }

    const items = cartToHeldItems();
    const signature = JSON.stringify({
      cash_session_id: summary?.open_cash_session?.id ?? null,
      items,
      cash_received: cashPaid,
      card_received: cardPaid,
      transfer_received: transferPaid,
    });
    if (signature === lastSavedActiveDraftRef.current) return;

    const timer = window.setTimeout(() => {
      saveActiveSaleDraft({
        cashier_id: session.id,
        cash_session_id: summary?.open_cash_session?.id ?? null,
        items,
        cash_received: cashPaid,
        card_received: cardPaid,
        transfer_received: transferPaid,
      })
        .then(() => {
          lastSavedActiveDraftRef.current = signature;
        })
        .catch((error) => showToast(`Borrador no guardado: ${String(error)}`));
    }, ACTIVE_DRAFT_SAVE_DELAY_MS);

    return () => window.clearTimeout(timer);
  }, [
    activeHeldTicketId,
    cardPaid,
    cart.length,
    cartToHeldItems,
    cashPaid,
    recoveryDraft,
    session,
    showToast,
    summary?.open_cash_session?.id,
    transferPaid,
  ]);

  const persistActiveHeldTicket = useCallback(async () => {
    if (!session || !activeHeldTicketId || cart.length === 0) return;
    const current = heldTickets.find((ticket) => ticket.id === activeHeldTicketId);
    if (!current) return;
    await saveHeldTicket({
      id: current.id,
      name: current.name,
      cashier_id: session.id,
      items: cartToHeldItems(),
    });
    await refreshHeldTickets();
  }, [activeHeldTicketId, cart.length, cartToHeldItems, heldTickets, refreshHeldTickets, session]);

  const clearSaleDraft = useCallback(() => {
    setActiveHeldTicketId(null);
    setSelectedCartProductId(null);
    setCart([]);
    setCashReceived("");
    setCardReceived("");
    setTransferReceived("");
    setQuery("");
  }, []);

  const openHoldTicketDialog = () => {
    if (!session) return;
    if (cart.length === 0) {
      showToast("No hay articulos para dejar abierto");
      return;
    }
    const current = activeHeldTicketId ? heldTickets.find((ticket) => ticket.id === activeHeldTicketId) : null;
    setTicketNameDraft({ id: current?.id, name: current?.name ?? "" });
  };

  const saveOpenTicket = async (name: string, ticketId?: number) => {
    if (!session) return;
    if (name.length < 2) {
      showToast("Nombre de ticket requerido");
      return;
    }
    const current = ticketId ? heldTickets.find((ticket) => ticket.id === ticketId) : null;
    try {
      const ticket = await saveHeldTicket({
        id: current?.id,
        name,
        cashier_id: session.id,
        items: cartToHeldItems(),
      });
      setCart([]);
      setActiveHeldTicketId(null);
      setSelectedCartProductId(null);
      setCashReceived("");
      setCardReceived("");
      setTransferReceived("");
      setQuery("");
      setTicketNameDraft(null);
      await clearActiveDraftForSession();
      await refreshHeldTickets();
      showToast(`Ticket abierto: ${ticket.name}`);
      searchRef.current?.focus();
    } catch (error) {
      showToast(String(error));
    }
  };

  const recoverHeldTicket = async (ticket: HeldTicket, forceReplace = false) => {
    if (activeHeldTicketId === ticket.id) {
      showToast(`Viendo ${ticket.name}`);
      return;
    }
    if (cart.length > 0 && !activeHeldTicketId && !forceReplace) {
      setConfirmDraft({
        title: "Abrir ticket",
        message: "Venta actual se reemplaza. Si quieres conservarla, usa Dejar abierto primero.",
        confirmLabel: "Abrir ticket",
        tone: "warning",
        onConfirm: () => recoverHeldTicket(ticket, true),
      });
      return;
    }
    try {
      await persistActiveHeldTicket();
      const catalog = await getProductsByIds(ticket.items.map((item) => item.product_id));
      const byId = new Map(catalog.map((product) => [product.id, product]));
      const nextCart = ticket.items.map((item) => {
        const product = byId.get(item.product_id);
        if (!product) {
          throw new Error(`Producto no disponible: ${item.product_id}`);
        }
        return { product: { ...product, price: item.unit_price, tax_rate: item.tax_rate }, quantity: item.quantity, discount: item.discount };
      });
      setCart(nextCart);
      setSelectedCartProductId(nextCart[0]?.product.id ?? null);
      setCashReceived("");
      setCardReceived("");
      setTransferReceived("");
      setActiveHeldTicketId(ticket.id);
      await clearActiveDraftForSession();
      setHeldTicketsOpen(false);
      showToast(`Viendo ${ticket.name}`);
      searchRef.current?.focus();
    } catch (error) {
      showToast(String(error));
    }
  };

  const recoverActiveSaleDraft = async () => {
    if (!recoveryDraft) return;
    try {
      const catalog = await getProductsByIds(recoveryDraft.items.map((item) => item.product_id));
      const byId = new Map(catalog.map((product) => [product.id, product]));
      const nextCart = recoveryDraft.items.map((item) => {
        const product = byId.get(item.product_id);
        if (!product) {
          throw new Error(`Producto no disponible: ${item.product_id}`);
        }
        return { product: { ...product, price: item.unit_price, tax_rate: item.tax_rate }, quantity: item.quantity, discount: item.discount };
      });
      setCart(nextCart);
      setSelectedCartProductId(nextCart[0]?.product.id ?? null);
      setCashReceived(recoveryDraft.cash_received > 0 ? String(recoveryDraft.cash_received) : "");
      setCardReceived(recoveryDraft.card_received > 0 ? String(recoveryDraft.card_received) : "");
      setTransferReceived(recoveryDraft.transfer_received > 0 ? String(recoveryDraft.transfer_received) : "");
      setActiveHeldTicketId(null);
      lastSavedActiveDraftRef.current = "recovered";
      setRecoveryDraft(null);
      requestView("sale");
      showToast("Venta pendiente recuperada");
      searchRef.current?.focus();
    } catch (error) {
      showToast(String(error));
    }
  };

  const discardActiveSaleDraft = async () => {
    try {
      await clearActiveDraftForSession();
      setRecoveryDraft(null);
      showToast("Venta pendiente descartada");
      searchRef.current?.focus();
    } catch (error) {
      showToast(String(error));
    }
  };

  const removeHeldTicket = async (ticket: HeldTicket) => {
    try {
      await deleteHeldTicket(ticket.id);
      if (activeHeldTicketId === ticket.id) {
        setActiveHeldTicketId(null);
        setCart([]);
        setSelectedCartProductId(null);
      }
      setTicketDeleteDraft(null);
      await refreshHeldTickets();
      showToast("Ticket abierto eliminado");
    } catch (error) {
      showToast(String(error));
    }
  };

  const startNewTicket = useCallback(async (forceClear = false) => {
    if (!session) return;
    if (!activeHeldTicketId && cart.length > 0 && !forceClear) {
      setConfirmDraft({
        title: "Nuevo ticket",
        message: "Venta actual se limpia. Si necesitas guardarla, usa Dejar abierto.",
        confirmLabel: "Crear nuevo",
        tone: "warning",
        onConfirm: () => startNewTicket(true),
      });
      return;
    }
    try {
      await persistActiveHeldTicket();
      clearSaleDraft();
      await clearActiveDraftForSession();
      showToast("Nuevo ticket listo");
      searchRef.current?.focus();
    } catch (error) {
      showToast(String(error));
    }
  }, [activeHeldTicketId, cart.length, clearActiveDraftForSession, clearSaleDraft, persistActiveHeldTicket, session, showToast]);

  const runFunctionKeyAction = useCallback(async (key: string) => {
    if (!session) return;
    if (key === "F1") {
      await completeSale({ printTicket: true });
      return;
    }
    if (key === "F2") {
      await completeSale({ printTicket: false });
      return;
    }
    if (key === "F3") {
      requestView("products");
      return;
    }
    if (key === "F4") {
      requestView("inventory");
      return;
    }
    if (key === "F5") {
      requestView("customers");
      return;
    }
    if (key === "F6") {
      await openHoldTicketDialog();
      return;
    }
    if (key === "F7") {
      const line = selectedCartProductId ? cart.find((item) => item.product.id === selectedCartProductId) : null;
      if (!line) {
        showToast("No hay producto para quitar");
        return;
      }
      updateLine(line.product.id, { quantity: 0 });
      showToast("Producto quitado");
      return;
    }
    if (key === "F8") {
      setExpenseOpen(true);
      return;
    }
    if (key === "F9") {
      cashRef.current?.focus();
      return;
    }
    if (key === "F10") {
      await openDrawerAndRecord();
      return;
    }
    if (key === "F11") {
      applyWholesaleToSelectedLine();
      return;
    }
    if (key === "F12") {
      requestView("settings");
    }
  }, [
    cart,
    cashRef,
    completeSale,
    applyWholesaleToSelectedLine,
    openDrawerAndRecord,
    openHoldTicketDialog,
    requestView,
    selectedCartProductId,
    session,
    showToast,
    updateLine,
  ]);

  usePosShortcuts({
    session,
    currentView,
    cart,
    selectedCartProductId,
    searchRef,
    cashRef,
    requestView,
    updateLine,
    completeSale,
    holdCurrentTicket: openHoldTicketDialog,
    openExpenseDialog: () => setExpenseOpen(true),
    openDrawer: openDrawerAndRecord,
    applyWholesaleToSelectedLine,
    setSelectedCartProductId,
    setQuery,
    setProducts: setSaleProducts,
    showToast,
  });

  const openCash = async (openingCash = 0) => {
    if (!session) return;
    try {
      await openCashSession(openingCash, session.id);
      await refreshSummary();
      showToast("Caja abierta");
    } catch (error) {
      showToast(String(error));
    }
  };

  const logout = async () => {
    await enterLoginMode(async () => {
      try {
        await clearActiveDraftForSession();
      } catch (error) {
        console.warn("Active sale draft clear failed", error);
      }
      setCart([]);
      setCashReceived("");
      setCardReceived("");
      setTransferReceived("");
      resetNavigation();
      setSession(null);
    });
    setApiActor(null);
  };

  if (!session) {
    return (
      <div className="chrome-shell login-chrome">
        <WindowTitlebar
          clock={clock}
          roleLabel="Acceso"
        />
        <LoginScreen onLogin={handleLoginSuccess} setupRequired={setupRequired} showToast={showToast} maximized={loginMaximized} />
        <ToastStack notifications={notifications} onDismiss={dismissToast} />
        {windowTransition !== "idle" && <WindowTransitionCover phase={windowTransition} />}
      </div>
    );
  }

  return (
    <div className="chrome-shell app-chrome">
      <AppShell
        clock={clock}
        session={session}
        summary={summary}
        currentView={currentView}
        isAdmin={isAdmin}
        authorizedAdminView={authorizedAdminView}
        navItems={navItems}
        requestView={requestView}
        logout={logout}
      >
        <ErrorBoundary resetKey={currentView}>
          {currentView === "sale" ? (
            <SaleView
              query={query}
              products={saleProducts}
              cart={cart}
              totals={totals}
              paid={paid}
              shortage={shortage}
              change={change}
              cashReceived={cashReceived}
              cardReceived={cardReceived}
              transferReceived={transferReceived}
              cardTerminals={cardTerminals}
              selectedCardTerminal={selectedCardTerminal}
              lastReceipt={lastReceipt}
              heldTickets={heldTickets}
              activeHeldTicketId={activeHeldTicketId}
              selectedCartProductId={selectedCartProductId}
              busy={busy}
              hasOpenCash={Boolean(summary?.open_cash_session)}
              searchRef={searchRef}
              cashRef={cashRef}
              setQuery={setQuery}
              setCashReceived={setCashReceived}
              setCardReceived={setCardReceived}
              setTransferReceived={setTransferReceived}
              setSelectedCardTerminal={setSelectedCardTerminal}
              refreshProducts={refreshSaleProducts}
              submitSearch={submitSearch}
              addProduct={addProduct}
              updateLine={updateLine}
              selectCartLine={setSelectedCartProductId}
              completeSale={completeSale}
              holdCurrentTicket={openHoldTicketDialog}
              newTicket={() => startNewTicket()}
              recoverHeldTicket={recoverHeldTicket}
              removeHeldTicket={(ticket) => {
                setTicketDeleteDraft(ticket);
                return Promise.resolve();
              }}
              runFunctionKeyAction={runFunctionKeyAction}
              showToast={showToast}
              openHeldTickets={() => setHeldTicketsOpen(true)}
            />
          ) : (
            <AdminView
              view={currentView}
              session={session}
              products={adminProducts}
              summary={summary}
              openCash={openCash}
              refreshProducts={refreshAdminProducts}
              refreshSummary={refreshSummary}
              showToast={showToast}
              onTaxModeChange={({ enabled, pricesIncludeTax: nextPricesIncludeTax }) => {
                setTaxEnabled(enabled);
                setPricesIncludeTax(nextPricesIncludeTax);
              }}
              requestConfirm={setConfirmDraft}
              requestView={requestView}
            />
          )}
        </ErrorBoundary>
      </AppShell>

      <ToastStack notifications={notifications} onDismiss={dismissToast} />
        {heldTicketsOpen && (
          <HeldTicketsModal
            tickets={heldTickets}
            onClose={() => setHeldTicketsOpen(false)}
            onRecover={recoverHeldTicket}
            onDelete={(ticket) => setTicketDeleteDraft(ticket)}
          />
        )}
        {ticketNameDraft && (
          <TicketNameModal
            initialName={ticketNameDraft.name}
            onClose={() => setTicketNameDraft(null)}
            onSave={(name) => saveOpenTicket(name.trim(), ticketNameDraft.id)}
          />
        )}
        {ticketDeleteDraft && (
          <DeleteHeldTicketModal
            ticket={ticketDeleteDraft}
            onCancel={() => setTicketDeleteDraft(null)}
            onConfirm={() => removeHeldTicket(ticketDeleteDraft)}
          />
        )}
        {recoveryDraft && (
          <RecoveryDraftModal
            draft={recoveryDraft}
            onRecover={recoverActiveSaleDraft}
            onDiscard={discardActiveSaleDraft}
          />
        )}
      {confirmDraft && (
        <ConfirmActionModal
          draft={confirmDraft}
          onCancel={() => setConfirmDraft(null)}
          onConfirm={async () => {
            const action = confirmDraft.onConfirm;
            setConfirmDraft(null);
            await action();
          }}
        />
      )}
      {numberPromptDraft && (
        <NumberPromptModal
          draft={numberPromptDraft}
          onCancel={() => setNumberPromptDraft(null)}
          onConfirm={async (value) => {
            const action = numberPromptDraft.onConfirm;
            setNumberPromptDraft(null);
            await action(value);
          }}
        />
      )}
      {expenseOpen && (
        <ExpenseDialog
          onClose={() => setExpenseOpen(false)}
          onSave={recordExpense}
        />
      )}
      {pendingAdminView && (
        <AdminGate
          targetLabel={navItems.find((item) => item.key === pendingAdminView)?.label ?? "Admin"}
          onCancel={cancelPendingAdminView}
          onSuccess={(adminSession) => {
            grantPendingAdminView(adminSession);
            showToast("Acceso autorizado solo para esta opcion");
          }}
          showToast={showToast}
        />
      )}
      {helpOpen && <ShortcutHelp onClose={() => setHelpOpen(false)} />}
      {windowTransition !== "idle" && <WindowTransitionCover phase={windowTransition} />}
    </div>
  );
}

export default App;
