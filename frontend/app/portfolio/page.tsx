"use client";

import { Suspense, useCallback, useEffect, useState } from "react";
import { useSearchParams } from "next/navigation";
import { api } from "@/lib/api";
import DashboardTabs from "@/components/DashboardTabs";
import { createUserOrdersSubscription } from "@/lib/clob";
import {
  getSessionToken,
  SESSION_CHANGED_EVENT,
} from "@/lib/session";
import { PORTFOLIO_REFRESH_EVENT } from "@/lib/portfolio";
import type { BalanceEntry, Market, Order } from "@/lib/types";

type Tab = "position" | "open" | "history";

function parseTab(value: string | null): Tab {
  if (value === "open" || value === "history") {
    return value;
  }
  return "position";
}

function PortfolioClient() {
  const searchParams = useSearchParams();
  const activeTab = parseTab(searchParams.get("tab"));

  const [balances, setBalances] = useState<BalanceEntry[]>([]);
  const [orders, setOrders] = useState<Order[]>([]);
  const [markets, setMarkets] = useState<Market[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    if (!getSessionToken()) {
      setBalances([]);
      setOrders([]);
      setMarkets([]);
      setError(null);
      setLoading(false);
      return;
    }

    setLoading(true);
    try {
      const [nextBalances, nextOrders, nextMarkets] = await Promise.all([
        api.getBalances(),
        api.listOrders(),
        api.listMarkets(),
      ]);
      setBalances(nextBalances);
      setOrders(nextOrders);
      setMarkets(nextMarkets);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load portfolio data");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();

    function onRefresh() {
      void load();
    }

    window.addEventListener(PORTFOLIO_REFRESH_EVENT, onRefresh);
    window.addEventListener(SESSION_CHANGED_EVENT, onRefresh);
    return () => {
      window.removeEventListener(PORTFOLIO_REFRESH_EVENT, onRefresh);
      window.removeEventListener(SESSION_CHANGED_EVENT, onRefresh);
    };
  }, [load]);

  useEffect(() => {
    if (!getSessionToken()) {
      return;
    }

    let unsubscribe: (() => void) | null = null;
    let cancelled = false;

    async function startOrdersWs() {
      try {
        const account = await api.getAccount();
        if (cancelled) {
          return;
        }
        unsubscribe = createUserOrdersSubscription(account.address, {
          onOrders: (nextOrders) => {
            setOrders(nextOrders);
          },
          onError: (err) => {
            setError(err.message);
          },
        });
      } catch {
        // Keep HTTP snapshot when WS setup fails.
      }
    }

    void startOrdersWs();
    return () => {
      cancelled = true;
      unsubscribe?.();
    };
  }, []);

  if (!getSessionToken() && !loading) {
    return (
      <p className="text-sm text-slate-500">
        Connect your wallet and sign in to view portfolio positions and orders.
      </p>
    );
  }

  if (loading && balances.length === 0 && orders.length === 0) {
    return <p className="text-sm text-slate-500">Loading…</p>;
  }

  return (
    <DashboardTabs
      activeTab={activeTab}
      balances={balances}
      orders={orders}
      markets={markets}
      error={error}
    />
  );
}

export default function PortfolioPage() {
  return (
    <Suspense fallback={<p className="text-sm text-slate-500">Loading…</p>}>
      <PortfolioClient />
    </Suspense>
  );
}
