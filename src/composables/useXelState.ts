import { reactive, computed } from 'vue';
import type { XelEvent, XelSessionStats, XelFile, XelFilter } from '../types/xel';
import { emptyFilter } from '../types/xel';

export type XelView = 'table' | 'timeline' | 'lockchain' | 'dashboard';

interface XelState {
  files: XelFile[];
  loading: boolean;
  error: string | null;
  stats: XelSessionStats | null;
  selectedEvent: XelEvent | null;
  selectedSessionId: number | null;
  activeView: XelView;
  filter: XelFilter;
  revision: number;
  // Loading progress per file
  loadingProgress: Map<string, number>;
}

const state = reactive<XelState>({
  files: [],
  loading: false,
  error: null,
  stats: null,
  selectedEvent: null,
  selectedSessionId: null,
  activeView: 'table',
  filter: emptyFilter(),
  revision: 0,
  loadingProgress: new Map(),
});

export const useXelState = () => {
  const addFile = (file: XelFile) => {
    if (!state.files.some(f => f.path === file.path)) {
      state.files.push(file);
    }
  };

  const removeFile = (path: string) => {
    state.files = state.files.filter(f => f.path !== path);
  };

  const setStats = (stats: XelSessionStats) => {
    state.stats = stats;
    state.revision++;
  };

  const selectEvent = (event: XelEvent | null) => {
    state.selectedEvent = event;
  };

  const selectSession = (sessionId: number | null) => {
    state.selectedSessionId = sessionId;
    if (sessionId !== null) {
      state.filter.sessionIds = [sessionId];
    } else {
      state.filter.sessionIds = [];
    }
    state.revision++;
  };

  const setActiveView = (view: XelView) => {
    state.activeView = view;
  };

  const setFilter = (filter: Partial<XelFilter>) => {
    Object.assign(state.filter, filter);
    state.revision++;
  };

  const clearFilter = () => {
    state.filter = emptyFilter();
    state.selectedSessionId = null;
    state.revision++;
  };

  const clearAll = () => {
    state.files = [];
    state.stats = null;
    state.selectedEvent = null;
    state.selectedSessionId = null;
    state.filter = emptyFilter();
    state.error = null;
    state.revision++;
  };

  const setLoading = (loading: boolean) => {
    state.loading = loading;
  };


  const setError = (error: string | null) => {
    state.error = error;
  };

  const setFileProgress = (fileName: string, progress: number) => {
    state.loadingProgress.set(fileName, progress);
  };

  // Computed
  const hasData = computed(() => state.stats !== null && state.stats.totalEvents > 0);

  const eventTypes = computed(() => {
    if (!state.stats) return [];
    return Object.entries(state.stats.eventTypeCounts)
      .sort((a, b) => b[1] - a[1])
      .map(([name, count]) => ({ name, count }));
  });

  const sessions = computed(() => state.stats?.uniqueSessions ?? []);

  const timeRange = computed(() => {
    if (!state.stats?.timeRangeStart || !state.stats?.timeRangeEnd) return null;
    return {
      start: new Date(state.stats.timeRangeStart),
      end: new Date(state.stats.timeRangeEnd),
    };
  });

  const hasActiveFilters = computed(() => {
    const f = state.filter;
    return (
      f.eventNames.length > 0 ||
      f.sessionIds.length > 0 ||
      f.textSearch !== null ||
      f.objectNameContains !== null ||
      f.sqlTextContains !== null ||
      f.username !== null ||
      f.clientAppName !== null ||
      f.databaseName !== null ||
      f.minDurationUs !== null ||
      f.maxDurationUs !== null ||
      f.timeFrom !== null ||
      f.timeTo !== null ||
      f.result !== null ||
      f.errorsOnly ||
      f.deadlocksOnly
    );
  });

  return {
    state,
    addFile,
    removeFile,
    setStats,
    selectEvent,
    selectSession,
    setActiveView,
    setFilter,
    clearFilter,
    clearAll,
    setLoading,
    setError,
    setFileProgress,
    hasData,
    eventTypes,
    sessions,
    timeRange,
    hasActiveFilters,
  };
};
