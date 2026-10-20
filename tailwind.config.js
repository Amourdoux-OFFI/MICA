/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        brand: {
          50: "#eefbf7",
          100: "#d5f5ec",
          200: "#aeead9",
          300: "#7cd9c2",
          400: "#48c0a6",
          500: "#2e9e8f",
          600: "#237f74",
          700: "#20665f",
          800: "#1e524d",
          900: "#1c4541",
          950: "#0a2826",
        },
        ink: {
          50: "#f6f7f8",
          100: "#eceef0",
          200: "#d6dadd",
          300: "#b3babf",
          400: "#8a949b",
          500: "#6b757d",
          600: "#565f67",
          700: "#464d54",
          800: "#33383d",
          900: "#22262a",
          950: "#15181a",
        },
      },
      fontFamily: {
        sans: [
          "Inter",
          "HarmonyOS Sans SC",
          "PingFang SC",
          "Microsoft YaHei UI",
          "Segoe UI",
          "sans-serif",
        ],
        mono: ["JetBrains Mono", "Cascadia Code", "Consolas", "monospace"],
      },
      borderRadius: {
        xl2: "14px",
      },
      boxShadow: {
        card: "0 1px 2px rgba(0,0,0,0.06), 0 8px 24px -12px rgba(0,0,0,0.25)",
        pop: "0 12px 48px -12px rgba(0,0,0,0.45)",
      },
      keyframes: {
        "fade-in": {
          from: { opacity: "0", transform: "translateY(4px)" },
          to: { opacity: "1", transform: "translateY(0)" },
        },
        shimmer: {
          "0%": { backgroundPosition: "-200% 0" },
          "100%": { backgroundPosition: "200% 0" },
        },
      },
      animation: {
        "fade-in": "fade-in .18s ease-out both",
        shimmer: "shimmer 1.4s linear infinite",
      },
    },
  },
  plugins: [],
};
