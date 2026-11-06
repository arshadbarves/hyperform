import { svelte } from "@sveltejs/vite-plugin-svelte";
import path from "node:path";
import { defineConfig } from "vite";

// Port must match `build.devUrl` in src-tauri/tauri.conf.json.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  resolve: {
    alias: {
      $lib: path.resolve("./src/lib"),
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
