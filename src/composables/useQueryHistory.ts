import { reactive, computed } from 'vue';
import { tauriInvoke } from './tauriApi';

export interface QueryHistoryEntry {
  id: string;
  sql: string;
  connectionId: string;
  connectionName: string;
  executedAt: string;
  durationMs: number;
  success: boolean;
  error: string | null;
}

export interface PlanHistoryEntry {
  id: string;
  queryId: string;
  planXml: string;
  planType: string;
  executedAt: string;
  connectionId: string;
  sqlPreview: string;
}

interface HistoryState {
  queries: QueryHistoryEntry[];
  plans: PlanHistoryEntry[];
  searchTerm: string;
  loaded: boolean;
}

const state = reactive<HistoryState>({
  queries: [],
  plans: [],
  searchTerm: '',
  loaded: false,
});

export const useQueryHistory = () => {
  const loadHistory = async () => {
    if (state.loaded) return;
    try {
      const [queries, plans] = await Promise.all([
        tauriInvoke<QueryHistoryEntry[]>('get_query_history'),
        tauriInvoke<PlanHistoryEntry[]>('get_plan_history'),
      ]);
      state.queries = queries;
      state.plans = plans;
      state.loaded = true;
    } catch (e) {
      console.error('Failed to load history:', e);
    }
  };

  const addQueryEntry = async (entry: QueryHistoryEntry) => {
    state.queries.unshift(entry);
    if (state.queries.length > 100) {
      state.queries = state.queries.slice(0, 100);
    }
    try {
      await tauriInvoke('save_query_history_entry', { entry });
    } catch (e) {
      console.error('Failed to save query history:', e);
    }
  };

  const addPlanEntry = async (entry: PlanHistoryEntry) => {
    state.plans.unshift(entry);
    if (state.plans.length > 50) {
      state.plans = state.plans.slice(0, 50);
    }
    try {
      await tauriInvoke('save_plan_history_entry', { entry });
    } catch (e) {
      console.error('Failed to save plan history:', e);
    }
  };

  const filteredQueries = computed(() => {
    if (!state.searchTerm) return state.queries;
    const term = state.searchTerm.toLowerCase();
    return state.queries.filter((q) => q.sql.toLowerCase().includes(term));
  });

  const getPlansForQuery = (sqlPreview: string) => {
    return state.plans.filter((p) => p.sqlPreview === sqlPreview);
  };

  const recentPlans = computed(() => {
    return state.plans.slice(0, 20);
  });

  return {
    state,
    loadHistory,
    addQueryEntry,
    addPlanEntry,
    filteredQueries,
    getPlansForQuery,
    recentPlans,
  };
};
