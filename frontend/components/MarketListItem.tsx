"use client";

import Link from "next/link";
import { MarketIcon } from "@/components/MarketIcon";
import type { Market } from "@/lib/types";

function formatYesRate(yesRate?: string | null): string {
  if (!yesRate) {
    return "—";
  }
  const value = Number.parseFloat(yesRate);
  if (!Number.isFinite(value)) {
    return "—";
  }
  return `${value % 1 === 0 ? value.toFixed(0) : value.toFixed(1)}%`;
}

export function MarketListItem({ market }: { market: Market }) {
  return (
    <article className="w-full rounded-2xl border border-brand-line bg-gradient-to-b from-brand-panel-2 to-brand-panel transition hover:border-brand-gold-dim">
      <Link
        href={`/markets/${market.slug}`}
        className="flex items-start justify-between gap-4 px-5 py-5"
      >
        <div className="flex min-w-0 flex-1 items-start gap-3.5">
          <MarketIcon iconUrl={market.icon_url} question={market.question} size="md" />
          <div className="min-w-0 flex-1">
            <h2 className="truncate text-[1.15rem] font-semibold tracking-[-0.02em] text-brand-text">
              {market.question}
            </h2>
            <div className="mt-1.5 flex flex-wrap items-center gap-2">
              <span className="break-all font-mono text-xs text-brand-muted">
                {market.market_address}
              </span>
              <span className="rounded-full border border-brand-line px-2.5 py-0.5 text-[10px] font-medium uppercase tracking-wide text-brand-gold-dim">
                {market.state}
              </span>
            </div>
            <p className="mt-2 text-xs text-brand-muted">
              Deadline: {new Date(market.resolution_deadline * 1000).toLocaleString()}
            </p>
          </div>
        </div>

        <div className="shrink-0 self-end text-right">
          <span className="text-[1.85rem] font-semibold tabular-nums tracking-[-0.04em] text-brand-yes">
            {formatYesRate(market.yes_rate)}
          </span>
        </div>
      </Link>
    </article>
  );
}
