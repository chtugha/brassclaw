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
  const [isLoading, setIsLoading] = React.useState(true);
  const [loadError, setLoadError] = React.useState(null);
  const [regenerating, setRegenerating] = React.useState(() => new Set());
  const [regenerateError, setRegenerateError] = React.useState(null);
  const [tick, setTick] = React.useState(0);

  React.useEffect(() => {
    let cancelled = false;
    setIsLoading(true);
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
      setEntries((prev) =>
        prev
          ? prev.map((e) => (e.name === name ? { ...e, ...updated } : e))
          : prev
      );
    } catch (err) {
      setRegenerateError(err.message || String(err));
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
