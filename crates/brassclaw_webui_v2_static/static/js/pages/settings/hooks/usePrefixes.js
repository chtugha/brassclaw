import { React } from "../../../lib/html.js";
import { fetchPrefixes, regeneratePrefix } from "../lib/settings-api.js";

/**
 * Manages the prefix cache list and per-entry regenerate action.
 *
 * Returns:
 *   entries        — PrefixEntry[] | null
 *   isLoading      — initial load in flight
 *   loadError      — Error | null for initial load
 *   regenerating   — Set<string> of names currently regenerating
 *   regenerateError — string | null last regenerate error message
 *   handleRegenerate — (name: string) => Promise<void>
 *   reload         — () => void  force a fresh fetch
 */
export function usePrefixes() {
  const [entries, setEntries] = React.useState(null);
  // isLoading is only true on the very first mount fetch, not on background reloads,
  // so the skeleton does not flash every time the list is refreshed.
  const [isLoading, setIsLoading] = React.useState(true);
  const [loadError, setLoadError] = React.useState(null);
  const [regenerating, setRegenerating] = React.useState(() => new Set());
  const [regenerateError, setRegenerateError] = React.useState(null);
  const [tick, setTick] = React.useState(0);

  React.useEffect(() => {
    let cancelled = false;
    // Only show the full-page skeleton on the initial mount (tick === 0).
    // Subsequent reloads update entries silently in the background.
    if (tick === 0) setIsLoading(true);
    setLoadError(null);
    fetchPrefixes()
      .then((data) => {
        if (!cancelled) setEntries(data.prefixes ?? []);
      })
      .catch((err) => {
        if (!cancelled) setLoadError(err);
      })
      .finally(() => {
        if (!cancelled) setIsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [tick]);

  const reload = React.useCallback(() => setTick((n) => n + 1), []);

  const handleRegenerate = React.useCallback(async (name) => {
    setRegenerateError(null);
    setRegenerating((prev) => new Set([...prev, name]));
    try {
      const updated = await regeneratePrefix(name);
      // Merge the regenerate response directly into the entry — no skeleton flash.
      // The optimistic merge is sufficient; a background reload syncs any other fields.
      setEntries((prev) =>
        prev
          ? prev.map((e) =>
              e.name === name
                ? { ...e, ...updated, is_stale: false }
                : e
            )
          : prev
      );
    } catch (err) {
      // Use the structured payload from ApiError when available.
      // For a 429 rate-limit the payload is { code: "rate_limited", kind: "busy", retryable: true }.
      // Fall back to raw message parsing for non-ApiError throws.
      let msg;
      const payload = err?.payload;
      if (payload?.code === "rate_limited") {
        msg = "rate_limited_cooldown";
      } else if (payload?.code) {
        msg = `${payload.code}${payload.kind && payload.kind !== payload.code ? ` (${payload.kind})` : ""}`;
      } else {
        msg = err.message || String(err);
        try {
          const parsed = typeof msg === "string" ? JSON.parse(msg) : null;
          if (parsed?.error) {
            msg = `${parsed.error}${parsed.kind && parsed.kind !== parsed.error ? ` (${parsed.kind})` : ""}`;
          }
        } catch (_) {}
      }
      setRegenerateError(msg);
    } finally {
      setRegenerating((prev) => {
        const next = new Set(prev);
        next.delete(name);
        return next;
      });
    }
  }, []);

  return {
    entries,
    isLoading,
    loadError,
    regenerating,
    regenerateError,
    handleRegenerate,
    reload,
  };
}
