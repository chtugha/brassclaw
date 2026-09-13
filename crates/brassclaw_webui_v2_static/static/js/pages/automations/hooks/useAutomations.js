import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { React } from "../../../lib/html.js";
import {
  listAutomations,
  createAutomation,
  deleteAutomation,
  setAutomationState,
  fireAutomationNow,
} from "../../../lib/api.js";

import {
  automationSummary,
  normalizeAutomations,
} from "../lib/automations-presenters.js";

const AUTOMATIONS_PAGE_LIMIT = 50;

export function useAutomations() {
  const queryClient = useQueryClient();

  const query = useQuery({
    queryKey: ["automations"],
    queryFn: () => listAutomations({ limit: AUTOMATIONS_PAGE_LIMIT }),
    refetchInterval: 30000,
    refetchIntervalInBackground: false,
  });

  const automations = React.useMemo(
    () => normalizeAutomations(query.data),
    [query.data]
  );
  const summary = React.useMemo(
    () => automationSummary(automations),
    [automations]
  );

  const invalidate = () =>
    queryClient.invalidateQueries({ queryKey: ["automations"] });

  const create = useMutation({
    mutationFn: createAutomation,
    onSuccess: invalidate,
  });

  const remove = useMutation({
    mutationFn: (id) => deleteAutomation(id),
    onSuccess: invalidate,
  });

  const setState = useMutation({
    mutationFn: ({ id, action }) => setAutomationState(id, action),
    onSuccess: invalidate,
  });

  const fire = useMutation({
    mutationFn: (id) => fireAutomationNow(id),
    onSuccess: invalidate,
  });

  return {
    automations,
    summary,
    isLoading: query.isLoading,
    isRefreshing: query.isFetching,
    error: query.error || null,
    refetch: query.refetch,
    create,
    remove,
    setState,
    fire,
  };
}
