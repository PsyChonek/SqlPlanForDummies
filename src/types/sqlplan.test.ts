import {
  getCostSeverity,
  getCostColor,
  formatTime,
  formatRows,
  formatCostPercentage,
  getOperatorIcon,
} from './sqlplan';

describe('getCostSeverity', () => {
  it('returns low for cost < 10%', () => {
    expect(getCostSeverity(0)).toBe('low');
    expect(getCostSeverity(5)).toBe('low');
    expect(getCostSeverity(9.9)).toBe('low');
  });

  it('returns medium for cost 10-30%', () => {
    expect(getCostSeverity(10)).toBe('medium');
    expect(getCostSeverity(15)).toBe('medium');
    expect(getCostSeverity(25)).toBe('medium');
    expect(getCostSeverity(29.9)).toBe('medium');
  });

  it('returns high for cost 30-50%', () => {
    expect(getCostSeverity(30)).toBe('high');
    expect(getCostSeverity(35)).toBe('high');
    expect(getCostSeverity(45)).toBe('high');
    expect(getCostSeverity(49.9)).toBe('high');
  });

  it('returns critical for cost >= 50%', () => {
    expect(getCostSeverity(50)).toBe('critical');
    expect(getCostSeverity(75)).toBe('critical');
    expect(getCostSeverity(100)).toBe('critical');
    expect(getCostSeverity(150)).toBe('critical');
  });

  it('handles negative values', () => {
    expect(getCostSeverity(-5)).toBe('low');
    expect(getCostSeverity(-100)).toBe('low');
  });
});

