import { React } from "../../../lib/html.js";
import {
  fetchInterceptorConfig,
  updateInterceptorConfig,
  reassembleInterceptor,
  prewarmInterceptor,
} from "../lib/settings-api.js";

/**
 * Manages interceptor configuration load, update, reassemble, and pre-warm.
 *
 * Returns:
 *   config         — InterceptorConfigSnapshot | null
 *   isLoading      — initial load in flight
 *   loadError      — Error | null for initial load
 *   isMutating     — any mutation in flight
 *   mutationError  — string | null last mutation error message
 *   actionStatus   — { reassemble: string, prewarm: string }  ("" | "ok" | "error")
 *   handleUpdate   — (persona: string) => Promise<void>
 *   handleReassemble — () => Promise<void>
 *   handlePrewarm  — () => Promise<void>
 */
export function useInterceptor() {
  const [config, setConfig] = React.useState(null);
  const [isLoading, setIsLoading] = React.useState(true);
  const [loadError, setLoadError] = React.useState(null);
  const [isMutating, setIsMutating] = React.useState(false);
  const [mutationError, setMutationError] = React.useState(null);
  const [actionStatus, setActionStatus] = React.useState({
    reassemble: "",
    prewarm: "",
  });

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

  const handleReassemble = React.useCallback(async () => {
    setIsMutating(true);
    setMutationError(null);
    setActionStatus((s) => ({ ...s, reassemble: "" }));
    try {
      const updated = await reassembleInterceptor();
      setConfig(updated);
      setActionStatus((s) => ({ ...s, reassemble: "ok" }));
    } catch (err) {
      setMutationError(err.message || String(err));
      setActionStatus((s) => ({ ...s, reassemble: "error" }));
    } finally {
      setIsMutating(false);
    }
  }, []);

  const handlePrewarm = React.useCallback(async () => {
    setIsMutating(true);
    setMutationError(null);
    setActionStatus((s) => ({ ...s, prewarm: "" }));
    try {
      const updated = await prewarmInterceptor();
      setConfig(updated);
      setActionStatus((s) => ({ ...s, prewarm: "ok" }));
    } catch (err) {
      setMutationError(err.message || String(err));
      setActionStatus((s) => ({ ...s, prewarm: "error" }));
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
    actionStatus,
    handleUpdate,
    handleReassemble,
    handlePrewarm,
  };
}
