import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The Rust backend (src/bin/web.rs). Matches GROUP_CONTROL_PORT below.
const API_TARGET = process.env.VITE_API_TARGET ?? "http://127.0.0.1:8876";

// Optional secondary screen/unlock service. Disabled unless VITE_SCREEN_TARGET
// is set, so a fresh clone starts with zero hard-coded LAN addresses.
const SCREEN_TARGET = process.env.VITE_SCREEN_TARGET;
const screenProxy = SCREEN_TARGET
  ? Object.fromEntries(
      [
        ["/api/unlock", false],
        ["/api/device", false],
        ["/unlock", false],
        ["/console", false],
        ["/screen", true],
        ["/static", false],
      ].map(([path, ws]) => [path, { target: SCREEN_TARGET, changeOrigin: true, ws }])
    )
  : {};

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      // Order matters: the more specific prefixes must win over "/api".
      ...screenProxy,
      "/api": {
        target: API_TARGET,
        changeOrigin: true,
        ws: true,
      },
    },
  },
  test: {
    environment: "jsdom",
    setupFiles: "./src/test/setup.js",
    globals: true,
  },
});
