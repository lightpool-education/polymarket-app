"use client";

import Link from "next/link";
import { useState } from "react";
import { api } from "@/lib/api";
import { findPositionMeta, getPositionBalances } from "@/lib/balances";
import { requestPortfolioRefresh } from "@/lib/portfolio";
import type { BalanceEntry, Market, Order } from "@/lib/types";

type Tab = "position" | "open" | "history";

const OPEN_STATUSES = new Set(["open", "partial_filled"]);
const HISTORY_STATUSES = new Set([
  "filled",
  "cancelled",
  "canceled",
  "rejected",
  "failed",
]);

function isOpenOrderStatus(status: string): boolean {
  return OPEN_STATUSES.has(status.trim().toLowerCase());
}

function isHistoryOrderStatus(status: string): boolean {
  return HISTORY_STATUSES.has(status.trim().toLowerCase());
}

const tabs: { id: Tab; label: string }[] = [
  { id: "position", label: "Position" },
  { id: "open", label: "Open Orders" },
  { id: "history", label: "Order History" },
];

function tabHref(tab: Tab) {
  return tab === "position" ? "/portfolio" : `/portfolio?tab=${tab}`;
}

function isActiveTab(current: Tab, tab: Tab) {
  return current === tab;
}

export default function DashboardTabs({
  activeTab,
  balances,
  orders,
  markets,
  error,
}: {
  activeTab: Tab;
  balances: BalanceEntry[];
  orders: Order[];
  markets: Market[];
  error: string | null;
}) {
  const openOrders = orders.filter((o) => isOpenOrderStatus(o.status));
  const historyOrders = orders.filter((o) => isHistoryOrderStatus(o.status));
  const positions = getPositionBalances(balances, markets);

  return (
    <div>
      <div className="mb-6 flex gap-1 border-b border-sky-100">
        {tabs.map((tab) => (
          <Link
            key={tab.id}
            href={tabHref(tab.id)}
            className={[
              "px-4 py-2 text-sm font-medium",
              isActiveTab(activeTab, tab.id)
                ? "border-b-2 border-sky-500 text-sky-700"
                : "text-slate-500 hover:text-sky-700",
            ].join(" ")}
          >
            {tab.label}
          </Link>
        ))}
      </div>

      {error && (
        <div className="mb-4 rounded border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700">
          {error}
        </div>
      )}

      {activeTab === "position" && (
        <div className="overflow-hidden rounded-xl border border-sky-100 bg-white">
          <table className="min-w-full text-sm">
            <thead className="bg-slate-100 text-left">
              <tr>
                <th className="px-4 py-3">Outcome</th>
                <th className="px-4 py-3">Total</th>
                <th className="px-4 py-3">Locked</th>
                <th className="px-4 py-3">Available</th>
              </tr>
            </thead>
            <tbody>
              {positions.map((b) => {
                const meta = findPositionMeta(b.token, markets);

                return (
                  <tr key={b.token} className="border-t border-slate-100">
                    <td className="px-4 py-3">
                      {meta ? (
                        <Link
                          href={`/markets/${meta.marketSlug}?outcome=${meta.outcome}`}
                          className="text-slate-900 hover:underline"
                        >
                          {meta.question}{" "}
                          <span className="font-medium">[{outcomeLabel(meta.outcome)}]</span>
                        </Link>
                      ) : (
                        <span className="font-medium">{b.symbol}</span>
                      )}
                    </td>
                    <td className="px-4 py-3">{b.total}</td>
                    <td className="px-4 py-3">{b.locked}</td>
                    <td className="px-4 py-3">{b.available}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          {!error && positions.length === 0 && (
            <p className="px-4 py-6 text-sm text-slate-500">No positions yet.</p>
          )}
        </div>
      )}

      {activeTab === "open" && (
        <OrdersTable
          orders={openOrders}
          emptyMessage="No open orders."
          showError={!!error}
          allowCancel
        />
      )}

      {activeTab === "history" && (
        <OrdersTable
          orders={historyOrders}
          emptyMessage="No order history."
          showError={!!error}
        />
      )}
    </div>
  );
}

function outcomeLabel(outcome: string): string {
  if (outcome === "yes") {
    return "Yes";
  }
  if (outcome === "no") {
    return "No";
  }
  return outcome;
}

function statusLabel(status: string): string {
  if (status === "partial_filled") {
    return "Partial filled";
  }
  if (status === "filled") {
    return "Filled";
  }
  if (status === "cancelled") {
    return "Cancelled";
  }
  if (status === "open") {
    return "Open";
  }
  return status;
}

function OrdersTable({
  orders,
  emptyMessage,
  showError,
  allowCancel = false,
  cancelStatuses = [],
}: {
  orders: Order[];
  emptyMessage: string;
  showError: boolean;
  allowCancel?: boolean;
  cancelStatuses?: string[];
}) {
  const [cancellingId, setCancellingId] = useState<string | null>(null);
  const [cancelError, setCancelError] = useState<string | null>(null);

  async function onCancel(orderId: string) {
    setCancellingId(orderId);
    setCancelError(null);
    try {
      await api.cancelOrder(orderId);
      requestPortfolioRefresh();
    } catch (err) {
      setCancelError(err instanceof Error ? err.message : "Cancel failed");
    } finally {
      setCancellingId(null);
    }
  }

  return (
    <div className="overflow-hidden rounded-xl border border-sky-100 bg-white">
      {cancelError && (
        <div className="border-b border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700">
          {cancelError}
        </div>
      )}
      <table className="min-w-full text-sm">
        <thead className="bg-slate-100 text-left">
          <tr>
            <th className="px-4 py-3">Outcome</th>
            <th className="px-4 py-3">Side</th>
            <th className="px-4 py-3">Price (¢)</th>
            <th className="px-4 py-3">Size</th>
            <th className="px-4 py-3">Status</th>
            {(allowCancel || cancelStatuses.length > 0) && <th className="px-4 py-3" />}
          </tr>
        </thead>
        <tbody>
          {orders.map((order) => (
            <tr key={order.id} className="border-t border-slate-100">
              <td className="px-4 py-3">
                <Link
                  href={`/markets/${order.market_slug || order.market_id}?outcome=${order.outcome}`}
                  className="text-slate-900 hover:underline"
                >
                  {order.question || "Unknown market"}{" "}
                  <span className="font-medium">[{outcomeLabel(order.outcome)}]</span>
                </Link>
              </td>
              <td className="px-4 py-3 capitalize">{order.side}</td>
              <td className="px-4 py-3">{order.price}</td>
              <td className="px-4 py-3">{order.size}</td>
              <td className="px-4 py-3">{statusLabel(order.status)}</td>
              {(allowCancel || cancelStatuses.includes(order.status)) && (
                <td className="px-4 py-3 text-right">
                  <button
                    type="button"
                    onClick={() => onCancel(order.id)}
                    disabled={cancellingId === order.id}
                    className="rounded border border-slate-300 px-3 py-1 text-xs font-medium text-slate-700 hover:bg-slate-50 disabled:opacity-50"
                  >
                    {cancellingId === order.id ? "Cancelling..." : "Cancel"}
                  </button>
                </td>
              )}
            </tr>
          ))}
        </tbody>
      </table>
      {!showError && orders.length === 0 && (
        <p className="px-4 py-6 text-sm text-slate-500">{emptyMessage}</p>
      )}
    </div>
  );
}
