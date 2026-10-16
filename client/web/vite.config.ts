import tailwindcss from "@tailwindcss/vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [tailwindcss(), svelte()],
  // Load `VITE_*` from the monorepo root (same place as example.env).
  envDir: "../..",
  server: {
    port: 5173,
    fs: { allow: [".."] },
  },
});
