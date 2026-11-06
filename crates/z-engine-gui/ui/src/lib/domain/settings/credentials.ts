import type { KeyStatus } from "../../protocol/config/KeyStatus";
import type { SearchBackend } from "../../protocol/config/SearchBackend";

const NO_KEY: KeyStatus = { hasKey: false, hint: null };

const KNOWN_HOSTS: ReadonlyArray<[domain: string, bucket: string]> = [
  ["openrouter.ai", "openrouter"],
  ["opencode.ai", "opencode"],
  ["anthropic.com", "anthropic"],
  ["githubcopilot.com", "github-copilot"],
];

/** `host[:port]` of a URL, lowercased and without credentials. */
function authority(url: string): string {
  const trimmed = url.trim();
  const scheme = trimmed.indexOf("://");
  const rest = scheme >= 0 ? trimmed.slice(scheme + 3) : trimmed;
  const hostPart = rest.split(/[/?#]/)[0] ?? "";
  const at = hostPart.lastIndexOf("@");
  return (at >= 0 ? hostPart.slice(at + 1) : hostPart).toLowerCase();
}

function hostName(value: string): string {
  if (value.startsWith("[")) return value.slice(1).split("]")[0] ?? value;
  return value.split(":")[0] ?? value;
}

/** The lowercased host of a URL, without scheme, credentials, port or path. */
export function hostOf(url: string): string {
  return hostName(authority(url));
}

/** The `auth.json` bucket of a model API, as the config crate names it; keys never cross hosts. */
export function credentialKey(baseUrl: string): string {
  const auth = authority(baseUrl);
  const host = hostName(auth);
  for (const [domain, bucket] of KNOWN_HOSTS) {
    if (host === domain || host.endsWith(`.${domain}`)) return bucket;
  }
  if (host === "api.openai.com") return "openai";
  return `custom:${auth}`;
}

/** SearXNG and "none" need no key. */
export function searchKeyBucket(backend: SearchBackend): string | null {
  return backend === "brave" || backend === "tavily" || backend === "exa" ? backend : null;
}

export function keyStatus(credentials: Record<string, KeyStatus>, bucket: string | null): KeyStatus {
  return (bucket && credentials[bucket]) || NO_KEY;
}

export function keyHintText(status: KeyStatus): string {
  return status.hint ? `Saved (••••${status.hint})` : "Saved";
}
