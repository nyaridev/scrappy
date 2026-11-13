// Sites: the sites that get Download buttons in their own place and look (ADR-0035). Add a site as its own file here.

import type { Site } from "./site";
import { soundcloud } from "./soundcloud";
import { twitch } from "./twitch";
import { x } from "./x";
import { youtube } from "./youtube";

/** Every supported site. */
export const SITES: Site[] = [youtube, soundcloud, x, twitch];
