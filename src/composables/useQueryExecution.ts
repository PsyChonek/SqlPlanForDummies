import { reactive } from 'vue';
import { tauriInvoke } from './tauriApi';

export type PlanType = 'None' | 'Estimated' | 'Actual';

export interface QueryResult {
  columns: string[];
  rows: any[][];
  messages: string[];
  planXml: string | null;
  durationMs: number;
  rowsAffected: number;
}

export interface QueryResultTab {
  id: string;
  query: string;
  result: QueryResult | null;
  error: string | null;
  timestamp: Date;
  duration: number;
  planType: PlanType;
}

interface ExecutionState {
  executing: boolean;
  results: QueryResultTab[];
  activeResultTab: number;
}

const state = reactive<ExecutionState>({
  executing: false,
  results: [],
  activeResultTab: 0,
});

export const useQueryExecution = () => {
  const executeQuery = async (sql: string, planType: PlanType) => {
    state.executing = true;
    const timeout = Number(import.meta.env.VITE_QUERY_TIMEOUT) || 30000;

    try {
      const result = await tauriInvoke<QueryResult>('execute_query', {
        request: {
          sql,
          timeoutSeconds: Math.floor(timeout / 1000),
          planType,
        },
      });

      const tab: QueryResultTab = {
        id: crypto.randomUUID(),
        query: sql.substring(0, 200),
        result,
        error: null,
        timestamp: new Date(),
        duration: result.durationMs,
        planType,
      };

      state.results.push(tab);
      state.activeResultTab = state.results.length - 1;

      return result;
    } catch (e) {
      const tab: QueryResultTab = {
        id: crypto.randomUUID(),
        query: sql.substring(0, 200),
        result: null,
        error: String(e),
        timestamp: new Date(),
        duration: 0,
        planType,
      };

      state.results.push(tab);
      state.activeResultTab = state.results.length - 1;
      throw e;
    } finally {
      state.executing = false;
    }
  };

  const closeResultTab = (index: number) => {
    state.results.splice(index, 1);
    if (state.activeResultTab >= state.results.length) {
      state.activeResultTab = Math.max(0, state.results.length - 1);
    }
  };

  const clearResults = () => {
    state.results = [];
    state.activeResultTab = 0;
  };

  return { state, executeQuery, closeResultTab, clearResults };
};
