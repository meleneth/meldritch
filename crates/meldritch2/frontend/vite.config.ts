import { defineConfig } from "vite";
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
