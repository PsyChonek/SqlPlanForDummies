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

// Actual plan where estimates are tiny and actuals are 0 - must NOT warn (noise)
const tinyMismatchXml = `<?xml version="1.0" encoding="utf-16"?>
<ShowPlanXML xmlns="http://schemas.microsoft.com/sqlserver/2004/07/showplan" Version="1.5" Build="17.0">
  <BatchSequence>
    <Batch>
      <Statements>
        <StmtSimple StatementId="1" StatementText="SELECT * FROM Users WHERE Id = 1" StatementType="SELECT"
                    StatementSubTreeCost="0.003" StatementEstRows="2">
          <QueryPlan DegreeOfParallelism="1">
            <RelOp NodeId="0" PhysicalOp="Index Seek" LogicalOp="Index Seek"
                   EstimateRows="2" EstimateCPU="0.0001" EstimateIO="0.003"
                   EstimatedTotalSubtreeCost="0.003" AvgRowSize="50" Parallel="false">
              <OutputList></OutputList>
              <RunTimeInformation>
                <RunTimeCountersPerThread Thread="0" ActualRows="0" ActualExecutions="1"
                                          ActualElapsedms="0" ActualCPUms="0" />
              </RunTimeInformation>
            </RelOp>
          </QueryPlan>
        </StmtSimple>
      </Statements>
    </Batch>
  </BatchSequence>
</ShowPlanXML>`;

// Actual plan with a large underestimate - must warn
const largeMismatchXml = `<?xml version="1.0" encoding="utf-16"?>
<ShowPlanXML xmlns="http://schemas.microsoft.com/sqlserver/2004/07/showplan" Version="1.5" Build="17.0">
  <BatchSequence>
    <Batch>
      <Statements>
        <StmtSimple StatementId="1" StatementText="SELECT * FROM Orders WHERE Status = 1" StatementType="SELECT"
                    StatementSubTreeCost="0.5" StatementEstRows="100">
          <QueryPlan DegreeOfParallelism="1">
            <RelOp NodeId="0" PhysicalOp="Index Seek" LogicalOp="Index Seek"
                   EstimateRows="100" EstimateCPU="0.01" EstimateIO="0.05"
                   EstimatedTotalSubtreeCost="0.5" AvgRowSize="50" Parallel="false">
              <OutputList></OutputList>
              <RunTimeInformation>
                <RunTimeCountersPerThread Thread="0" ActualRows="250000" ActualExecutions="1"
                                          ActualElapsedms="900" ActualCPUms="800" />
              </RunTimeInformation>
            </RelOp>
          </QueryPlan>
        </StmtSimple>
      </Statements>
    </Batch>
  </BatchSequence>
</ShowPlanXML>`;

// Nested loops inner side: estimate is per execution, so 10 rows x 500 executions
// matching ActualRows=5000 is NOT a mismatch
const perExecutionXml = `<?xml version="1.0" encoding="utf-16"?>
<ShowPlanXML xmlns="http://schemas.microsoft.com/sqlserver/2004/07/showplan" Version="1.5" Build="17.0">
  <BatchSequence>
    <Batch>
      <Statements>
        <StmtSimple StatementId="1" StatementText="SELECT ..." StatementType="SELECT"
                    StatementSubTreeCost="0.5" StatementEstRows="5000">
          <QueryPlan DegreeOfParallelism="1">
            <RelOp NodeId="0" PhysicalOp="Index Seek" LogicalOp="Index Seek"
                   EstimateRows="10" EstimateCPU="0.01" EstimateIO="0.05"
                   EstimatedTotalSubtreeCost="0.5" AvgRowSize="50" Parallel="false">
              <OutputList></OutputList>
              <RunTimeInformation>
                <RunTimeCountersPerThread Thread="0" ActualRows="5000" ActualExecutions="500"
                                          ActualElapsedms="10" ActualCPUms="10" />
              </RunTimeInformation>
            </RelOp>
          </QueryPlan>
        </StmtSimple>
      </Statements>
    </Batch>
  </BatchSequence>
</ShowPlanXML>`;

// Plan with SQL Server's own warnings and a missing index suggestion
const serverWarningsXml = `<?xml version="1.0" encoding="utf-16"?>
<ShowPlanXML xmlns="http://schemas.microsoft.com/sqlserver/2004/07/showplan" Version="1.5" Build="17.0">
  <BatchSequence>
    <Batch>
      <Statements>
        <StmtSimple StatementId="1" StatementText="SELECT * FROM Orders" StatementType="SELECT"
                    StatementSubTreeCost="1.0" StatementEstRows="100">
          <QueryPlan DegreeOfParallelism="1">
            <MissingIndexes>
              <MissingIndexGroup Impact="92.1">
                <MissingIndex Database="[MyDb]" Schema="[dbo]" Table="[Orders]">
                  <ColumnGroup Usage="EQUALITY">
                    <Column Name="[UserId]" ColumnId="2" />
                  </ColumnGroup>
                </MissingIndex>
              </MissingIndexGroup>
            </MissingIndexes>
            <Warnings>
              <PlanAffectingConvert ConvertIssue="Seek Plan" Expression="CONVERT_IMPLICIT(nvarchar(50),[Code],0)" />
            </Warnings>
            <RelOp NodeId="0" PhysicalOp="Sort" LogicalOp="Sort"
                   EstimateRows="100" EstimateCPU="0.5" EstimateIO="0.5"
                   EstimatedTotalSubtreeCost="1.0" AvgRowSize="100" Parallel="false">
              <OutputList></OutputList>
              <Warnings>
                <SpillToTempDb SpillLevel="1" />
              </Warnings>
            </RelOp>
          </QueryPlan>
        </StmtSimple>
      </Statements>
    </Batch>
  </BatchSequence>
</ShowPlanXML>`;

describe('AnalysisPanel issue detection', () => {
  let planState: ReturnType<typeof usePlanState>;

  beforeEach(() => {
    planState = usePlanState();
    planState.clearPlan();
  });

  it('does not report estimate mismatch for tiny row counts', () => {
    planState.loadPlan(tinyMismatchXml);

    const wrapper = mount(AnalysisPanel);

    expect(wrapper.text()).not.toContain('Estimate Mismatch');
  });

  it('reports estimate mismatch for large underestimates', () => {
    planState.loadPlan(largeMismatchXml);

    const wrapper = mount(AnalysisPanel);

    expect(wrapper.text()).toContain('Estimate Mismatch');
  });

  it('accounts for actual executions when comparing estimates', () => {
    planState.loadPlan(perExecutionXml);

    const wrapper = mount(AnalysisPanel);

    expect(wrapper.text()).not.toContain('Estimate Mismatch');
  });

  it('reports tempdb spills from plan warnings', () => {
    planState.loadPlan(serverWarningsXml);

    const wrapper = mount(AnalysisPanel);

    expect(wrapper.text()).toContain('Spill to TempDb');
  });

  it('reports implicit conversions from plan warnings', () => {
    planState.loadPlan(serverWarningsXml);

    const wrapper = mount(AnalysisPanel);

    expect(wrapper.text()).toContain('Implicit Conversion');
  });

  it('reports missing index suggestions', () => {
    planState.loadPlan(serverWarningsXml);

    const wrapper = mount(AnalysisPanel);
    const text = wrapper.text();

    expect(text).toContain('Missing Index Suggested');
    expect(text).toContain('Orders');
  });
});
