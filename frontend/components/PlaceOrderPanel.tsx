"use client";

type OrderSide = "buy" | "sell";
type OrderType = "limit" | "market";
type Outcome = "yes" | "no";

const QUICK_AMOUNTS = [1, 5, 10, 25, 50];

type PlaceOrderPanelProps = {
  outcome: Outcome;
  side: OrderSide;
  orderType: OrderType;
  price: string;
  size: string;
  tickSize: string;
  loading: boolean;
  message: string | null;
  onSideChange: (side: OrderSide) => void;
  onOrderTypeChange: (orderType: OrderType) => void;
  onPriceChange: (price: string) => void;
  onSizeChange: (size: string) => void;
  onSubmit: (e: React.FormEvent) => void;
};

function TabItem({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={[
        "border-b-2 pb-2 text-sm font-medium transition",
        active
          ? "border-brand-gold text-brand-text"
          : "border-transparent text-brand-muted hover:text-brand-text",
      ].join(" ")}
    >
      {label}
    </button>
  );
}

function OrderTypeToggle({
  value,
  onChange,
}: {
  value: OrderType;
  onChange: (orderType: OrderType) => void;
}) {
  function toggleOrderType() {
    onChange(value === "limit" ? "market" : "limit");
  }

  return (
    <button
      type="button"
      aria-label={`Switch to ${value === "limit" ? "market" : "limit"} order`}
      onClick={toggleOrderType}
      className="inline-flex cursor-pointer items-center pb-2 text-sm font-medium text-brand-muted hover:text-brand-text"
    >
      <span>{value === "limit" ? "Limit" : "Market"}</span>
      <svg
        aria-hidden="true"
        viewBox="0 0 20 20"
        fill="currentColor"
        className="ml-[2ch] h-3.5 w-3.5 shrink-0 text-brand-muted"
      >
        <path
          fillRule="evenodd"
          d="M5.23 7.21a.75.75 0 011.06.02L10 11.168l3.71-3.938a.75.75 0 111.08 1.04l-4.25 4.5a.75.75 0 01-1.08 0l-4.25-4.5a.75.75 0 01.02-1.06z"
          clipRule="evenodd"
        />
      </svg>
    </button>
  );
}

export function PlaceOrderPanel({
  outcome,
  side,
  orderType,
  price,
  size,
  tickSize,
  loading,
  message,
  onSideChange,
  onOrderTypeChange,
  onPriceChange,
  onSizeChange,
  onSubmit,
}: PlaceOrderPanelProps) {
  const submitLabel =
    side === "buy"
      ? `Buy ${outcome === "yes" ? "Yes" : "No"}`
      : `Sell ${outcome === "yes" ? "Yes" : "No"}`;

  return (
    <section className="sticky top-6 rounded-2xl border border-brand-line bg-gradient-to-b from-brand-panel-2 to-brand-panel p-5">
      <div className="mb-5 flex items-end justify-between border-b border-brand-line">
        <div className="flex gap-5">
          <TabItem label="Buy" active={side === "buy"} onClick={() => onSideChange("buy")} />
          <TabItem label="Sell" active={side === "sell"} onClick={() => onSideChange("sell")} />
        </div>
        <OrderTypeToggle value={orderType} onChange={onOrderTypeChange} />
      </div>

      <form onSubmit={onSubmit} className="space-y-4">
        {orderType === "limit" && (
          <label className="block text-sm">
            <span className="mb-2 block text-xs font-medium uppercase tracking-wide text-brand-muted">
              Limit price (¢)
            </span>
            <input
              type="number"
              min="0"
              max="100"
              step="any"
              inputMode="decimal"
              className="w-full rounded-lg border border-brand-line bg-brand-bg px-3 py-2.5 text-brand-text tabular-nums outline-none focus:border-brand-gold focus:ring-1 focus:ring-brand-gold"
              value={price}
              onChange={(e) => onPriceChange(e.target.value)}
            />
          </label>
        )}

        <div>
          <label className="block text-sm">
            <span className="mb-2 block text-xs font-medium uppercase tracking-wide text-brand-muted">
              Amount
            </span>
            <input
              type="number"
              min="0"
              step="0.000001"
              className="w-full rounded-lg border border-brand-line bg-brand-bg px-3 py-2.5 text-brand-text outline-none focus:border-brand-gold focus:ring-1 focus:ring-brand-gold"
              value={size}
              onChange={(e) => onSizeChange(e.target.value)}
            />
          </label>
          <div className="mt-2 flex flex-wrap gap-2">
            {QUICK_AMOUNTS.map((amount) => (
              <button
                key={amount}
                type="button"
                onClick={() => onSizeChange(String(amount))}
                className="rounded-full border border-brand-gold-dim px-3 py-1 text-xs font-medium text-brand-gold hover:bg-[#22c55e14]"
              >
                {amount}
              </button>
            ))}
          </div>
        </div>

        <button
          type="submit"
          disabled={loading}
          className={[
            "w-full rounded-lg px-4 py-3 text-sm font-semibold text-white transition disabled:opacity-50",
            side === "buy"
              ? "bg-brand-yes hover:brightness-110"
              : "bg-brand-no hover:brightness-110",
          ].join(" ")}
        >
          {loading ? "Submitting..." : submitLabel}
        </button>
      </form>

      {message && <p className="mt-3 text-sm text-brand-muted">{message}</p>}
    </section>
  );
}
