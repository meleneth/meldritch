import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";
export default defineConfig({
  plugins: [vue()],
  build: {
    outDir: "../web/dist",
    emptyOutDir: true,
    rollupOptions: {
      output: {
        entryFileNames: "assets/app.js",
        assetFileNames: "assets/app.[ext]",
        // Rust embeds a fixed app.js/app.css surface. Keep lazy modules in the
        // same JavaScript artifact so a content-hashed chunk cannot become an
        // unserved runtime dependency.
        inlineDynamicImports: true,
      },
    },
  },
  server: {
    proxy: {
      "/api": "http://127.0.0.1:8080",
      "/performance": { target: "ws://127.0.0.1:8080", ws: true },
    },
  },
  test: { environment: "jsdom" },
});
