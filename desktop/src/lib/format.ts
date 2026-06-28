import { DeliveryStatus } from "../models/Message";

/**
 * Returns up to 2 uppercase initials from a display name.
 * Empty or whitespace-only names return "".
 */
export function initials(name: string): string {
  return name
    .split(/\s+/)
    .filter((w) => w.length > 0)
    .slice(0, 2)
    .map((w) => w[0].toUpperCase())
    .join("");
}

/**
 * Encodes a contact invite token (base64url of `{s:serverUrl,d:inviterDid}`),
 * matching iOS `IdentityDetailView.makeInviteToken`. Single-char wire keys keep
 * the token short. The decode side lives in `AppContext`'s deep-link handler.
 */
export function makeInviteToken(serverUrl: string, inviterDid: string): string {
  const json = JSON.stringify({ s: serverUrl, d: inviterDid });
  return btoa(json).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/**
 * Builds the shareable contact URL for an identity, matching iOS
 * `IdentityDetailView.contactURL` (`https://go.theavalanche.net/i/<token>`).
 */
export function contactInviteUrl(serverUrl: string, inviterDid: string): string {
  return `https://go.theavalanche.net/i/${makeInviteToken(serverUrl, inviterDid)}`;
}

/**
 * Returns the hostname of `url`, or `fallback` if the URL cannot be parsed.
 */
export function displayHost(url: string, fallback: string): string {
  try {
    return new URL(url).hostname;
  } catch {
    return fallback;
  }
}

/**
 * Formats a unix-ms timestamp as a locale hour:minute string (e.g. "2:34 PM").
 * Used by MessageBubble timestamps.
 */
export function formatTime(ms: number): string {
  return new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

/**
 * Formats a unix-ms timestamp as a relative string for the conversation list.
 * < 60s → "Just now", < 60m → "{m}m", < 24h → "{h}h", < 48h → "Yesterday",
 * else locale date string.
 */
export function formatRelative(ms: number): string {
  const diff = Date.now() - ms;
  const secs = Math.floor(diff / 1000);
  if (secs < 60) return "Just now";
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h`;
  if (hours < 48) return "Yesterday";
  return new Date(ms).toLocaleDateString();
}

/**
 * Maps DeliveryStatus to a numeric rank for forward-progression comparisons.
 * sending=0, sent=1, delivered=2, read=3.  `failed`(4) returns -1 — it is a
 * terminal error state, not "more advanced than read".  Callers must handle
 * `failed` separately rather than comparing by magnitude.
 */
export function deliveryRank(s: DeliveryStatus): number {
  switch (s) {
    case DeliveryStatus.sending:   return 0;
    case DeliveryStatus.sent:      return 1;
    case DeliveryStatus.delivered: return 2;
    case DeliveryStatus.read:      return 3;
    case DeliveryStatus.failed:    return -1;
  }
}

/**
 * Deterministic DID→palette-index in 0..11.  Same DID always yields the same
 * index across calls and sessions — pure, no randomness.
 * Used by AccountAvatar to pick a CSS palette class (CSP forbids inline style).
 */
export function avatarColorIndex(did: string): number {
  let h = 0;
  for (let i = 0; i < did.length; i++) {
    h = (h * 31 + did.charCodeAt(i)) | 0;
  }
  return ((h % 12) + 12) % 12;
}
