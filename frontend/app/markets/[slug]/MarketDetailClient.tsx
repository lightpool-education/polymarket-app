"use client";

import { useEffect, useState } from "react";
import { MarketOutcomeSelector } from "@/components/MarketOutcomeSelector";
import { OrderBook } from "@/components/OrderBook";
import { PlaceOrderPanel } from "@/components/PlaceOrderPanel";
import {
  createOrderBookSubscription,
  createUserOrdersSubscription,
  fetchMarketInfo,
  upsertOrder,
} from "@/lib/clob";
import { api } from "@/lib/api";
import {
  bookLevelKey,
  bookSideForOrderSide,
  filterOrdersForBook,
} from "@/lib/orders";
import { requestPortfolioRefresh } from "@/lib/portfolio";
import type { BookResponse, Market, Order } from "@/lib/types";

export default function MarketDetailClient({
  params,
  initialMarket,
  initialOutcome,
}: {
  params: { slug: string };
  initialMarket: Market | null;
  initialOutcome: "yes" | "no";
}) {
  const [outcome, setOutcome] = useState<"yes" | "no">(initialOutcome);
  const [side, setSide] = useState<"buy" | "sell">("buy");
  const [orderType, setOrderType] = useState<"limit" | "market">("limit");
  const [price, setPrice] = useState("50");
  const [size, setSize] = useState("10");
  const [message, setMessage] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [yesBook, setYesBook] = useState<BookResponse | null>(null);
  const [noBook, setNoBook] = useState<BookResponse | null>(null);
  const [bookLoading, setBookLoading] = useState(true);
  const [bookError, setBookError] = useState<string | null>(null);
  const [orders, setOrders] = useState<Order[]>([]);
  const [closeAllLoading, setCloseAllLoading] = useState(false);
  const [cancellingLevelKey, setCancellingLevelKey] = useState<string | null>(null);
  const [tickSize, setTickSize] = useState("0.01");

  useEffect(() => {
    setOutcome(initialOutcome);
  }, [initialOutcome]);

  useEffect(() => {
    let unsubscribeOrders: (() => void) | null = null;
    let cancelled = false;

    async function startUserOrdersSubscription() {
      try {
        const account = await api.getAccount();
        if (cancelled) {
          return;
        }
        unsubscribeOrders = createUserOrdersSubscription(account.address, {
          onOrders: (nextOrders) => {
            setOrders(nextOrders);
          },
        });
      } catch {
        // Keep the previous order list when account or subscription setup fails.
      }
    }

    void startUserOrdersSubscription();

    return () => {
      cancelled = true;
      unsubscribeOrders?.();
    };
  }, []);

  useEffect(() => {
    if (!initialMarket) {
      return;
    }

    setBookLoading(true);
    setBookError(null);

    let yesLoaded = false;
    let noLoaded = false;

    const markLoaded = () => {
      if (yesLoaded && noLoaded) {
        setBookLoading(false);
      }
    };

    const unsubscribeYes = createOrderBookSubscription(initialMarket.yes_spot_market, 10, {
      onBook: (book) => {
        setYesBook(book);
        setBookError(null);
        yesLoaded = true;
        markLoaded();
      },
      onError: (error) => {
        setBookError(error.message);
        setBookLoading(false);
      },
    });
    const unsubscribeNo = createOrderBookSubscription(initialMarket.no_spot_market, 10, {
      onBook: (book) => {
        setNoBook(book);
        setBookError(null);
        noLoaded = true;
        markLoaded();
      },
      onError: (error) => {
        setBookError(error.message);
        setBookLoading(false);
      },
    });

    return () => {
      unsubscribeYes();
      unsubscribeNo();
    };
  }, [initialMarket]);

  useEffect(() => {
    if (!initialMarket) {
      return;
    }

    const spotMarket =
      outcome === "yes" ? initialMarket.yes_spot_market : initialMarket.no_spot_market;
    let cancelled = false;

    async function loadTickSize() {
      try {
        const account = await api.getAccount();
        if (cancelled) {
          return;
        }
        const info = await fetchMarketInfo(spotMarket, account.address);
        if (!cancelled) {
          setTickSize(info.tick_size);
        }
      } catch {
        if (!cancelled) {
          setTickSize("0.01");
        }
      }
    }

    void loadTickSize();

    return () => {
      cancelled = true;
    };
  }, [initialMarket, outcome]);

  if (!initialMarket) {
    return <p className="text-sm text-slate-500">Market not found.</p>;
  }

  const activeBook = outcome === "yes" ? yesBook : noBook;
  const openOrders = filterOrdersForBook(orders, params.slug, outcome);

  async function cancelOrders(levelOrders: Order[]) {
    if (levelOrders.length === 0) {
      return;
    }

    const firstOrder = levelOrders[0];
    const levelKey = bookLevelKey(
      bookSideForOrderSide(firstOrder.side),
      firstOrder.price,
    );
    setCancellingLevelKey(levelKey);
    setMessage(null);

    try {
      const cancelled = await Promise.all(
        levelOrders.map((order) => api.cancelOrder(order.id)),
      );
      setOrders((prev) =>
        cancelled.reduce((next, order) => upsertOrder(next, order), prev),
      );
      requestPortfolioRefresh();
      setMessage(
        levelOrders.length === 1
          ? "Cancel submitted."
          : `${levelOrders.length} cancels submitted.`,
      );
    } catch (err) {
      setMessage(err instanceof Error ? err.message : "Cancel failed");
    } finally {
      setCancellingLevelKey(null);
    }
  }

  async function closeAllOrders() {
    if (openOrders.length === 0) {
      return;
    }

    setCloseAllLoading(true);
    setMessage(null);

    try {
      const cancelled = await Promise.all(
        openOrders.map((order) => api.cancelOrder(order.id)),
      );
      setOrders((prev) =>
        cancelled.reduce((next, order) => upsertOrder(next, order), prev),
      );
      requestPortfolioRefresh();
      setMessage(
        openOrders.length === 1
          ? "Cancel submitted."
          : `${openOrders.length} cancels submitted.`,
      );
    } catch (err) {
      setMessage(err instanceof Error ? err.message : "Close all failed");
    } finally {
      setCloseAllLoading(false);
    }
  }

  function selectBuyYes() {
    setOutcome("yes");
    setSide("buy");
    const ask = yesBook?.asks[0]?.price;
    if (ask) {
      setPrice(ask);
    }
  }

  function selectBuyNo() {
    setOutcome("no");
    setSide("buy");
    const ask = noBook?.asks[0]?.price;
    if (ask) {
      setPrice(ask);
    }
  }

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    setLoading(true);
    setMessage(null);
    try {
      const order = await api.placeOrder({
        market_slug: params.slug,
        outcome,
        side,
        price,
        size,
        order_type: orderType,
      });
      setOrders((prev) => upsertOrder(prev, order));
      setMessage(`Order submitted: ${order.id} (${order.status}).`);
      requestPortfolioRefresh();
    } catch (err) {
      setMessage(err instanceof Error ? err.message : "Order failed");
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="grid w-full gap-6 lg:grid-cols-[minmax(0,1fr)_380px]">
      <div className="space-y-4">
        <MarketOutcomeSelector
          question={initialMarket.question}
          iconUrl={initialMarket.icon_url}
          yesBook={yesBook}
          noBook={noBook}
          selectedOutcome={outcome}
          onBuyYes={selectBuyYes}
          onBuyNo={selectBuyNo}
        />

        <OrderBook
          book={activeBook}
          outcome={outcome}
          tickSize={tickSize}
          openOrders={openOrders}
          loading={bookLoading}
          error={bookError}
          closeAllLoading={closeAllLoading}
          cancellingLevelKey={cancellingLevelKey}
          onCancelLevel={(levelOrders) => {
            void cancelOrders(levelOrders);
          }}
          onCloseAll={() => {
            void closeAllOrders();
          }}
          onPriceClick={(nextPrice) => {
            setOrderType("limit");
            setPrice(nextPrice);
          }}
        />
      </div>

      <PlaceOrderPanel
        outcome={outcome}
        side={side}
        orderType={orderType}
        price={price}
        size={size}
        tickSize={tickSize}
        loading={loading}
        message={message}
        onSideChange={setSide}
        onOrderTypeChange={setOrderType}
        onPriceChange={setPrice}
        onSizeChange={setSize}
        onSubmit={onSubmit}
      />
    </div>
  );
}
