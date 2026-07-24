import { useQuery } from "@tanstack/react-query";

// Polling interval for mission detail (ms).
const MISSION_DETAIL_REFETCH_INTERVAL_MS = 5_000;
import { fetchMissionDetail } from "../lib/missions-api.js";

export function useMissionDetail(missionId) {
  const query = useQuery({
    queryKey: ["mission-detail", missionId],
    queryFn: () => fetchMissionDetail(missionId),
    enabled: Boolean(missionId),
    refetchInterval: missionId ? MISSION_DETAIL_REFETCH_INTERVAL_MS : false,
  });

  return {
    mission: query.data?.mission || null,
    isLoading: query.isLoading,
    isRefreshing: query.isFetching,
    error: query.error || null,
  };
}
