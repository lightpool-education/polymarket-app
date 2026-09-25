import type { BookLevel, BookResponse, MarketInfo, Order } from "@/lib/types";

const CLOB_INDEX_URL =
  process.env.NEXT_PUBLIC_CLOB_INDEX_URL ?? "http://127.0.0.1:3002";
const CLOB_INDEX_WS_URL =
  process.env.NEXT_PUBLIC_CLOB_INDEX_WS_URL ?? "ws://127.0.0.1:3002";

export type OrderBookDelta = {
  type: "orderbook_delta";
  spot_market: string;
  sequence: number;
  block_num: number;
  bids: BookLevel[];
  asks: BookLevel[];
  last_trade_price?: string | null;
};

type OrderBookSnapshot = BookResponse & {
  type: "orderbook_snapshot";
  spot_market: string;
};

type UserOrderWsMessage = {
  type: "order";
  event: string;
  user_address: string;
  chain_order_id: string;
  block_num: number;
  id: string;
  market_id: string;
  market_slug: string;
  question: string;
  outcome: string;
  side: string;
  price: string;
  size: string;
  status: string;
  spot_market?: string;
};

type UserTradeWsMessage = {
  type: "trade";
  user_address: string;
  chain_order_id: string;
  order_id: string;
  market_slug: string;
  outcome: string;
  side: string;
  price: string;
  fill_amount: string;
  remaining_amount: string;
  is_fully_filled: boolean;
  spot_market: string;
  block_num: number;
};

const OPEN_STATUSES = new Set(["open", "partial_filled"]);
const HISTORY_STATUSES = new Set([
  "filled",
  "cancelled",
  "canceled",
  "rejected",
  "failed",
]);

export function isOpenStatus(status: string): boolean {
  return OPEN_STATUSES.has(status.trim().toLowerCase());
}

export function isHistoryStatus(status: string): boolean {
  return HISTORY_STATUSES.has(status.trim().toLowerCase());
}

function spotMarketPath(spotMarket: string): string {
  return encodeURIComponent(spotMarket);
}

function comparePriceDesc(a: string, b: string): number {
  return Number.parseFloat(b) - Number.parseFloat(a);
}

function comparePriceAsc(a: string, b: string): number {
  return Number.parseFloat(a) - Number.parseFloat(b);
}

function applyLevelChanges(
  levels: BookLevel[],
  changes: BookLevel[],
  sort: (a: string, b: string) => number,
): BookLevel[] {
  const map = new Map(levels.map((level) => [level.price, level.size]));
  for (const change of changes) {
    const sizeValue = Number.parseFloat(change.size);
    if (!Number.isFinite(sizeValue) || sizeValue === 0) {
      map.delete(change.price);
    } else {
      map.set(change.price, change.size);
    }
  }
  return Array.from(map.entries())
    .map(([price, size]) => ({ price, size }))
    .sort((a, b) => sort(a.price, b.price));
}

function trimLevels(levels: BookLevel[], depth: number): BookLevel[] {
  if (depth <= 0 || levels.length <= depth) {
    return levels;
  }
  return levels.slice(0, depth);
}

export function applyOrderBookDelta(
  book: BookResponse,
  delta: OrderBookDelta,
  depth = 10,
): BookResponse {
  return {
    sequence: delta.sequence,
    bids: trimLevels(
      applyLevelChanges(book.bids, delta.bids, comparePriceDesc),
      depth,
    ),
    asks: trimLevels(
      applyLevelChanges(book.asks, delta.asks, comparePriceAsc),
      depth,
    ),
    last_trade_price: delta.last_trade_price ?? book.last_trade_price,
  };
}

export async function fetchBooksBySpotMarket(
  spotMarkets: string[],
  depth = 1,
): Promise<Map<string, BookResponse>> {
  const uniqueMarkets = [...new Set(spotMarkets.filter((market) => market.trim().length > 0))];
  const books = new Map<string, BookResponse>();

  await Promise.all(
    uniqueMarkets.map(async (spotMarket) => {
      try {
        const book = await fetchBookSnapshot(spotMarket, depth);
        books.set(spotMarket, book);
      } catch {
        // Skip unavailable books; portfolio valuation falls back for that market.
      }
    }),
  );

  return books;
}

function orderFromUserMessage(message: UserOrderWsMessage): Order {
  return {
    id: message.id,
    market_id: message.market_id,
    market_slug: message.market_slug,
    question: message.question,
    outcome: message.outcome,
    side: message.side,
    price: message.price,
    size: message.size,
    status: message.status,
    chain_order_id: message.chain_order_id,
    spot_market: message.spot_market,
    user_address: message.user_address,
  };
}

