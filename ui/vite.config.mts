import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// ReScript compiles `.res` → `.res.mjs` in place; Vite only ever sees JS.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    // Same-origin `/ws` in dev too: run `cargo run -p moor-client-web`.
    proxy: { "/ws": { target: "ws://127.0.0.1:9777", ws: true } },
  },
  test: {
    include: ["tests/**/*.test.ts", "__tests__/**/*_test.res.mjs"],
    environment: "jsdom",
  },
});
