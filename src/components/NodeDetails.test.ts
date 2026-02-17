import { mount } from '@vue/test-utils';
import NodeDetails from './NodeDetails.vue';
import { usePlanState } from '../composables/planState';

const simpleXml = `<?xml version="1.0" encoding="utf-16"?>
<ShowPlanXML xmlns="http://schemas.microsoft.com/sqlserver/2004/07/showplan" Version="1.5" Build="17.0">
  <BatchSequence>
    <Batch>
      <Statements>
        <StmtSimple StatementId="1" StatementText="SELECT * FROM Users" StatementType="SELECT"
                    StatementSubTreeCost="0.01" StatementEstRows="100">
          <QueryPlan DegreeOfParallelism="1">
            <RelOp NodeId="0" PhysicalOp="Table Scan" LogicalOp="Table Scan"
                   EstimateRows="100" EstimateCPU="0.001" EstimateIO="0.009"
                   EstimatedTotalSubtreeCost="0.01" AvgRowSize="50" Parallel="false">
              <OutputList>
                <ColumnReference Database="[TestDB]" Schema="[dbo]" Table="[Users]" Column="UserId" />
              </OutputList>
            </RelOp>
          </QueryPlan>
        </StmtSimple>
      </Statements>
    </Batch>
  </BatchSequence>
</ShowPlanXML>`;

const nestedXml = `<?xml version="1.0" encoding="utf-16"?>
<ShowPlanXML xmlns="http://schemas.microsoft.com/sqlserver/2004/07/showplan" Version="1.5" Build="17.0">
  <BatchSequence>
    <Batch>
      <Statements>
        <StmtSimple StatementId="1" StatementText="SELECT u.Id FROM Users u JOIN Orders o ON u.Id = o.UserId" StatementType="SELECT"
                    StatementSubTreeCost="0.1" StatementEstRows="500">
          <QueryPlan DegreeOfParallelism="1">
            <RelOp NodeId="0" PhysicalOp="Nested Loops" LogicalOp="Join"
                   EstimateRows="500" EstimateCPU="0.001" EstimateIO="0.0"
                   EstimatedTotalSubtreeCost="0.1" AvgRowSize="100" Parallel="false">
              <OutputList></OutputList>
              <RelOp NodeId="1" PhysicalOp="Index Seek" LogicalOp="Index Seek"
                     EstimateRows="100" EstimateCPU="0.001" EstimateIO="0.003"
                     EstimatedTotalSubtreeCost="0.004" AvgRowSize="50" Parallel="false">
                <OutputList></OutputList>
              </RelOp>
              <RelOp NodeId="2" PhysicalOp="Index Scan" LogicalOp="Index Scan"
                     EstimateRows="1000" EstimateCPU="0.005" EstimateIO="0.09"
                     EstimatedTotalSubtreeCost="0.095" AvgRowSize="50" Parallel="false">
                <OutputList></OutputList>
              </RelOp>
            </RelOp>
          </QueryPlan>
        </StmtSimple>
      </Statements>
    </Batch>
  </BatchSequence>
</ShowPlanXML>`;

describe('NodeDetails', () => {
  let planState: ReturnType<typeof usePlanState>;

  beforeEach(() => {
    planState = usePlanState();
    planState.clearPlan();
  });

  it('shows empty state when nothing is selected', () => {
    const wrapper = mount(NodeDetails);

    expect(wrapper.text()).toContain('Click a node or edge to view details');
    expect(wrapper.find('[v-else-if]')).toBeDefined();
  });

  it('shows node details when a node is selected', () => {
    planState.loadPlan(simpleXml);
    const relOp = planState.state.selectedStatement!.queryPlan.relOp;
    planState.selectNode(relOp);

    const wrapper = mount(NodeDetails);

    expect(wrapper.text()).toContain('Table Scan');
    expect(wrapper.text()).toContain('Node ID: 0');
  });

  it('shows cost information for selected node', () => {
    planState.loadPlan(simpleXml);
    const relOp = planState.state.selectedStatement!.queryPlan.relOp;
    planState.selectNode(relOp);

    const wrapper = mount(NodeDetails);

    expect(wrapper.text()).toContain('Cost Percentage');
    expect(wrapper.text()).toContain('100.0%'); // 0.01/0.01 = 100%
  });

  it('shows physical and logical op in metrics', () => {
    planState.loadPlan(simpleXml);
    const relOp = planState.state.selectedStatement!.queryPlan.relOp;
    planState.selectNode(relOp);

    const wrapper = mount(NodeDetails);

    expect(wrapper.text()).toContain('Physical Op');
    expect(wrapper.text()).toContain('Logical Op');
  });

  it('shows edge details when an edge is selected', () => {
    planState.loadPlan(nestedXml);
    const root = planState.state.selectedStatement!.queryPlan.relOp;
    const child = root.children[0];

    planState.selectEdge({ source: root, target: child });

    const wrapper = mount(NodeDetails);

    expect(wrapper.text()).toContain('Data Flow');
    expect(wrapper.text()).toContain('Index Seek');
    expect(wrapper.text()).toContain('Nested Loops');
  });

  it('shows output columns when available', () => {
    planState.loadPlan(simpleXml);
    const relOp = planState.state.selectedStatement!.queryPlan.relOp;
    planState.selectNode(relOp);

    const wrapper = mount(NodeDetails);

    expect(wrapper.text()).toContain('Output Columns');
    expect(wrapper.text()).toContain('Users.UserId');
  });

  it('does not show parallel execution notice for non-parallel nodes', () => {
    planState.loadPlan(simpleXml);
    const relOp = planState.state.selectedStatement!.queryPlan.relOp;
    planState.selectNode(relOp);

    const wrapper = mount(NodeDetails);

    expect(wrapper.text()).not.toContain('Parallel Execution');
  });

  it('renders without error', () => {
    expect(() => mount(NodeDetails)).not.toThrow();
  });
});
