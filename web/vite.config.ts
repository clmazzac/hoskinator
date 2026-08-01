import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The daemon serves the built bundle itself; in dev, Vite proxies RPC to the daemon instead.
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: { "/rpc": "http://127.0.0.1:8737" },
  },
});
