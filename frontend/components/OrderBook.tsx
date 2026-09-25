"use client";

import { useMemo } from "react";
import { formatPriceCents, subtractPriceCents } from "@/lib/price";
import type { BookLevel, BookResponse, Order } from "@/lib/types";
import { bookLevelKey, ordersAtBookLevel } from "@/lib/orders";

type OrderBookProps = {
  book: BookResponse | null;
  outcome: "yes" | "no";
  tickSize: string;
  openOrders?: Order[];
  loading?: boolean;
  error?: string | null;
  closeAllLoading?: boolean;
  cancellingLevelKey?: string | null;
  onCancelLevel?: (orders: Order[]) => void;
  onCloseAll?: () => void;
  onPriceClick?: (price: string) => void;
};

function parseLevelSize(size: string): number {
  const value = Number.parseFloat(size);
  return Number.isFinite(value) ? value : 0;
}

function parseLevelPrice(price: string): number {
  const value = Number.parseFloat(price);
  return Number.isFinite(value) ? value : 0;
}

function maxLevelSize(levels: BookLevel[]): number {
  if (levels.length === 0) {
    return 1;
  }
  return Math.max(...levels.map((level) => parseLevelSize(level.size)), 1);
}

function formatBookAmount(value: string | number): string {
  const numeric =
    typeof value === "string" ? Number.parseFloat(value) : value;
  if (!Number.isFinite(numeric)) {
    return "--";
  }
  return numeric.toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

function formatNotional(price: string, size: string): string {
  const notionalCents = parseLevelPrice(price) * parseLevelSize(size);
  if (!Number.isFinite(notionalCents) || notionalCents === 0) {
    return "--";
  }
  return `$${formatBookAmount(notionalCents / 100)}`;
}

function formatSpread(
  bestBid: string | undefined,
  bestAsk: string | undefined,
  tickSize: string,
): string | null {
  if (!bestBid || !bestAsk) {
    return null;
  }

  const spread = subtractPriceCents(bestAsk, bestBid, tickSize);
  if (!spread) {
    return null;
  }

  const spreadParsed = Number.parseFloat(spread);
  if (!Number.isFinite(spreadParsed)) {
    return null;
  }

  if (spreadParsed <= 0) {
    return `${formatPriceCents(bestAsk, tickSize)}¢`;
  }
  return `${spread}¢ spread`;
}

function CloseOrderButton({
  disabled,
  onClick,
}: {
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-label="Cancel order"
      title="Cancel order"
      className="inline-flex h-4 w-4 shrink-0 items-center justify-center text-sm leading-none text-slate-500 transition hover:text-rose-600 disabled:cursor-not-allowed disabled:opacity-50"
    >
      ×
    </button>
  );
}

type DepthRowProps = {
  level: BookLevel;
  side: "ask" | "bid";
  maxSize: number;
  tickSize: string;
  levelOrders: Order[];
  cancelling: boolean;
  onCancelLevel?: (orders: Order[]) => void;
  onPriceClick?: (price: string) => void;
};

function DepthRow({
  level,
  side,
  maxSize,
  tickSize,
  levelOrders,
  cancelling,
  onCancelLevel,
  onPriceClick,
}: DepthRowProps) {
  const sizeValue = parseLevelSize(level.size);
  const depthPercent = Math.min(100, (sizeValue / maxSize) * 100);
  const isAsk = side === "ask";
  const barColor = isAsk ? "bg-rose-200" : "bg-emerald-200";
  const textColor = isAsk ? "text-rose-600" : "text-emerald-600";
  const hasUserOrder = levelOrders.length > 0;
  const priceLabel = `${formatPriceCents(level.price, tickSize)}¢`;

  return (
    <tr className="h-6">
      <td className="relative w-1/2 p-0 pr-2">
        <div className="flex h-6 w-full items-center gap-1">
          <div className="flex h-4 w-4 shrink-0 items-center justify-center">
            {hasUserOrder && onCancelLevel ? (
              <CloseOrderButton
                disabled={cancelling}
                onClick={() => onCancelLevel(levelOrders)}
              />
            ) : null}
          </div>
          <div className="relative h-6 min-w-0 flex-1">
            <div
              className={["absolute inset-y-0 left-0", barColor].join(" ")}
              style={{ width: `${depthPercent}%` }}
            />
          </div>
        </div>
      </td>
      <td className="h-6 p-0 align-middle pl-1 pr-2">
        {onPriceClick ? (
          <button
            type="button"
            onClick={() => onPriceClick(level.price)}
            title={`Use ${priceLabel} as limit price`}
            className={[
              "font-medium tabular-nums underline-offset-2 hover:underline",
              textColor,
            ].join(" ")}
          >
            {priceLabel}
          </button>
        ) : (
          <span className={["font-medium tabular-nums", textColor].join(" ")}>
            {priceLabel}
          </span>
        )}
      </td>
      <td className="h-6 p-0 align-middle pr-2 text-right">
        <span className="tabular-nums text-slate-700">{formatBookAmount(level.size)}</span>
      </td>
      <td className="h-6 p-0 align-middle text-right">
        <span className="tabular-nums text-slate-600">{formatNotional(level.price, level.size)}</span>
      </td>
    </tr>
  );
}

function SpreadSection({
  lastTradePrice,
  spreadLabel,
  tickSize,
}: {
  lastTradePrice: string | null;
  spreadLabel: string | null;
  tickSize: string;
}) {
  return (
    <div className="my-2 border-y border-brand-line py-2">
      <table className="w-full table-fixed border-collapse">
        <tbody>
          <tr>
            <td className="w-1/2" />
            <td className="w-[17%] py-0.5 pl-1 pr-2">
              <span className="text-xs font-semibold tabular-nums text-slate-900">
                {lastTradePrice ? `${formatPriceCents(lastTradePrice, tickSize)}¢` : "--"}
              </span>
            </td>
            <td colSpan={2} className="py-0.5 text-center">
              {spreadLabel && (
                <span className="text-xs font-medium tabular-nums text-slate-500">
                  {spreadLabel}
                </span>
              )}
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  );
}

export function OrderBook({
  book,
  outcome,
  tickSize,
  openOrders = [],
  loading = false,
  error = null,
  closeAllLoading = false,
  cancellingLevelKey = null,
  onCancelLevel,
  onCloseAll,
  onPriceClick,
}: OrderBookProps) {
  const asks = useMemo(() => [...(book?.asks ?? [])].reverse(), [book?.asks]);
  const bids = book?.bids ?? [];
  const maxSize = useMemo(() => maxLevelSize([...asks, ...bids]), [asks, bids]);
  const hasOpenOrders = openOrders.length > 0;

  const bestBid = bids[0]?.price;
  const bestAsk = book?.asks[0]?.price;
  const spreadLabel = formatSpread(bestBid, bestAsk, tickSize);
  const lastTradePrice = book?.last_trade_price ?? null;
  const tradeLabel = outcome === "yes" ? "TRADE YES" : "TRADE NO";

  const columnHeaders = (
    <thead>
      <tr className="text-left text-xs text-slate-500">
        <th className="w-1/2 pb-2 text-xs font-semibold uppercase tracking-wide text-slate-700">
          <div className="flex items-center gap-2">
            <span>{tradeLabel}</span>
            {hasOpenOrders && onCloseAll && (
              <button
                type="button"
                onClick={onCloseAll}
                disabled={closeAllLoading || cancellingLevelKey !== null}
                className="rounded border border-slate-300 px-2 py-0.5 text-[10px] font-medium normal-case tracking-normal text-slate-600 transition hover:border-rose-300 hover:bg-rose-50 hover:text-rose-600 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {closeAllLoading ? "Closing..." : "Close all"}
              </button>
            )}
          </div>
        </th>
        <th className="w-[17%] pb-2 font-medium">Price</th>
        <th className="w-[16%] pb-2 text-right font-medium">Size</th>
        <th className="w-[17%] pb-2 text-right font-medium">Total</th>
      </tr>
    </thead>
  );

  return (
    <section className="rounded-2xl border border-brand-line bg-gradient-to-b from-brand-panel-2 to-brand-panel">
      <div className="border-b border-brand-line px-5 py-3">
        <h2 className="text-sm font-semibold text-brand-text">Order Book</h2>
      </div>

      <div className="px-5 py-4">
        {loading && <p className="text-sm text-slate-500">Loading book...</p>}
        {error && <p className="text-sm text-red-600">{error}</p>}

        {!loading && !error && (
          <div className="text-sm">
            <table className="w-full table-fixed border-collapse">
              {columnHeaders}
              <tbody>
                {asks.map((level, index) => {
                  const levelOrders = ordersAtBookLevel(openOrders, level.price, "ask");
                  return (
                    <DepthRow
                      key={`ask-${level.price}-${index}`}
                      level={level}
                      side="ask"
                      maxSize={maxSize}
                      tickSize={tickSize}
                      levelOrders={levelOrders}
                      cancelling={cancellingLevelKey === bookLevelKey("ask", level.price)}
                      onCancelLevel={onCancelLevel}
                      onPriceClick={onPriceClick}
                    />
                  );
                })}
                {asks.length === 0 && (
                  <tr>
                    <td colSpan={4} className="py-2 text-slate-400">
                      No asks
                    </td>
                  </tr>
                )}
              </tbody>
            </table>

            <SpreadSection
              lastTradePrice={lastTradePrice}
              spreadLabel={spreadLabel}
              tickSize={tickSize}
            />

            <table className="w-full table-fixed border-collapse">
              <tbody>
                {bids.map((level, index) => {
                  const levelOrders = ordersAtBookLevel(openOrders, level.price, "bid");
                  return (
                    <DepthRow
                      key={`bid-${level.price}-${index}`}
                      level={level}
                      side="bid"
                      maxSize={maxSize}
                      tickSize={tickSize}
                      levelOrders={levelOrders}
                      cancelling={cancellingLevelKey === bookLevelKey("bid", level.price)}
                      onCancelLevel={onCancelLevel}
                      onPriceClick={onPriceClick}
                    />
                  );
                })}
                {bids.length === 0 && (
                  <tr>
                    <td colSpan={4} className="py-2 text-slate-400">
                      No bids
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </section>
  );
}