describe('getCostColor', () => {
  it('returns correct colors for each severity', () => {
    expect(getCostColor('low')).toMatch(/#[0-9a-f]{6}/i);
    expect(getCostColor('medium')).toMatch(/#[0-9a-f]{6}/i);
    expect(getCostColor('high')).toMatch(/#[0-9a-f]{6}/i);
    expect(getCostColor('critical')).toMatch(/#[0-9a-f]{6}/i);
  });

  it('returns default colors when env vars not set', () => {
    expect(getCostColor('low')).toBe('#22c55e');
    expect(getCostColor('medium')).toBe('#eab308');
    expect(getCostColor('high')).toBe('#f97316');
    expect(getCostColor('critical')).toBe('#ef4444');
  });
});

describe('formatTime', () => {
  it('formats time < 1ms', () => {
    expect(formatTime(0)).toBe('<1ms');
    expect(formatTime(0.1)).toBe('<1ms');
    expect(formatTime(0.5)).toBe('<1ms');
    expect(formatTime(0.9)).toBe('<1ms');
  });

  it('formats time < 1000ms in milliseconds', () => {
    expect(formatTime(1)).toBe('1ms');
    expect(formatTime(10)).toBe('10ms');
    expect(formatTime(50)).toBe('50ms');
    expect(formatTime(100)).toBe('100ms');
    expect(formatTime(500)).toBe('500ms');
    expect(formatTime(999)).toBe('999ms');
  });

  it('rounds milliseconds correctly', () => {
    expect(formatTime(1.4)).toBe('1ms');
    expect(formatTime(1.5)).toBe('2ms');
    expect(formatTime(1.6)).toBe('2ms');
    expect(formatTime(999.4)).toBe('999ms');
    expect(formatTime(999.6)).toBe('1000ms');
  });

  it('formats time >= 1000ms in seconds', () => {
    expect(formatTime(1000)).toBe('1.00s');
    expect(formatTime(1500)).toBe('1.50s');
    expect(formatTime(2000)).toBe('2.00s');
    expect(formatTime(5432)).toBe('5.43s');
    expect(formatTime(10000)).toBe('10.00s');
    expect(formatTime(60000)).toBe('60.00s');
  });

  it('formats large time values correctly', () => {
    expect(formatTime(100000)).toBe('100.00s');
    expect(formatTime(1000000)).toBe('1000.00s');
  });
});

describe('formatRows', () => {
  it('formats rows < 1000 as plain number', () => {
    expect(formatRows(0)).toBe('0');
    expect(formatRows(1)).toBe('1');
    expect(formatRows(10)).toBe('10');
    expect(formatRows(100)).toBe('100');
    expect(formatRows(999)).toBe('999');
  });

  it('formats rows 1K-1M with K suffix', () => {
    expect(formatRows(1000)).toBe('1.0K');
    expect(formatRows(1500)).toBe('1.5K');
    expect(formatRows(10000)).toBe('10.0K');
    expect(formatRows(50000)).toBe('50.0K');
    expect(formatRows(100000)).toBe('100.0K');
    expect(formatRows(500000)).toBe('500.0K');
    expect(formatRows(999999)).toBe('1000.0K');
  });

  it('formats rows >= 1M with M suffix', () => {
    expect(formatRows(1000000)).toBe('1.0M');
    expect(formatRows(2500000)).toBe('2.5M');
    expect(formatRows(10000000)).toBe('10.0M');
    expect(formatRows(100000000)).toBe('100.0M');
  });
});

describe('formatCostPercentage', () => {
  it('calculates percentage correctly', () => {
    expect(formatCostPercentage(10, 100)).toBe('10.0%');
    expect(formatCostPercentage(25, 100)).toBe('25.0%');
    expect(formatCostPercentage(50, 100)).toBe('50.0%');
    expect(formatCostPercentage(75, 100)).toBe('75.0%');
    expect(formatCostPercentage(100, 100)).toBe('100.0%');
  });

  it('handles decimal precision', () => {
    expect(formatCostPercentage(33.333, 100)).toBe('33.3%');
    expect(formatCostPercentage(66.666, 100)).toBe('66.7%');
    expect(formatCostPercentage(12.345, 100)).toBe('12.3%');
  });

  it('handles totalCost of 0', () => {
    expect(formatCostPercentage(10, 0)).toBe('0%');
    expect(formatCostPercentage(100, 0)).toBe('0%');
  });

  it('handles small percentages', () => {
    expect(formatCostPercentage(1, 1000)).toBe('0.1%');
    expect(formatCostPercentage(0.1, 100)).toBe('0.1%');
    expect(formatCostPercentage(0.01, 100)).toBe('0.0%');
  });

  it('handles cost > totalCost (should not happen but test anyway)', () => {
    expect(formatCostPercentage(150, 100)).toBe('150.0%');
  });

  it('handles very small values', () => {
    expect(formatCostPercentage(0.001, 1000)).toBe('0.0%');
    expect(formatCostPercentage(0, 100)).toBe('0.0%');
  });
});

describe('getOperatorIcon', () => {
  it('returns correct icons for scan operators', () => {
    expect(getOperatorIcon('Table Scan')).toBe('fa-table');
    expect(getOperatorIcon('Clustered Index Scan')).toBe('fa-database');
    expect(getOperatorIcon('Nonclustered Index Scan')).toBe('fa-database');
    expect(getOperatorIcon('Index Scan')).toBe('fa-database');
    expect(getOperatorIcon('Columnstore Index Scan')).toBe('fa-bars');
  });

  it('returns correct icons for seek operators', () => {
    expect(getOperatorIcon('Clustered Index Seek')).toBe('fa-magnifying-glass');
    expect(getOperatorIcon('Nonclustered Index Seek')).toBe('fa-magnifying-glass');
    expect(getOperatorIcon('Index Seek')).toBe('fa-magnifying-glass');
  });

  it('returns correct icons for lookup operators', () => {
    expect(getOperatorIcon('Key Lookup')).toBe('fa-key');
    expect(getOperatorIcon('RID Lookup')).toBe('fa-key');
  });

  it('returns correct icons for join operators', () => {
    expect(getOperatorIcon('Nested Loops')).toBe('fa-link');
    expect(getOperatorIcon('Hash Match')).toBe('fa-hashtag');
    expect(getOperatorIcon('Merge Join')).toBe('fa-code-merge');
    expect(getOperatorIcon('Adaptive Join')).toBe('fa-code-branch');
  });

  it('returns correct icons for aggregation operators', () => {
    expect(getOperatorIcon('Stream Aggregate')).toBe('fa-calculator');
    expect(getOperatorIcon('Hash Aggregate')).toBe('fa-calculator');
    expect(getOperatorIcon('Window Aggregate')).toBe('fa-window-maximize');
  });

  it('returns correct icons for sorting operators', () => {
    expect(getOperatorIcon('Sort')).toBe('fa-arrow-down-a-z');
    expect(getOperatorIcon('Top Sort')).toBe('fa-arrow-up-1-9');
    expect(getOperatorIcon('Top')).toBe('fa-arrow-up');
  });

  it('returns correct icon for filter operator', () => {
    expect(getOperatorIcon('Filter')).toBe('fa-filter');
  });

  it('returns correct icons for parallelism operators', () => {
    expect(getOperatorIcon('Parallelism')).toBe('fa-network-wired');
    expect(getOperatorIcon('Distribute Streams')).toBe('fa-arrows-split-up-and-left');
    expect(getOperatorIcon('Gather Streams')).toBe('fa-compress');
    expect(getOperatorIcon('Repartition Streams')).toBe('fa-shuffle');
  });

  it('returns correct icons for spool operators', () => {
    expect(getOperatorIcon('Table Spool')).toBe('fa-memory');
    expect(getOperatorIcon('Index Spool')).toBe('fa-memory');
    expect(getOperatorIcon('Eager Spool')).toBe('fa-memory');
    expect(getOperatorIcon('Lazy Spool')).toBe('fa-memory');
  });

  it('returns correct icons for DML operators', () => {
    expect(getOperatorIcon('Insert')).toBe('fa-plus');
    expect(getOperatorIcon('Update')).toBe('fa-pen');
    expect(getOperatorIcon('Delete')).toBe('fa-trash');
    expect(getOperatorIcon('Table Insert')).toBe('fa-plus');
    expect(getOperatorIcon('Table Update')).toBe('fa-pen');
    expect(getOperatorIcon('Table Delete')).toBe('fa-trash');
  });

  it('returns correct icons for miscellaneous operators', () => {
    expect(getOperatorIcon('Compute Scalar')).toBe('fa-square-root-variable');
    expect(getOperatorIcon('Concatenation')).toBe('fa-object-group');
    expect(getOperatorIcon('Constant Scan')).toBe('fa-circle-dot');
    expect(getOperatorIcon('Sequence')).toBe('fa-list-ol');
    expect(getOperatorIcon('Assert')).toBe('fa-circle-exclamation');
  });

  it('returns default icon for unknown operators', () => {
    expect(getOperatorIcon('Unknown Operator')).toBe('fa-circle-nodes');
    expect(getOperatorIcon('Custom Op')).toBe('fa-circle-nodes');
    expect(getOperatorIcon('')).toBe('fa-circle-nodes');
  });
});
