"use client";

import type { BookResponse } from "@/lib/types";
import { MarketIcon } from "@/components/MarketIcon";

type Outcome = "yes" | "no";

function bestAsk(book: BookResponse | null): string | null {
  if (!book || book.asks.length === 0) {
    return null;
  }
  return book.asks[0].price;
}

type MarketOutcomeSelectorProps = {
  question: string;
  iconUrl?: string | null;
  yesBook: BookResponse | null;
  noBook: BookResponse | null;
  selectedOutcome: Outcome;
  onBuyYes: () => void;
  onBuyNo: () => void;
};

export function MarketOutcomeSelector({
  question,
  iconUrl,
  yesBook,
  noBook,
  selectedOutcome,
  onBuyYes,
  onBuyNo,
}: MarketOutcomeSelectorProps) {
  const yesPrice = bestAsk(yesBook);
  const noPrice = bestAsk(noBook);

  return (
    <section className="rounded-2xl border border-brand-line bg-gradient-to-b from-brand-panel-2 to-brand-panel p-5">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between sm:gap-6">
        <div className="min-w-0 flex-1">
          <div className="flex items-start gap-3">
            <MarketIcon iconUrl={iconUrl} question={question} size="lg" />
            <h1 className="min-w-0 flex-1 text-xl font-semibold leading-snug tracking-[-0.02em] text-brand-text lg:text-2xl">
              {question}
            </h1>
          </div>
        </div>

        <div className="grid w-full shrink-0 grid-cols-2 gap-1.5 sm:w-auto sm:min-w-[200px] sm:max-w-[220px]">
          <button
            type="button"
            onClick={onBuyYes}
            className={[
              "rounded-lg border px-2.5 py-1.5 text-left transition",
              selectedOutcome === "yes"
                ? "border-brand-yes bg-brand-yes text-white"
                : "border-emerald-200 bg-emerald-50 text-brand-yes hover:bg-emerald-100",
            ].join(" ")}
          >
            <span
              className={[
                "block text-[10px] font-medium",
                selectedOutcome === "yes" ? "text-white/80" : "text-brand-yes",
              ].join(" ")}
            >
              Buy Yes
            </span>
            <span
              className={[
                "block text-sm font-semibold tabular-nums",
                selectedOutcome === "yes" ? "text-white" : "text-brand-text",
              ].join(" ")}
            >
              {yesPrice ? `${yesPrice}¢` : "--"}
            </span>
          </button>
          <button
            type="button"
            onClick={onBuyNo}
            className={[
              "rounded-lg border px-2.5 py-1.5 text-left transition",
              selectedOutcome === "no"
                ? "border-brand-no bg-brand-no text-white"
                : "border-rose-200 bg-rose-50 text-brand-no hover:bg-rose-100",
            ].join(" ")}
          >
            <span
              className={[
                "block text-[10px] font-medium",
                selectedOutcome === "no" ? "text-white/80" : "text-brand-no",
              ].join(" ")}
            >
              Buy No
            </span>
            <span
              className={[
                "block text-sm font-semibold tabular-nums",
                selectedOutcome === "no" ? "text-white" : "text-brand-text",
              ].join(" ")}
            >
              {noPrice ? `${noPrice}¢` : "--"}
            </span>
          </button>
        </div>
      </div>
    </section>
  );
}
