import { defineConfig } from "wxt";

// One codebase, two builds: `pnpm build` (Chrome, Brave, Edge) and `pnpm build:firefox` (ADR-0006).
export default defineConfig({
  modules: ["@wxt-dev/module-react"],
  // Firefox defaults to MV2 in WXT; both browsers build MV3.
  manifestVersion: 3,
  manifest: {
    name: "Scrappy",
    description: "Lists the videos and images on this page and sends the one you pick to the Scrappy app.",
    // Permissions arrive with M5, each one named in ADR-0006.
    permissions: [],
  },
});
