import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  getAutomation,
  updateAutomation,
  getAutomationRunHistory,
} from "../../../lib/api.js";

const RUN_HISTORY_LIMIT = 20;

export function useAutomationDetail(automationId) {
  const queryClient = useQueryClient();

  const detailQuery = useQuery({
    queryKey: ["automations", automationId],
    queryFn: () => getAutomation(automationId),
    enabled: !!automationId,
  });

  const runsQuery = useQuery({
    queryKey: ["automations", automationId, "runs"],
    queryFn: () =>
      getAutomationRunHistory(automationId, { limit: RUN_HISTORY_LIMIT }),
    enabled: !!automationId,
  });

  const update = useMutation({
    mutationFn: (patch) => updateAutomation(automationId, patch),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["automations"] });
      queryClient.invalidateQueries({
        queryKey: ["automations", automationId],
      });
    },
  });

  return {
    automation: detailQuery.data?.automation ?? null,
    isLoadingDetail: detailQuery.isLoading,
    detailError: detailQuery.error ?? null,
    runs: runsQuery.data?.runs ?? [],
    isLoadingRuns: runsQuery.isLoading,
    update,
  };
}
