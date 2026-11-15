/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  // Keep the base reset OFF: the rest of the app is hand-written plain CSS
  // and a global preflight would clobber it. Utilities are appended after
  // the hand-written rules in styles.css so they win same-specificity ties.
  corePlugins: { preflight: false },
  theme: {
    extend: {
      colors: {
        iso: "#3b82f6", // app accent (blue)
      },
    },
  },
  plugins: [],
};
