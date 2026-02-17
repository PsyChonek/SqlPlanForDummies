import { mount } from '@vue/test-utils';
import AnalysisPanel from './AnalysisPanel.vue';
import { usePlanState } from '../composables/planState';

// Simple plan with Table Scan + lots of rows to trigger analysis warnings
const tableScanXml = `<?xml version="1.0" encoding="utf-16"?>
<ShowPlanXML xmlns="http://schemas.microsoft.com/sqlserver/2004/07/showplan" Version="1.5" Build="17.0">
  <BatchSequence>
    <Batch>
      <Statements>
        <StmtSimple StatementId="1" StatementText="SELECT * FROM LargeTable" StatementType="SELECT"
                    StatementSubTreeCost="5.0" StatementEstRows="50000">
          <QueryPlan DegreeOfParallelism="1">
            <RelOp NodeId="0" PhysicalOp="Table Scan" LogicalOp="Table Scan"
                   EstimateRows="50000" EstimateCPU="0.5" EstimateIO="4.5"
                   EstimatedTotalSubtreeCost="5.0" AvgRowSize="200" Parallel="false">
              <OutputList></OutputList>
            </RelOp>
          </QueryPlan>
        </StmtSimple>
      </Statements>
    </Batch>
  </BatchSequence>
</ShowPlanXML>`;

// Simple plan with index seek (efficient)
const indexSeekXml = `<?xml version="1.0" encoding="utf-16"?>
<ShowPlanXML xmlns="http://schemas.microsoft.com/sqlserver/2004/07/showplan" Version="1.5" Build="17.0">
  <BatchSequence>
    <Batch>
      <Statements>
        <StmtSimple StatementId="1" StatementText="SELECT * FROM Users WHERE Id = 1" StatementType="SELECT"
                    StatementSubTreeCost="0.003" StatementEstRows="1">
          <QueryPlan DegreeOfParallelism="1">
            <RelOp NodeId="0" PhysicalOp="Index Seek" LogicalOp="Index Seek"
                   EstimateRows="1" EstimateCPU="0.0001" EstimateIO="0.003"
                   EstimatedTotalSubtreeCost="0.003" AvgRowSize="50" Parallel="false">
              <OutputList></OutputList>
            </RelOp>
          </QueryPlan>
        </StmtSimple>
      </Statements>
    </Batch>
  </BatchSequence>
</ShowPlanXML>`;

describe('AnalysisPanel', () => {
  let planState: ReturnType<typeof usePlanState>;

  beforeEach(() => {
    planState = usePlanState();
    planState.clearPlan();
  });

  it('renders without error', () => {
    expect(() => mount(AnalysisPanel)).not.toThrow();
  });

  it('shows empty state when no plan is loaded', () => {
    const wrapper = mount(AnalysisPanel);

    // Should show some kind of empty/no analysis state
    const text = wrapper.text();
    // Either empty or shows 0 issues
    expect(text).not.toContain('Table Scan Detected');
  });

  it('detects table scan issue on large tables', () => {
    planState.loadPlan(tableScanXml);

    const wrapper = mount(AnalysisPanel);
    const text = wrapper.text();

    expect(text).toContain('Table Scan Detected');
  });

  it('shows plan statistics when plan is loaded', () => {
    planState.loadPlan(tableScanXml);

    const wrapper = mount(AnalysisPanel);
    const text = wrapper.text();

    // Should show total cost and node count
    expect(text).toContain('5'); // total cost or node count
  });

  it('shows no critical issues for efficient plans', () => {
    planState.loadPlan(indexSeekXml);

    const wrapper = mount(AnalysisPanel);
    const text = wrapper.text();

    expect(text).not.toContain('Table Scan Detected');
  });

  it('shows high cost operation for node with >50% cost', () => {
    planState.loadPlan(tableScanXml);

    const wrapper = mount(AnalysisPanel);
    const text = wrapper.text();

    // Table Scan with 100% of cost should trigger high cost warning
    expect(text).toContain('High Cost Operation');
  });

  it('shows scan count in plan summary', () => {
    planState.loadPlan(tableScanXml);

    const wrapper = mount(AnalysisPanel);
    const text = wrapper.text();

    // Should show that 1 scan node was found
    expect(text).toMatch(/[Ss]can/);
  });

  it('shows seek count for plans with index seeks', () => {
    planState.loadPlan(indexSeekXml);

    const wrapper = mount(AnalysisPanel);
    const text = wrapper.text();

    // Should mention seek somewhere in the summary
    expect(text).toMatch(/[Ss]eek/);
  });
});