export function upsertOrder(orders: Order[], order: Order): Order[] {
  const index = orders.findIndex(
    (item) =>
      item.id === order.id ||
      (item.chain_order_id &&
        order.chain_order_id &&
        item.chain_order_id === order.chain_order_id &&
        (item.spot_market ?? "") === (order.spot_market ?? "")),
  );
  if (index >= 0) {
    return orders.map((item, itemIndex) =>
      itemIndex === index ? { ...item, ...order } : item,
    );
  }
  return [order, ...orders];
}

export function applyUserOrderEvent(
  orders: Order[],
  message: UserOrderWsMessage,
): Order[] {
  return upsertOrder(orders, orderFromUserMessage(message));
}

export function applyUserTradeEvent(
  orders: Order[],
  message: UserTradeWsMessage,
): Order[] {
  const existing = orders.find((order) => order.id === message.order_id);
  if (existing) {
    return orders.map((order) => {
      if (order.id !== message.order_id) {
        return order;
      }
      return {
        ...order,
        size: message.remaining_amount,
        status: message.is_fully_filled ? "filled" : "partial_filled",
        price: message.price || order.price,
        spot_market: message.spot_market || order.spot_market,
      };
    });
  }
  // Market fills often skip order_created; seed a history row from the trade WS.
  return upsertOrder(orders, {
    id: message.order_id,
    market_id: "",
    market_slug: message.market_slug ?? "",
    question: "",
    outcome: message.outcome ?? "",
    side: message.side ?? "—",
    price: message.price ?? "—",
    size: message.fill_amount,
    status: message.is_fully_filled ? "filled" : "partial_filled",
    chain_order_id: message.chain_order_id,
    spot_market: message.spot_market,
    user_address: message.user_address,
  });
}

async function parseOrdersResponse(res: Response): Promise<Order[]> {
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(
      (body as { error?: string }).error ?? `Failed to load orders: ${res.status}`,
    );
  }
  return res.json() as Promise<Order[]>;
}

/** Live open / partial orders (hot index). */
export async function fetchOpenOrders(userAddress: string): Promise<Order[]> {
  const q = `user_address=${encodeURIComponent(userAddress)}`;
  const res = await fetch(`${CLOB_INDEX_URL}/api/orders/openOrders?${q}`, {
    cache: "no-store",
  });
  if (res.status === 404) {
    const fallback = await fetch(`${CLOB_INDEX_URL}/api/orders?${q}`, {
      cache: "no-store",
    });
    return parseOrdersResponse(fallback);
  }
  return parseOrdersResponse(res);
}

/** Terminal orders from sqlite history (filled / cancelled / …). */
export async function fetchHistoricalOrders(
  userAddress: string,
): Promise<Order[]> {
  const q = `user_address=${encodeURIComponent(userAddress)}`;
  const res = await fetch(
    `${CLOB_INDEX_URL}/api/orders/historicalOrders?${q}`,
    { cache: "no-store" },
  );
  return parseOrdersResponse(res);
}

/** Open + historical orders for portfolio / market detail. */
export async function fetchUserOrders(userAddress: string): Promise<Order[]> {
  const [open, historical] = await Promise.all([
    fetchOpenOrders(userAddress),
    fetchHistoricalOrders(userAddress),
  ]);
  const byId = new Map<string, Order>();
  for (const order of historical) {
    byId.set(order.id, order);
  }
  for (const order of open) {
    byId.set(order.id, order);
  }
  return Array.from(byId.values());
}

export async function fetchMarketInfo(
  spotMarket: string,
  account: string,
): Promise<MarketInfo> {
  const res = await fetch(
    `${CLOB_INDEX_URL}/api/spot/${spotMarketPath(spotMarket)}/info?account=${encodeURIComponent(account)}`,
    { cache: "no-store" },
  );

  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error ?? `Failed to load market info: ${res.status}`);
  }

  return res.json() as Promise<MarketInfo>;
}

export async function fetchBookSnapshot(
  spotMarket: string,
  depth = 10,
): Promise<BookResponse> {
  const res = await fetch(
    `${CLOB_INDEX_URL}/api/spot/${spotMarketPath(spotMarket)}/book?depth=${depth}`,
    { cache: "no-store" },
  );

  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error ?? `Failed to load order book: ${res.status}`);
  }

  return res.json() as Promise<BookResponse>;
}

