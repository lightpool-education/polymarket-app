type MarketIconProps = {
  iconUrl?: string | null;
  question: string;
  size?: "sm" | "md" | "lg";
  className?: string;
};

const sizeClasses = {
  sm: "h-10 w-10 text-sm",
  md: "h-11 w-11 text-base",
  lg: "h-14 w-14 text-lg",
};

export function MarketIcon({
  iconUrl,
  question,
  size = "md",
  className = "",
}: MarketIconProps) {
  const sizeClass = sizeClasses[size];
  const fallbackLetter = question.trim().charAt(0).toUpperCase() || "?";

  if (iconUrl) {
    return (
      <img
        src={iconUrl}
        alt=""
        className={[
          "shrink-0 rounded-full object-cover ring-1 ring-brand-gold-dim",
          sizeClass,
          className,
        ].join(" ")}
      />
    );
  }

  return (
    <div
      className={[
        "flex shrink-0 items-center justify-center rounded-full border border-brand-gold-dim font-semibold text-brand-gold",
        sizeClass,
        className,
      ].join(" ")}
    >
      {fallbackLetter}
    </div>
  );
}
