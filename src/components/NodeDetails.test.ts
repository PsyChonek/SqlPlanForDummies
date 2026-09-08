import { mount } from '@vue/test-utils';
import NodeDetails from './NodeDetails.vue';
import { usePlanState } from '../composables/planState';
const { readFileSync } = await vi.importActual<{
  readFileSync: (path: string, encoding: string) => string;
}>('node:fs');

const companyXml = readFileSync('examples/company.sqlplan', 'utf16le');

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

const partitionedXml = `<?xml version="1.0" encoding="utf-16"?>
<ShowPlanXML xmlns="http://schemas.microsoft.com/sqlserver/2004/07/showplan" Version="1.5" Build="17.0">
  <BatchSequence>
    <Batch>
      <Statements>
        <StmtSimple StatementId="1" StatementText="SELECT * FROM IA_Data WHERE PartitionNumber = 1" StatementType="SELECT"
                    StatementSubTreeCost="0.56" StatementEstRows="29207">
          <QueryPlan DegreeOfParallelism="1">
            <RelOp NodeId="0" PhysicalOp="Clustered Index Scan" LogicalOp="Clustered Index Scan"
                   EstimateRows="29207" EstimateCPU="0.05" EstimateIO="0.48"
                   EstimatedTotalSubtreeCost="0.56" AvgRowSize="323" Parallel="0" Partitioned="1">
              <OutputList></OutputList>
              <IndexScan Ordered="1" ScanDirection="FORWARD" Storage="RowStore">
                <Object Database="[JobkaProduction]" Schema="[dbo]" Table="[IA_Data]" Index="[IX_IA_Data_ClusterID]" IndexKind="Clustered" />
                <SeekPredicates>
                  <SeekPredicateNew>
                    <SeekKeys>
                      <Prefix ScanType="EQ">
                        <RangeColumns>
                          <ColumnReference Column="PtnId1000" />
                        </RangeColumns>
                        <RangeExpressions>
                          <ScalarOperator ScalarString="(2)" />
                        </RangeExpressions>
                      </Prefix>
                    </SeekKeys>
                  </SeekPredicateNew>
                </SeekPredicates>
              </IndexScan>
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

  it('shows partition panel with elimination info for partitioned nodes', () => {
    planState.loadPlan(partitionedXml);
    const relOp = planState.state.selectedStatement!.queryPlan.relOp;
    planState.selectNode(relOp);

    const wrapper = mount(NodeDetails);

    expect(wrapper.text()).toContain('Partitioned Table Access');
    expect(wrapper.text()).toContain('Partition ID = 2');
    // Seek on the internal PtnId column is labeled as partition elimination
    expect(wrapper.text()).toContain('Partition Elimination');
    expect(wrapper.text()).not.toContain('PtnId1000 =');
  });

  it('does not show partition panel for non-partitioned nodes', () => {
    planState.loadPlan(simpleXml);
    const relOp = planState.state.selectedStatement!.queryPlan.relOp;
    planState.selectNode(relOp);

    const wrapper = mount(NodeDetails);

    expect(wrapper.text()).not.toContain('Partitioned Table Access');
  });

  describe('copy for AI', () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.restoreAllMocks();
      vi.clearAllTimers();
      vi.useRealTimers();
    });

    it('copies every descendant in the company plan with one query and each node own properties', async () => {
      planState.loadPlan(companyXml);
      expect(planState.state.error).toBeNull();
      const statement = planState.statements.value.find(statement =>
        statement.queryPlan.relOp.children.some(child => child.children.length > 0)
      );
      expect(statement).toBeDefined();
      planState.selectStatement(statement!);
      const root = planState.state.selectedStatement!.queryPlan.relOp;
      planState.selectNode(root);
      const writeText = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
      const wrapper = mount(NodeDetails);

      await wrapper.get('button[title^="Copy the selected node"]').trigger('click');

      expect(writeText).toHaveBeenCalledTimes(1);
      const text = writeText.mock.calls[0][0];
      expect(text).toContain('Analyze this SQL Server execution plan subtree');
      expect(text.match(/^## Query$/gm)).toHaveLength(1);
      expect(text.match(/^### .* \(Node \d+\)$/gm)).toHaveLength(planState.allNodes.value.length - 1);
      expect(planState.allNodes.value.some(node => node.children.some(child => child.children.length > 0))).toBe(true);
      for (const parent of planState.allNodes.value) {
        for (const child of parent.children) {
          const section = text.split(`### ${child.physicalOp} (Node ${child.nodeId})\n`)[1].split('\n### ')[0];
          expect(section).toContain(`- **Parent Node:** ${parent.nodeId}`);
          expect(section).toContain(`- **estimateRows:** ${child.estimateRows}`);
          if (child.runtimeInfo) {
            expect(section).toContain(`- **runtimeInfo.actualRows:** ${child.runtimeInfo.actualRows}`);
          }
        }
      }
      expect(wrapper.get('button[title^="Copy the selected node"]').text()).toBe('Copied!');
      expect(planState.state.selectedNode?.nodeId).toBe(root.nodeId);
      wrapper.unmount();
    });

    it('copies only the selected branch and ignores the property search filter', async () => {
      planState.loadPlan(nestedXml);
      const root = planState.state.selectedStatement!.queryPlan.relOp;
      const branch = root.children[0];
      branch.children.push({ ...root.children[1], nodeId: 3, children: [], attributes: { TestProperty: 'grandchild details' } });
      planState.selectNode(branch);
      const writeText = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
      const wrapper = mount(NodeDetails);
      await wrapper.get('input').setValue('no matching properties');

      await wrapper.get('button[title^="Copy the selected node"]').trigger('click');

      const text = writeText.mock.calls[0][0];
      expect(text).toContain('# Index Seek (Node 1)');
      expect(text).toContain('Nested Loops (Node 0)');
      expect(text).toContain('### Index Scan (Node 3)');
      expect(text).toContain('- **Parent Node:** 1');
      expect(text).toContain('- **attributes.TestProperty:** grandchild details');
      expect(text).not.toContain('Index Scan (Node 2)');
      expect(text).not.toContain('### Nested Loops (Node 0)');
      wrapper.unmount();
    });

    it('supports leaf nodes and keeps the original single-node copy available', async () => {
      planState.loadPlan(simpleXml);
      planState.selectNode(planState.state.selectedStatement!.queryPlan.relOp);
      const writeText = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
      const wrapper = mount(NodeDetails);

      await wrapper.get('button[title^="Copy the selected node"]').trigger('click');
      expect(writeText.mock.calls[0][0]).toContain('The selected node has no child nodes.');
      await wrapper.get('button[title^="Copy node details"]').trigger('click');
      expect(writeText.mock.calls[1][0]).toContain('# Table Scan (Node 0)');
      expect(writeText.mock.calls[1][0]).not.toContain('## Descendant Nodes');
      wrapper.unmount();
    });
  });
});
