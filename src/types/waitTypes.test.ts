import { getWaitTypeDescription } from './waitTypes';

describe('getWaitTypeDescription', () => {
  it('returns exact-match descriptions', () => {
    expect(getWaitTypeDescription('CXPACKET')).toContain('Parallelism');
    expect(getWaitTypeDescription('SOS_SCHEDULER_YIELD')).toContain('CPU');
  });

  it('returns prefix-match descriptions for wait families', () => {
    expect(getWaitTypeDescription('LCK_M_S')).toContain('Lock wait');
    expect(getWaitTypeDescription('LCK_M_SCH_M')).toContain('Lock wait');
    expect(getWaitTypeDescription('PAGEIOLATCH_SH')).toContain('disk');
    expect(getWaitTypeDescription('PAGELATCH_EX')).toContain('In-memory');
  });

  it('returns undefined for unknown wait types', () => {
    expect(getWaitTypeDescription('SOME_EXOTIC_WAIT')).toBeUndefined();
  });
});
