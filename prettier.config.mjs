/**
 * @see https://prettier.io/docs/configuration
 * @type {import("prettier").Config &
 *   import("prettier-plugin-tailwindcss").PluginOptions}
 */
const config = {
  plugins: [
    "prettier-plugin-svelte",
    // Must be last. It patches the other plugins so class sorting still runs.
    "prettier-plugin-tailwindcss",
  ],
  overrides: [
    {
      files: "*.svelte",
      options: {
        parser: "svelte",
      },
    },
  ],
  // Tailwind v4 has no JS config. Theme tokens live in this CSS entry
  // (`@import "tailwindcss"` plus the shared `@theme` stylesheet).
  // The desktop client uses the same imports.
  tailwindStylesheet: "./client/web/src/app.css",
  // Sort class strings inside these calls, not only `class` attributes.
  tailwindFunctions: ["cn", "clsx", "tv"],
};

export default config;
