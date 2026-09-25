"use client";

import Link from "next/link";
import { MarketIcon } from "@/components/MarketIcon";
import { marketStrikeLabel } from "@/lib/marketGroups";
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

function yesRateValue(yesRate?: string | null): number {
  const value = Number.parseFloat(yesRate ?? "");
  return Number.isFinite(value) ? value : Number.NEGATIVE_INFINITY;
}

export function EventMarketGroup({
  title,
  markets,
}: {
  title: string;
  eventSlug?: string;
  markets: Market[];
}) {
  const lead = markets[0];
  const topRate = Math.max(...markets.map((market) => yesRateValue(market.yes_rate)));

  return (
    <article className="w-full overflow-hidden rounded-2xl border border-brand-line bg-gradient-to-b from-brand-panel-2 to-brand-panel">
      <div className="flex items-start gap-3.5 border-b border-brand-line px-5 py-5">
        <MarketIcon iconUrl={lead?.icon_url} question={title} size="md" />
        <div className="min-w-0 flex-1">
          <h2 className="truncate text-[1.15rem] font-semibold tracking-[-0.02em] text-brand-text">
            {title}
          </h2>
          <p className="mt-1.5 text-xs text-brand-muted">
            {markets.length} market{markets.length === 1 ? "" : "s"} · ranked by Yes %
          </p>
        </div>
      </div>

      <ul>
        {markets.map((market) => {
          const hot = yesRateValue(market.yes_rate) === topRate && topRate > Number.NEGATIVE_INFINITY;
          return (
            <li key={market.id} className="border-b border-brand-line last:border-b-0">
              <Link
                href={`/markets/${market.slug}`}
                className="flex items-center gap-3 px-5 py-4 transition hover:bg-slate-50"
              >
                <div className="min-w-0 flex-1">
                  <span className="text-[0.95rem] font-semibold tracking-[-0.01em] text-brand-text">
                    {marketStrikeLabel(market)}
                  </span>
                  <span className="ml-2 rounded-full border border-brand-line px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide text-brand-gold-dim">
                    {market.state}
                  </span>
                </div>

                <div className="shrink-0 text-right">
                  <span
                    className={[
                      "text-[1.85rem] font-semibold tabular-nums tracking-[-0.04em]",
                      hot ? "text-brand-yes" : "text-brand-muted",
                    ].join(" ")}
                  >
                    {formatYesRate(market.yes_rate)}
                  </span>
                </div>
              </Link>
            </li>
          );
        })}
      </ul>
    </article>
  );
}
