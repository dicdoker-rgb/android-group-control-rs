import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      "/api/unlock": {
        target: "http://192.168.31.9:8079",
        changeOrigin: true,
      },
      "/api/device": {
        target: "http://192.168.31.9:8079",
        changeOrigin: true,
      },
      "/api": {
        target: "http://127.0.0.1:8876",
        changeOrigin: true,
        ws: true,
      },
      "/unlock": {
        target: "http://192.168.31.9:8079",
        changeOrigin: true,
      },
      "/console": {
        target: "http://192.168.31.9:8079",
        changeOrigin: true,
      },
      "/screen": {
        target: "http://192.168.31.9:8079",
        changeOrigin: true,
        ws: true,
      },
      "/static": {
        target: "http://192.168.31.9:8079",
        changeOrigin: true,
      },
    },
  },
  test: {
    environment: "jsdom",
    setupFiles: "./src/test/setup.js",
    globals: true,
  },
});
