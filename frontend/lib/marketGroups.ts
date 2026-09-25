import type { Market } from "@/lib/types";

export type MarketListEntry =
  | { kind: "group"; eventSlug: string; title: string; markets: Market[] }
  | { kind: "single"; market: Market };

function yesRateValue(market: Market): number {
  const value = Number.parseFloat(market.yes_rate ?? "");
  return Number.isFinite(value) ? value : Number.NEGATIVE_INFINITY;
}

export function stripPriceFromQuestion(question: string): string {
  return question
    .replace(/\$[\d,]+(?:\.\d+)?/g, "")
    .replace(/\s{2,}/g, " ")
    .replace(/\s+([?.!,])/g, "$1")
    .trim();
}

export function marketStrikeLabel(market: Market): string {
  const item = market.group_item_title?.trim();
  if (item) {
    return item;
  }
  const match = market.question?.match(/\$[\d,]+(?:\.\d+)?/);
  return match?.[0] ?? "Untitled";
}

function groupKeyForMarket(market: Market): string {
  const eventSlug = market.event_slug?.trim();
  if (eventSlug) {
    return `event:${eventSlug}`;
  }
  const stripped = stripPriceFromQuestion(market.question || "");
  if (stripped && stripped !== market.question?.trim()) {
    return `question:${stripped.toLowerCase()}`;
  }
  return `single:${market.slug}`;
}

export function eventGroupTitle(markets: Market[]): string {
  const lead = markets[0];
  if (!lead) {
    return "";
  }
  const fromQuestion = stripPriceFromQuestion(lead.question || "");
  if (fromQuestion) {
    return fromQuestion;
  }
  return lead.event_slug?.trim() || lead.slug;
}

export function groupMarketsForList(markets: Market[]): MarketListEntry[] {
  const buckets = new Map<string, Market[]>();

  for (const market of markets) {
    const key = groupKeyForMarket(market);
    const list = buckets.get(key) ?? [];
    list.push(market);
    buckets.set(key, list);
  }

  const entries: MarketListEntry[] = [];

  for (const [key, groupMarkets] of buckets) {
    const sorted = [...groupMarkets].sort((left, right) => {
      const rateDiff = yesRateValue(right) - yesRateValue(left);
      if (rateDiff !== 0) {
        return rateDiff;
      }
      return left.slug.localeCompare(right.slug);
    });

    const hasEventSlug = Boolean(sorted[0]?.event_slug?.trim());
    const canGroup =
      hasEventSlug ||
      (sorted.length >= 2 && key.startsWith("question:"));

    if (!canGroup) {
      for (const market of sorted) {
        entries.push({ kind: "single", market });
      }
      continue;
    }

    const eventSlug = sorted[0]?.event_slug?.trim() || key;
    entries.push({
      kind: "group",
      eventSlug,
      title: eventGroupTitle(sorted),
      markets: sorted,
    });
  }

  entries.sort((left, right) => {
    const leftKey =
      left.kind === "group" ? left.title || left.eventSlug : left.market.slug;
    const rightKey =
      right.kind === "group" ? right.title || right.eventSlug : right.market.slug;
    return leftKey.localeCompare(rightKey);
  });

  return entries;
}
