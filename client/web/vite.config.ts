import tailwindcss from "@tailwindcss/vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";
import { VitePWA } from "vite-plugin-pwa";

export default defineConfig({
  plugins: [
    tailwindcss(),
    svelte(),
    VitePWA({
      registerType: "autoUpdate",
      injectRegister: false,
      includeAssets: ["favicon.ico", "favicon.png", "logo.png", "robots.txt"],
      manifest: {
        name: "Nimbus",
        short_name: "Nimbus",
        description:
          "Nimbus is a hosting platform for uploading, organizing, sharing, and previewing your files.",
        theme_color: "#171717",
        background_color: "#171717",
        display: "standalone",
        start_url: "/",
        scope: "/",
        icons: [
          {
            src: "/favicon.png",
            sizes: "192x192",
            type: "image/png",
          },
          {
            src: "/favicon.png",
            sizes: "192x192",
            type: "image/png",
            purpose: "maskable",
          },
        ],
      },
      workbox: {
        navigateFallback: "/index.html",
        // API and health checks must never be answered with the SPA shell.
        navigateFallbackDenylist: [/^\/api(?:\/|$)/, /^\/_health(?:\/|$)/],
        globPatterns: ["**/*.{js,css,html,ico,png,svg,woff2,webmanifest}"],
        runtimeCaching: [
          {
            urlPattern: ({ request, sameOrigin }) =>
              sameOrigin &&
              (request.destination === "script" ||
                request.destination === "style" ||
                request.destination === "worker"),
            handler: "CacheFirst",
            options: {
              cacheName: "nimbus-assets",
              expiration: {
                maxEntries: 64,
                maxAgeSeconds: 60 * 60 * 24 * 365,
              },
            },
          },
          {
            // GET /roots/{root_id}/files/{id}/icon and GET /files/shared/{file_id}/icon
            // (optional `/api` prefix when the SPA and API share an origin).
            urlPattern: ({ url, request, sameOrigin }) => {
              if (!sameOrigin || request.method !== "GET") {
                return false;
              }

              const parts = url.pathname.split("/").filter(Boolean);
              const path = parts[0] === "api" ? parts.slice(1) : parts;

              const privateIcon =
                path.length === 5 &&
                path[0] === "roots" &&
                path[2] === "files" &&
                path[4] === "icon";

              const sharedIcon =
                path.length === 4 &&
                path[0] === "files" &&
                path[1] === "shared" &&
                path[3] === "icon";

              return privateIcon || sharedIcon;
            },
            handler: "StaleWhileRevalidate",
            options: {
              cacheName: "nimbus-file-icons",
              expiration: {
                maxEntries: 500,
                maxAgeSeconds: 60 * 60 * 24 * 7,
              },
              cacheableResponse: {
                statuses: [0, 200],
              },
            },
          },
        ],
      },
      devOptions: {
        enabled: false,
      },
    }),
  ],
  // Load `VITE_*` from the monorepo root (same place as example.env).
  envDir: "../..",
  server: {
    port: 5173,
    fs: { allow: [".."] },
  },
});
