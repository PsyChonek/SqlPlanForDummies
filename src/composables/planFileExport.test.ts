vi.mock('./tauriApi', () => ({
  tauriInvoke: vi.fn(),
}));

import { tauriInvoke } from './tauriApi';
import { savePlanFile, suggestedPlanFileName } from './planFileExport';

describe('suggestedPlanFileName', () => {
  it('builds a zero-padded timestamped .sqlplan name', () => {
    expect(suggestedPlanFileName(new Date(2026, 0, 5, 9, 3, 7))).toBe('plan-20260105-090307.sqlplan');
  });
});

describe('savePlanFile', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('passes the XML and file name to the backend', async () => {
    vi.mocked(tauriInvoke).mockResolvedValueOnce('C:\plans\a.sqlplan');

    const path = await savePlanFile('<ShowPlanXML/>', 'a.sqlplan');

    expect(tauriInvoke).toHaveBeenCalledWith('save_plan_file', { xml: '<ShowPlanXML/>', suggestedName: 'a.sqlplan' });
    expect(path).toBe('C:\plans\a.sqlplan');
  });

  it('resolves to null when the dialog is cancelled', async () => {
    vi.mocked(tauriInvoke).mockResolvedValueOnce(null);

    expect(await savePlanFile('<ShowPlanXML/>')).toBeNull();
  });
});
