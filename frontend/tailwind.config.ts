import type { Config } from "tailwindcss";

const config: Config = {
  content: ["./app/**/*.{js,ts,jsx,tsx}", "./components/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        brand: {
          bg: "#ffffff",
          panel: "#ffffff",
          "panel-2": "#fafafa",
          line: "#e5e5e5",
          text: "#111111",
          muted: "#6b6b6b",
          gold: "#22c55e",
          "gold-dim": "#16a34a",
          yes: "#22c55e",
          no: "#f04438",
        },
        sky: {
          50: "#f5f5f5",
          100: "#f5f5f5",
          200: "#e5e5e5",
          300: "#d1d5dc",
          400: "#4ade80",
          500: "#22c55e",
          600: "#22c55e",
          700: "#16a34a",
          800: "#111111",
          900: "#111111",
        },
        slate: {
          50: "#fafafa",
          100: "#e5e5e5",
          200: "#e5e5e5",
          300: "#d1d5dc",
          400: "#9ca3af",
          500: "#6b6b6b",
          600: "#6b6b6b",
          700: "#374151",
          800: "#1f2937",
          900: "#111111",
        },
        emerald: {
          50: "#e8f9ee",
          100: "#dcfce7",
          200: "#bbf7d0",
          500: "#22c55e",
          600: "#22c55e",
          700: "#16a34a",
          800: "#15803d",
          900: "#053b1f",
        },
        rose: {
          50: "#fdecec",
          100: "#fbd9d9",
          200: "#fbd9d9",
          500: "#f04438",
          600: "#f04438",
          700: "#f04438",
          900: "#7f1d1d",
        },
        amber: {
          50: "#fffbeb",
          100: "#fef3c7",
          200: "#fde68a",
          800: "#92400e",
        },
        red: {
          50: "#fdecec",
          200: "#fbd9d9",
          600: "#f04438",
          700: "#f04438",
        },
      },
      fontFamily: {
        sans: ["var(--font-sans)", '"Instrument Sans"', "ui-sans-serif", "system-ui", "sans-serif"],
        display: ["var(--font-display)", '"Newsreader"', "Georgia", "serif"],
      },
    },
  },
  plugins: [],
};

export default config;
