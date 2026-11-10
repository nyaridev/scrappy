// SignedInSites: the sites whose browser sign-in Scrappy remembers (shared from the extension), each with Forget.

import { Button, Icon, SectionTitle, toast } from "iro-ui";
import { KeyIcon } from "iro-ui/icons";
import { useCached } from "../../app/useCached";
import { forgetSite, signedInSites } from "../../lib/backend";
import { formatWhen } from "../../lib/format";

/** The section. */
export function SignedInSites() {
  const [sites, setSites] = useCached("signedInSites", signedInSites);

  async function forget(site: string) {
    try {
      await forgetSite(site);
      setSites(await signedInSites());
    } catch (error) {
      toast({ tone: "error", title: "Couldn't forget the sign-in", description: String(error) });
    }
  }

  return (
    <section className="settings-section">
      <SectionTitle icon={<Icon icon={KeyIcon} />}>Signed-in sites</SectionTitle>
      {sites?.length === 0 && <p className="settings-hint">None</p>}
      {sites && sites.length > 0 && (
        <ul className="sites" aria-label="Signed-in sites">
          {sites.map((site) => (
            <li key={site.site} className="site-row">
              <span className="site-name">{site.site}</span>
              <span className="site-detail">
                {site.cookies} {site.cookies === 1 ? "cookie" : "cookies"} · {formatWhen(site.savedAt)}
              </span>
              <Button size="sm" variant="ghost" onClick={() => void forget(site.site)}>
                Forget
              </Button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
