/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        bg: "#1a1a2e",
        card: "#16213e",
        accent: "#e8633a",
        "text-primary": "#e8e8e8",
        "text-secondary": "#888888",
        "header-bg": "#0f3460",
        "logout-btn": "#333355",
        "qr-border": "#333366",
      },
      fontFamily: {
        sans: ["Microsoft YaHei", "PingFang SC", "sans-serif"],
      },
    },
  },
  plugins: [],
};
