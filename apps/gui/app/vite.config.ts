import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig({
  plugins: [solid()],
  optimizeDeps: {
    include: ["@tauri-apps/api/core", "@tauri-apps/api/webview"],
  },
  server: {
    host: "127.0.0.1",
    port: 5174,
    strictPort: true,
  },
});
