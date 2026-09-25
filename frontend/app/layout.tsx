import "./globals.css";
import type { Metadata } from "next";
import { Instrument_Sans, Newsreader } from "next/font/google";
import { NavBar } from "@/components/NavBar";
import { WalletProviders } from "@/lib/wallet";
import { loadPortfolioSummary } from "@/lib/portfolio";

const instrumentSans = Instrument_Sans({
  subsets: ["latin"],
  variable: "--font-sans",
  display: "swap",
});

const newsreader = Newsreader({
  subsets: ["latin"],
  variable: "--font-display",
  display: "swap",
  weight: ["500", "600"],
});

export const metadata: Metadata = {
  title: "LightPool Event Contracts",
  description: "Prediction markets on LightPool",
};

export default async function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  let portfolio = 0;
  let cash = 0;

  try {
    const summary = await loadPortfolioSummary();
    portfolio = summary.portfolio;
    cash = summary.cash;
  } catch {
    // Keep nav values at zero when backend is unavailable.
  }

  return (
    <html lang="en" className={`${instrumentSans.variable} ${newsreader.variable}`}>
      <body className={`${instrumentSans.className} min-h-screen text-brand-text antialiased`}>
        <WalletProviders>
          <NavBar initialPortfolio={portfolio} initialCash={cash} />
          <main className="lp-page mx-auto w-full max-w-7xl px-4 py-9 md:px-6">{children}</main>
        </WalletProviders>
      </body>
    </html>
  );
}
