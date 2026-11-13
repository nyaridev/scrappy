// SignInLine: the tab's site and whether Scrappy remembers its sign-in, with a button that shares it, downloading
// nothing (ADR-0042). That one site's cookies are read only on the click.

import { Button, Icon } from "iro-ui";
import { CheckIcon, KeyIcon } from "iro-ui/icons";
import { useEffect, useState } from "react";
import { sendToApp } from "../../lib/app";
import { siteCookies } from "../../lib/cookies";

// ── Types ─────────────────────────────────
type Line =
  | { state: "loading" }
  | { state: "none" }
  | { state: "ready"; site: string; signedIn: boolean }
  | { state: "sharing"; site: string; signedIn: boolean }
  | { state: "shared"; site: string }
  | { state: "failed"; site: string; signedIn: boolean; message: string };

// ── Public API ────────────────────────────
/** The sign-in line for the page at `url`; nothing when the app can't be reached or the page has no site. */
export function SignInLine({ url }: { url: string }) {
  const [line, setLine] = useState<Line>({ state: "loading" });

  useEffect(() => {
    sendToApp({ type: "site", url }).then(
      (reply) => setLine(reply.type === "site" ? { state: "ready", ...reply } : { state: "none" }),
      () => setLine({ state: "none" }),
    );
  }, [url]);

  if (line.state === "loading" || line.state === "none") return null;

  async function share(site: string, signedIn: boolean) {
    setLine({ state: "sharing", site, signedIn });
    const reply = await siteCookies(site)
      .then((cookies) => sendToApp({ type: "signIn", url, cookies }))
      .catch((error: Error) => ({ type: "error", message: error.message }) as const);
    setLine(
      reply.type === "signedIn"
        ? { state: "shared", site }
        : { state: "failed", site, signedIn, message: reply.type === "error" ? reply.message : "Not shared." },
    );
  }

  const signedIn = line.state === "shared" || ("signedIn" in line && line.signedIn);
  return (
    <div className="popup-signin">
      <Icon icon={KeyIcon} size={16} />
      <span className="popup-signin-text">
        <span className="popup-signin-site">{line.site}</span>
        <span className="popup-signin-state">{signedIn ? "Sign-in remembered" : "No sign-in shared"}</span>
        {line.state === "failed" && <span className="popup-error">{line.message}</span>}
      </span>
      {line.state === "shared" ? (
        <span className="row-sent">
          <Icon icon={CheckIcon} label="Shared" />
        </span>
      ) : (
        <Button
          size="sm"
          variant="ghost"
          loading={line.state === "sharing"}
          onClick={() => void share(line.site, line.signedIn)}
        >
          {line.signedIn ? "Update sign-in" : "Share sign-in"}
        </Button>
      )}
    </div>
  );
}
