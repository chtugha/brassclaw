import { React } from "../../../lib/html.js";
import {
  fetchInterceptorConfig,
  updateInterceptorConfig,
} from "../lib/settings-api.js";

/**
 * Manages interceptor configuration load and update.
 *
 * Returns:
 *   config         — InterceptorConfigSnapshot | null
 *   isLoading      — initial load in flight
 *   loadError      — Error | null for initial load
 *   isMutating     — any mutation in flight
 *   mutationError  — string | null last mutation error message
 *   handleUpdate   — (persona: string) => Promise<void>
 */
export function useInterceptor() {
  const [config, setConfig] = React.useState(null);
  const [isLoading, setIsLoading] = React.useState(true);
  const [loadError, setLoadError] = React.useState(null);
  const [isMutating, setIsMutating] = React.useState(false);
  const [mutationError, setMutationError] = React.useState(null);

  // Initial load.
  React.useEffect(() => {
    let cancelled = false;
    setIsLoading(true);
    setLoadError(null);
    fetchInterceptorConfig()
      .then((data) => {
        if (!cancelled) setConfig(data);
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
  }, []);

  const handleUpdate = React.useCallback(async (persona) => {
    setIsMutating(true);
    setMutationError(null);
    try {
      const updated = await updateInterceptorConfig({ persona });
      setConfig(updated);
    } catch (err) {
      setMutationError(err.message || String(err));
    } finally {
      setIsMutating(false);
    }
  }, []);

  return {
    config,
    isLoading,
    loadError,
    isMutating,
    mutationError,
    handleUpdate,
  };
}
