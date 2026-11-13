import { defineConfig } from "wxt";

/** The public half of a key made for Scrappy, so Chrome gives the unpacked extension the same ID everywhere. Its ID
 * is in `scripts/register-host.mjs`, which lets that ID reach the app. */
const CHROME_KEY =
  "MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAyb1TkB3i0RTkdUdSNyAGWzSIi3YhYESnIoNYBmZaD2xm4l6aucfn+gZoIlDCxiA+RI/MlaHISRmb3xZ08MPfeiqweNp7fEAWLD0U+mh11glOGLjGfSu06FUtLmxlnDWEb5r1HLD114Fl9GLYjx5VUWEul51IIIA/+EvOYe0W6X7f3Lla/bZxuqDSnec7Azyik2cTMrBLOU1qFq15SOpcrXe6M4qGjeJ9QHCw19vGKZmP+dgcoybmZ4qLrFb9cWJ9StaIfVZCccfmxKmrIvVxg9ZdGAZU/NAq5IzLisdsxFI6JVXtuljOWV4w/MmosEuuqC1c/OyasyNxqhbfSnHtIwIDAQAB";
/** Firefox's ID for Scrappy; the host manifest for Firefox lists it. */
const FIREFOX_ID = "scrappy@nyaridev";

// One codebase, two builds: `pnpm build` (Chrome, Brave, Edge) and `pnpm build:firefox` (ADR-0006).
export default defineConfig({
  modules: ["@wxt-dev/module-react"],
  // Firefox defaults to MV2 in WXT; both browsers build MV3.
  manifestVersion: 3,
  manifest: ({ browser }) => ({
    name: "Scrappy",
    description:
      "Sends videos and images to the Scrappy app: Download buttons on pages, and a list of each page's media.",
    // The toolbar shows the logo, not a letter.
    action: { default_icon: { 16: "icon/16.png", 32: "icon/32.png", 48: "icon/48.png" } },
    // Each one is explained in ADR-0006 → Permissions.
    permissions: ["nativeMessaging", "webRequest", "scripting", "cookies"],
    host_permissions: ["<all_urls>"],
    ...(browser === "firefox"
      ? { browser_specific_settings: { gecko: { id: FIREFOX_ID, strict_min_version: "128.0" } } }
      : { key: CHROME_KEY }),
  }),
});
