/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        brightwing: {
          blue: "#3B7DD8",
          "blue-dark": "#2A5BA0",
          orange: "#F5811F",
          "orange-dark": "#D06A10",
          navy: "#1E3A5F",
          gray: {
            50: "#F8FAFC",
            100: "#F1F5F9",
            200: "#E2E8F0",
            300: "#CBD5E1",
            400: "#94A3B8",
            500: "#64748B",
            600: "#475569",
            700: "#334155",
            800: "#1E293B",
            900: "#0F172A",
          },
        },
      },
    },
  },
  plugins: [],
};
