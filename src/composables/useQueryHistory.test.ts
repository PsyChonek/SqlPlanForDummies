// Mock the tauriApi module before importing useQueryHistory
vi.mock('./tauriApi', () => ({
  tauriInvoke: vi.fn().mockResolvedValue(undefined),
}));

import { useQueryHistory, type QueryHistoryEntry, type PlanHistoryEntry } from './useQueryHistory';

function makeQueryEntry(overrides: Partial<QueryHistoryEntry> = {}): QueryHistoryEntry {
  return {
    id: Math.random().toString(36).slice(2),
    sql: 'SELECT * FROM Users',
    connectionId: 'conn-1',
    connectionName: 'Local Dev',
    executedAt: new Date().toISOString(),
    durationMs: 100,
    success: true,
    error: null,
    ...overrides,
  };
}

function makePlanEntry(overrides: Partial<PlanHistoryEntry> = {}): PlanHistoryEntry {
  return {
    id: Math.random().toString(36).slice(2),
    queryId: Math.random().toString(36).slice(2),
    planXml: '<ShowPlanXML/>',
    planType: 'Actual',
    executedAt: new Date().toISOString(),
    connectionId: 'conn-1',
    sqlPreview: 'SELECT * FROM Users',
    ...overrides,
  };
}

describe('useQueryHistory', () => {
  let history: ReturnType<typeof useQueryHistory>;

  beforeEach(() => {
    history = useQueryHistory();
    // Reset state between tests (shared singleton)
    history.state.queries = [];
    history.state.plans = [];
    history.state.searchTerm = '';
    history.state.loaded = false;
    vi.clearAllMocks();
  });

  describe('addQueryEntry', () => {
    it('adds entry to front of queries list', async () => {
      const entry = makeQueryEntry({ sql: 'SELECT 1' });
      await history.addQueryEntry(entry);

      expect(history.state.queries).toHaveLength(1);
      expect(history.state.queries[0]).toStrictEqual(entry);
    });

    it('prepends new entries (most recent first)', async () => {
      const entry1 = makeQueryEntry({ sql: 'SELECT 1' });
      const entry2 = makeQueryEntry({ sql: 'SELECT 2' });

      await history.addQueryEntry(entry1);
      await history.addQueryEntry(entry2);

      expect(history.state.queries[0].sql).toBe('SELECT 2'); // most recent first
      expect(history.state.queries[1].sql).toBe('SELECT 1');
    });

    it('trims to 100 entries maximum', async () => {
      // Add 101 entries
      for (let i = 0; i < 101; i++) {
        await history.addQueryEntry(makeQueryEntry({ sql: `SELECT ${i}` }));
      }

      expect(history.state.queries).toHaveLength(100);
    });

    it('keeps the most recent 100 entries when trimming', async () => {
      // Add 101 entries where the last one is identifiable
      for (let i = 0; i < 101; i++) {
        await history.addQueryEntry(makeQueryEntry({ sql: `SELECT ${i}` }));
      }

      // The most recent (SELECT 100) should be at index 0
      expect(history.state.queries[0].sql).toBe('SELECT 100');
      // Entry at index 99 should be SELECT 1 (oldest retained)
      expect(history.state.queries[99].sql).toBe('SELECT 1');
    });

    it('does not trim when count is exactly 100', async () => {
      for (let i = 0; i < 100; i++) {
        await history.addQueryEntry(makeQueryEntry({ sql: `SELECT ${i}` }));
      }

      expect(history.state.queries).toHaveLength(100);
    });

    it('calls tauriInvoke to persist the entry', async () => {
      const { tauriInvoke } = await import('./tauriApi');
      const entry = makeQueryEntry();

      await history.addQueryEntry(entry);

      expect(tauriInvoke).toHaveBeenCalledWith('save_query_history_entry', { entry });
    });
  });

  describe('addPlanEntry', () => {
    it('adds entry to front of plans list', async () => {
      const entry = makePlanEntry();
      await history.addPlanEntry(entry);

      expect(history.state.plans).toHaveLength(1);
      expect(history.state.plans[0]).toStrictEqual(entry);
    });

    it('prepends new entries (most recent first)', async () => {
      const entry1 = makePlanEntry({ sqlPreview: 'SELECT 1' });
      const entry2 = makePlanEntry({ sqlPreview: 'SELECT 2' });

      await history.addPlanEntry(entry1);
      await history.addPlanEntry(entry2);

      expect(history.state.plans[0].sqlPreview).toBe('SELECT 2'); // most recent first
      expect(history.state.plans[1].sqlPreview).toBe('SELECT 1');
    });

    it('trims to 50 entries maximum', async () => {
      for (let i = 0; i < 51; i++) {
        await history.addPlanEntry(makePlanEntry({ sqlPreview: `SELECT ${i}` }));
      }

      expect(history.state.plans).toHaveLength(50);
    });

    it('keeps the most recent 50 entries when trimming', async () => {
      for (let i = 0; i < 51; i++) {
        await history.addPlanEntry(makePlanEntry({ sqlPreview: `SELECT ${i}` }));
      }

      // Most recent (SELECT 50) should be first
      expect(history.state.plans[0].sqlPreview).toBe('SELECT 50');
      // Entry at index 49 should be SELECT 1 (oldest retained)
      expect(history.state.plans[49].sqlPreview).toBe('SELECT 1');
    });

    it('calls tauriInvoke to persist the entry', async () => {
      const { tauriInvoke } = await import('./tauriApi');
      const entry = makePlanEntry();

      await history.addPlanEntry(entry);

      expect(tauriInvoke).toHaveBeenCalledWith('save_plan_history_entry', { entry });
    });
  });

  describe('filteredQueries', () => {
    beforeEach(async () => {
      await history.addQueryEntry(makeQueryEntry({ sql: 'SELECT * FROM Users' }));
      await history.addQueryEntry(makeQueryEntry({ sql: 'SELECT * FROM Orders WHERE id = 1' }));
      await history.addQueryEntry(makeQueryEntry({ sql: 'UPDATE Products SET price = 10' }));
    });

    it('returns all queries when search term is empty', () => {
      history.state.searchTerm = '';
      expect(history.filteredQueries.value).toHaveLength(3);
    });

    it('filters queries by search term', () => {
      history.state.searchTerm = 'users';
      const results = history.filteredQueries.value;

      expect(results).toHaveLength(1);
      expect(results[0].sql).toBe('SELECT * FROM Users');
    });

    it('is case-insensitive', () => {
      history.state.searchTerm = 'ORDERS';
      const results = history.filteredQueries.value;

      expect(results).toHaveLength(1);
      expect(results[0].sql).toContain('Orders');
    });

    it('returns empty when no matches found', () => {
      history.state.searchTerm = 'nonexistent_table';
      expect(history.filteredQueries.value).toHaveLength(0);
    });

    it('matches partial search terms', () => {
      history.state.searchTerm = 'SELECT';
      const results = history.filteredQueries.value;

      expect(results).toHaveLength(2); // Users and Orders queries
    });
  });

  describe('getPlansForQuery', () => {
    beforeEach(async () => {
      await history.addPlanEntry(makePlanEntry({ sqlPreview: 'SELECT * FROM Users', planType: 'Estimated' }));
      await history.addPlanEntry(makePlanEntry({ sqlPreview: 'SELECT * FROM Users', planType: 'Actual' }));
      await history.addPlanEntry(makePlanEntry({ sqlPreview: 'SELECT * FROM Orders' }));
    });

    it('returns plans matching exact SQL preview', () => {
      const plans = history.getPlansForQuery('SELECT * FROM Users');
      expect(plans).toHaveLength(2);
    });

    it('returns empty array when no matching plans', () => {
      const plans = history.getPlansForQuery('SELECT * FROM NonExistent');
      expect(plans).toHaveLength(0);
    });

    it('requires exact match (not partial)', () => {
      const plans = history.getPlansForQuery('SELECT * FROM');
      expect(plans).toHaveLength(0);
    });
  });

  describe('recentPlans', () => {
    it('returns up to 20 most recent plans', async () => {
      for (let i = 0; i < 25; i++) {
        await history.addPlanEntry(makePlanEntry({ sqlPreview: `SELECT ${i}` }));
      }

      expect(history.recentPlans.value).toHaveLength(20);
    });

    it('returns first 20 (most recent)', async () => {
      for (let i = 0; i < 25; i++) {
        await history.addPlanEntry(makePlanEntry({ sqlPreview: `SELECT ${i}` }));
      }

      expect(history.recentPlans.value[0].sqlPreview).toBe('SELECT 24'); // most recent
    });

    it('returns all plans when fewer than 20', async () => {
      await history.addPlanEntry(makePlanEntry());
      await history.addPlanEntry(makePlanEntry());

      expect(history.recentPlans.value).toHaveLength(2);
    });

    it('returns empty array when no plans', () => {
      expect(history.recentPlans.value).toHaveLength(0);
    });
  });
});
