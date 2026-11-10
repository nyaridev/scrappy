// Registers scrappy-host with Chrome, Brave, Edge, Chromium and Firefox for this user, so the extension can reach
// the app during development without the app running. The app does the same from Settings → Browser extension
// (`bridge/register.rs`, ADR-0014); keep the browser list and IDs the same in both.
//
//   pnpm register-host                     the debug build (target/debug), after `cargo build -p scrappy-host`
//   pnpm register-host --host <path>       another scrappy-host, such as release/scrappy-host.exe
//   pnpm register-host --remove            undoes it
//
// The app must sit next to the host (scrappy or Scrappy), as it does in target/debug and in release/.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { homedir, platform } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

// ── Constants ─────────────────────────────
const NAME = "com.nyaridev.scrappy";
/** The ID Chrome gives the extension, from the key in `extension/wxt.config.ts`. */
const CHROME_ID = "agmacpepkdohlgbkbgnafcmjggjmeohj";
const FIREFOX_ID = "scrappy@nyaridev";
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DESCRIPTION = "Relays the Scrappy extension's messages to the Scrappy app.";

/** Per browser: the Windows registry key, and the macOS and Linux folders for host manifests (under the home). */
const BROWSERS = [
  {
    name: "Chrome",
    firefox: false,
    key: "Software\\Google\\Chrome",
    mac: "Library/Application Support/Google/Chrome/NativeMessagingHosts",
    linux: ".config/google-chrome/NativeMessagingHosts",
  },
  {
    name: "Brave",
    firefox: false,
    key: "Software\\BraveSoftware\\Brave-Browser",
    mac: "Library/Application Support/BraveSoftware/Brave-Browser/NativeMessagingHosts",
    linux: ".config/BraveSoftware/Brave-Browser/NativeMessagingHosts",
  },
  {
    name: "Edge",
    firefox: false,
    key: "Software\\Microsoft\\Edge",
    mac: "Library/Application Support/Microsoft Edge/NativeMessagingHosts",
    linux: ".config/microsoft-edge/NativeMessagingHosts",
  },
  {
    name: "Chromium",
    firefox: false,
    key: "Software\\Chromium",
    mac: "Library/Application Support/Chromium/NativeMessagingHosts",
    linux: ".config/chromium/NativeMessagingHosts",
  },
  {
    name: "Firefox",
    firefox: true,
    key: "Software\\Mozilla",
    mac: "Library/Application Support/Mozilla/NativeMessagingHosts",
    linux: ".mozilla/native-messaging-hosts",
  },
];

// ── Private helpers ───────────────────────
function argument(flag) {
  const index = process.argv.indexOf(flag);
  return index === -1 ? null : (process.argv[index + 1] ?? null);
}

function manifest(host, firefox) {
  const allowed = firefox
    ? { allowed_extensions: [FIREFOX_ID] }
    : { allowed_origins: [`chrome-extension://${CHROME_ID}/`] };
  return `${JSON.stringify({ name: NAME, description: DESCRIPTION, path: host, type: "stdio", ...allowed }, null, 2)}\n`;
}

function registerWindows(host, remove) {
  const folder = join(ROOT, "target", "native-host");
  mkdirSync(folder, { recursive: true });
  for (const browser of BROWSERS) {
    const file = join(folder, browser.firefox ? "firefox.json" : "chromium.json");
    const key = `HKCU\\${browser.key}\\NativeMessagingHosts\\${NAME}`;
    if (remove) {
      try {
        execFileSync("reg", ["delete", key, "/f"], { stdio: "ignore" });
        console.log(`Removed from ${browser.name}`);
      } catch {
        // Not registered for this browser.
      }
      continue;
    }
    writeFileSync(file, manifest(host, browser.firefox));
    execFileSync("reg", ["add", key, "/ve", "/t", "REG_SZ", "/d", file, "/f"], { stdio: "ignore" });
    console.log(`Registered for ${browser.name}`);
  }
}

function registerUnix(host, remove) {
  for (const browser of BROWSERS) {
    const folder = join(homedir(), platform() === "darwin" ? browser.mac : browser.linux);
    const file = join(folder, `${NAME}.json`);
    if (remove) {
      if (existsSync(file)) {
        rmSync(file);
        console.log(`Removed from ${browser.name}`);
      }
      continue;
    }
    // Only browsers that are installed: their settings folder exists.
    if (!existsSync(dirname(folder))) continue;
    mkdirSync(folder, { recursive: true });
    writeFileSync(file, manifest(host, browser.firefox));
    console.log(`Registered for ${browser.name}`);
  }
}

// ── Main ──────────────────────────────────
const remove = process.argv.includes("--remove");
const exe = platform() === "win32" ? "scrappy-host.exe" : "scrappy-host";
const host = resolve(argument("--host") ?? join(ROOT, "target", "debug", exe));
if (!remove && !existsSync(host)) {
  console.error(`No scrappy-host at ${host}. Run \`cargo build -p scrappy-host\` first.`);
  process.exit(1);
}
if (platform() === "win32") registerWindows(host, remove);
else registerUnix(host, remove);
