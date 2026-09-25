import { api } from "@/lib/api";
import { EventMarketGroup } from "@/components/EventMarketGroup";
import { MarketListItem } from "@/components/MarketListItem";
import { groupMarketsForList } from "@/lib/marketGroups";
import type { Market } from "@/lib/types";

export default async function HomePage() {
  let markets: Market[] = [];
  let error: string | null = null;

  try {
    markets = await api.listMarkets();
  } catch (e) {
    error = e instanceof Error ? e.message : "Failed to load markets";
  }

  const entries = groupMarketsForList(markets);

  return (
    <div>
      {error && (
        <div className="mb-4 rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700">
          {error}
        </div>
      )}

      <div className="grid w-full gap-4">
        {entries.map((entry) =>
          entry.kind === "group" ? (
            <EventMarketGroup
              key={`event:${entry.eventSlug}`}
              title={entry.title}
              eventSlug={entry.eventSlug}
              markets={entry.markets}
            />
          ) : (
            <MarketListItem key={entry.market.id} market={entry.market} />
          ),
        )}

        {!error && entries.length === 0 && (
          <p className="text-sm text-brand-muted">No markets yet.</p>
        )}
      </div>
    </div>
  );
}
