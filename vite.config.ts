import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Isolate Client — minimal Vite config.
// Dev server on the Tauri-standard port; no source maps in release (smaller + faster).
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**", "**/blueprint/**", "**/assets/**"],
    },
  },
  build: {
    target: "chrome105",
    minify: "esbuild",
    sourcemap: false,
  },
});
