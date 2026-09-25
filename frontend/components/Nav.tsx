"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { cashLabel, portfolioLabel } from "@/lib/balances";
import { ConnectWallet } from "@/components/ConnectWallet";

type NavProps = {
  portfolio: number;
  cash: number;
};

function navClass(active: boolean) {
  return active
    ? "font-medium text-brand-text"
    : "text-brand-muted hover:text-brand-text";
}

export function Nav({ portfolio, cash }: NavProps) {
  const pathname = usePathname();

  return (
    <header className="sticky top-0 z-10 border-b border-brand-line bg-white/90 backdrop-blur">
      <div className="lp-page mx-auto flex w-full max-w-7xl items-center justify-between gap-6 px-4 py-4 md:px-6">
        <a
          href="https://github.com/lightpool-labs/event-contract-app"
          target="_blank"
          rel="noopener noreferrer"
          className="shrink-0 whitespace-nowrap text-brand-text"
        >
          <span className="text-lg font-semibold tracking-[-0.02em]">
            Event contracts
          </span>{" "}
          <span className="text-xs font-medium text-brand-muted">
            powered by <span className="text-brand-gold">LightPool</span>
          </span>
        </a>
        <nav className="flex flex-nowrap items-center justify-end gap-5 text-sm">
          <Link
            href="/"
            className={navClass(pathname === "/" || pathname.startsWith("/markets"))}
          >
            Markets
          </Link>
          <Link href="/vaults" className={navClass(pathname === "/vaults")}>
            Vaults
          </Link>
          <Link
            href="/portfolio"
            className={navClass(
              pathname === "/portfolio" || pathname.startsWith("/portfolio?"),
            )}
          >
            Portfolio {portfolioLabel(portfolio)}
          </Link>
          <Link href="/cash" className={navClass(pathname === "/cash")}>
            Cash {cashLabel(cash)}
          </Link>
          <ConnectWallet />
        </nav>
      </div>
    </header>
  );
}
