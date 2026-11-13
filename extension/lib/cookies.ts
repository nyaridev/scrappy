// Cookies: one site's cookies from the browser, read only after the app asked for that site and the user agreed in
// the popup, or when the user shares the tab's site's sign-in there (ADR-0005, ADR-0042). Nothing else in the
// extension reads cookies.

import type { Cookie } from "./types/Cookie";

/** The browser's cookies for `site` and its subdomains, in the app's shape. */
export async function siteCookies(site: string): Promise<Cookie[]> {
  const found = await browser.cookies.getAll({ domain: site });
  return found.map((cookie) => ({
    domain: cookie.domain,
    name: cookie.name,
    value: cookie.value,
    path: cookie.path,
    secure: cookie.secure,
    httpOnly: cookie.httpOnly,
    hostOnly: cookie.hostOnly,
    expires: cookie.expirationDate ?? null,
  }));
}