export function createOrderBookSubscription(
  spotMarket: string,
  depth: number,
  handlers: {
    onBook: (book: BookResponse) => void;
    onError?: (error: Error) => void;
  },
): () => void {
  let closed = false;
  let currentBook: BookResponse | null = null;
  let socket: WebSocket | null = null;
  let pending = false;
  let rafId: number | null = null;

  function scheduleRender() {
    if (pending || closed || !currentBook) {
      return;
    }
    pending = true;
    rafId = window.requestAnimationFrame(() => {
      pending = false;
      if (!closed && currentBook) {
        handlers.onBook(currentBook);
      }
    });
  }

  async function start() {
    if (closed) {
      return;
    }

    socket = new WebSocket(`${CLOB_INDEX_WS_URL}/api/ws`);
    socket.onopen = () => {
      socket?.send(
        JSON.stringify({
          op: "subscribe",
          channel: "orderbook_delta",
          spot_market: spotMarket,
          depth,
        }),
      );
    };
    socket.onmessage = (event) => {
      try {
        const payload = JSON.parse(event.data as string) as
          | OrderBookSnapshot
          | OrderBookDelta
          | { type: "error"; error: string };

        if (payload.type === "error") {
          handlers.onError?.(new Error(payload.error));
          return;
        }

        if (payload.type === "orderbook_snapshot") {
          currentBook = {
            sequence: payload.sequence,
            bids: trimLevels(payload.bids, depth),
            asks: trimLevels(payload.asks, depth),
            last_trade_price: payload.last_trade_price,
          };
          scheduleRender();
          return;
        }

        if (payload.type === "orderbook_delta") {
          if (!currentBook) {
            currentBook = {
              sequence: payload.sequence,
              bids: [],
              asks: [],
              last_trade_price: payload.last_trade_price ?? null,
            };
          }
          currentBook = applyOrderBookDelta(currentBook, payload, depth);
          scheduleRender();
        }
      } catch (error) {
        handlers.onError?.(
          error instanceof Error ? error : new Error("Invalid order book message"),
        );
      }
    };
    socket.onerror = () => {
      handlers.onError?.(new Error("Order book websocket error"));
    };
  }

  void start();

  return () => {
    closed = true;
    if (rafId !== null) {
      window.cancelAnimationFrame(rafId);
    }
    if (socket && socket.readyState <= WebSocket.OPEN) {
      try {
        socket.send(
          JSON.stringify({
            op: "unsubscribe",
            channel: "orderbook_delta",
            spot_market: spotMarket,
          }),
        );
      } catch {
        // ignore cleanup errors
      }
      socket.close();
    }
    socket = null;
  };
}

export function createUserOrdersSubscription(
  userAddress: string,
  handlers: {
    onOrders: (orders: Order[]) => void;
    onError?: (error: Error) => void;
  },
): () => void {
  let closed = false;
  let currentOrders: Order[] = [];
  let socket: WebSocket | null = null;
  let pending = false;
  let rafId: number | null = null;

  function scheduleRender() {
    if (pending || closed) {
      return;
    }
    pending = true;
    rafId = window.requestAnimationFrame(() => {
      pending = false;
      if (!closed) {
        handlers.onOrders(currentOrders);
      }
    });
  }

  async function syncOrders() {
    try {
      currentOrders = await fetchUserOrders(userAddress);
      scheduleRender();
    } catch (error) {
      handlers.onError?.(
        error instanceof Error ? error : new Error("Failed to load orders"),
      );
    }
  }

  async function start() {
    void syncOrders();

    if (closed) {
      return;
    }

    socket = new WebSocket(`${CLOB_INDEX_WS_URL}/api/ws`);
    socket.onopen = () => {
      socket?.send(
        JSON.stringify({
          op: "subscribe",
          channel: "user",
          user_address: userAddress,
        }),
      );
    };
    socket.onmessage = (event) => {
      try {
        const payload = JSON.parse(event.data as string) as
          | UserOrderWsMessage
          | UserTradeWsMessage
          | { type: "error"; error: string }
          | { type: "subscribed" | "unsubscribed"; channel?: string };

        if (payload.type === "error") {
          handlers.onError?.(new Error(payload.error));
          return;
        }

        if (payload.type === "subscribed") {
          if (payload.channel === "user") {
            void syncOrders();
          }
          return;
        }

        if (payload.type === "unsubscribed") {
          return;
        }

        if (payload.type === "order") {
          currentOrders = applyUserOrderEvent(currentOrders, payload);
          scheduleRender();
          return;
        }

        if (payload.type === "trade") {
          currentOrders = applyUserTradeEvent(currentOrders, payload);
          scheduleRender();
        }
      } catch (error) {
        handlers.onError?.(
          error instanceof Error ? error : new Error("Invalid user order message"),
        );
      }
    };
    socket.onerror = () => {
      handlers.onError?.(new Error("User orders websocket error"));
    };
  }

  void start();

  return () => {
    closed = true;
    if (rafId !== null) {
      window.cancelAnimationFrame(rafId);
    }
    if (socket && socket.readyState <= WebSocket.OPEN) {
      try {
        socket.send(
          JSON.stringify({
            op: "unsubscribe",
            channel: "user",
            user_address: userAddress,
          }),
        );
      } catch {
        // ignore cleanup errors
      }
      socket.close();
    }
    socket = null;
  };
}
