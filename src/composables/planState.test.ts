import { usePlanState } from './planState';

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
              <OutputList></OutputList>
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
        <StmtSimple StatementId="1" StatementText="SELECT u.Id, o.Total FROM Users u JOIN Orders o ON u.Id = o.UserId" StatementType="SELECT"
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

describe('usePlanState', () => {
  let planState: ReturnType<typeof usePlanState>;

  beforeEach(() => {
    planState = usePlanState();
    planState.clearPlan();
  });

  describe('loadPlan', () => {
    it('loads a valid plan', () => {
      planState.loadPlan(simpleXml);

      expect(planState.state.plan).not.toBeNull();
      expect(planState.state.error).toBeNull();
      expect(planState.state.loading).toBe(false);
    });

    it('auto-selects the first statement', () => {
      planState.loadPlan(simpleXml);

      expect(planState.state.selectedStatement).not.toBeNull();
      expect(planState.state.selectedStatement?.statementText).toBe('SELECT * FROM Users');
    });

    it('increments revision counter on successful load', () => {
      const initialRevision = planState.state.revision;
      planState.loadPlan(simpleXml);

      expect(planState.state.revision).toBeGreaterThan(initialRevision);
    });

    it('clears selected node when loading a new plan', () => {
      planState.loadPlan(nestedXml);
      planState.selectNode(planState.state.selectedStatement!.queryPlan.relOp);

      expect(planState.state.selectedNode).not.toBeNull();

      planState.loadPlan(simpleXml);

      expect(planState.state.selectedNode).toBeNull();
    });

    it('sets error on invalid XML', () => {
      planState.loadPlan('<invalid>xml</bad>');

      expect(planState.state.error).not.toBeNull();
      expect(planState.state.plan).toBeNull();
      expect(planState.state.loading).toBe(false);
    });

    it('does not set selectedStatement on error', () => {
      planState.loadPlan('<invalid>xml</bad>');

      expect(planState.state.selectedStatement).toBeNull();
    });
  });

  describe('selectNode', () => {
    it('selects a node', () => {
      planState.loadPlan(simpleXml);
      const relOp = planState.state.selectedStatement!.queryPlan.relOp;

      planState.selectNode(relOp);

      expect(planState.state.selectedNode).toBe(relOp);
    });

    it('clears selected edge when selecting a node', () => {
      planState.loadPlan(nestedXml);
      const root = planState.state.selectedStatement!.queryPlan.relOp;
      const child = root.children[0];

      planState.selectEdge({ source: root, target: child });
      expect(planState.state.selectedEdge).not.toBeNull();

      planState.selectNode(root);

      expect(planState.state.selectedEdge).toBeNull();
    });

    it('can select null to deselect', () => {
      planState.loadPlan(simpleXml);
      planState.selectNode(planState.state.selectedStatement!.queryPlan.relOp);
      planState.selectNode(null);

      expect(planState.state.selectedNode).toBeNull();
    });
  });

  describe('selectEdge', () => {
    it('selects an edge', () => {
      planState.loadPlan(nestedXml);
      const root = planState.state.selectedStatement!.queryPlan.relOp;
      const child = root.children[0];

      planState.selectEdge({ source: root, target: child });

      expect(planState.state.selectedEdge).toEqual({ source: root, target: child });
    });

    it('clears selected node when selecting an edge', () => {
      planState.loadPlan(nestedXml);
      const root = planState.state.selectedStatement!.queryPlan.relOp;
      const child = root.children[0];

      planState.selectNode(root);
      expect(planState.state.selectedNode).not.toBeNull();

      planState.selectEdge({ source: root, target: child });

      expect(planState.state.selectedNode).toBeNull();
    });
  });

  describe('clearPlan', () => {
    it('clears all plan state', () => {
      planState.loadPlan(simpleXml);
      planState.clearPlan();

      expect(planState.state.plan).toBeNull();
      expect(planState.state.selectedStatement).toBeNull();
      expect(planState.state.selectedNode).toBeNull();
      expect(planState.state.error).toBeNull();
    });

    it('disables comparison mode when clearing', () => {
      planState.loadPlan(simpleXml);
      planState.loadComparisonPlan(simpleXml);
      expect(planState.state.comparisonMode).toBe(true);

      planState.clearPlan();

      expect(planState.state.comparisonMode).toBe(false);
      expect(planState.state.comparisonPlan).toBeNull();
      expect(planState.state.comparisonStatement).toBeNull();
    });

    it('increments revision counter', () => {
      planState.loadPlan(simpleXml);
      const revisionAfterLoad = planState.state.revision;

      planState.clearPlan();

      expect(planState.state.revision).toBeGreaterThan(revisionAfterLoad);
    });
  });

  describe('getNodeCostPercentage', () => {
    it('calculates cost percentage correctly', () => {
      planState.loadPlan(simpleXml);
      const relOp = planState.state.selectedStatement!.queryPlan.relOp;

      // totalCost is 0.01 (from StatementSubTreeCost)
      // relOp cost is 0.01
      const percentage = planState.getNodeCostPercentage(relOp);

      expect(percentage).toBe(100);
    });

    it('returns 0 when total cost is 0', () => {
      planState.loadPlan(simpleXml);
      const relOp = planState.state.selectedStatement!.queryPlan.relOp;

      // Clear plan so totalCost.value becomes 0
      planState.clearPlan();

      // Create a mock relOp with some cost
      const mockRelOp = { ...relOp, estimatedTotalSubtreeCost: 0.5 };

      // After clearing, we can't easily test this through the state...
      // Test directly when plan has 0-cost statement:
      const zeroCostXml = `<?xml version="1.0"?>
        <ShowPlanXML xmlns="http://schemas.microsoft.com/sqlserver/2004/07/showplan" Version="1.5" Build="17.0">
          <BatchSequence>
            <Batch>
              <Statements>
                <StmtSimple StatementId="1" StatementText="SELECT 1" StatementType="SELECT"
                            StatementSubTreeCost="0" StatementEstRows="1">
                  <QueryPlan DegreeOfParallelism="1">
                    <RelOp NodeId="0" PhysicalOp="Constant Scan" LogicalOp="Constant Scan"
                           EstimateRows="1" EstimateCPU="0" EstimateIO="0"
                           EstimatedTotalSubtreeCost="0.5" AvgRowSize="0" Parallel="false">
                      <OutputList></OutputList>
                    </RelOp>
                  </QueryPlan>
                </StmtSimple>
              </Statements>
            </Batch>
          </BatchSequence>
        </ShowPlanXML>`;

      planState.loadPlan(zeroCostXml);
      const zeroRelOp = planState.state.selectedStatement!.queryPlan.relOp;
      const percentage = planState.getNodeCostPercentage(zeroRelOp);

      // totalCost is 0 (from StatementSubTreeCost=0), so returns 0
      expect(percentage).toBe(0);

      // Suppress unused variable warning
      void mockRelOp;
    });

    it('calculates correct percentage for partial cost', () => {
      planState.loadPlan(nestedXml);
      const root = planState.state.selectedStatement!.queryPlan.relOp;
      const child1 = root.children[0]; // nodeId=1, cost=0.004

      // totalCost is 0.1 (from StatementSubTreeCost)
      // child1 cost is 0.004
      const percentage = planState.getNodeCostPercentage(child1);

      expect(percentage).toBeCloseTo(4, 1); // 0.004 / 0.1 * 100 = 4%
    });
  });

  describe('computed: totalCost', () => {
    it('returns 0 when no plan loaded', () => {
      expect(planState.totalCost.value).toBe(0);
    });

    it('returns total cost when plan loaded', () => {
      planState.loadPlan(simpleXml);
      expect(planState.totalCost.value).toBe(0.01);
    });
  });

  describe('computed: allNodes', () => {
    it('returns empty array when no statement selected', () => {
      expect(planState.allNodes.value).toHaveLength(0);
    });

    it('returns all nodes when statement selected', () => {
      planState.loadPlan(nestedXml);
      // 3 nodes: root + 2 children
      expect(planState.allNodes.value).toHaveLength(3);
    });

    it('returns single node for simple plan', () => {
      planState.loadPlan(simpleXml);
      expect(planState.allNodes.value).toHaveLength(1);
    });
  });

  describe('computed: statements', () => {
    it('returns empty array when no plan loaded', () => {
      expect(planState.statements.value).toHaveLength(0);
    });

    it('returns statements when plan loaded', () => {
      planState.loadPlan(simpleXml);
      expect(planState.statements.value).toHaveLength(1);
      expect(planState.statements.value[0].statementText).toBe('SELECT * FROM Users');
    });
  });

  describe('navigation', () => {
    beforeEach(() => {
      planState.loadPlan(nestedXml);
    });

    it('navigateToFirstChild selects first child', () => {
      const root = planState.state.selectedStatement!.queryPlan.relOp;
      planState.selectNode(root);

      planState.navigateToFirstChild();

      expect(planState.state.selectedNode?.nodeId).toBe(1); // Index Seek
    });

    it('navigateToFirstChild does nothing when node has no children', () => {
      const root = planState.state.selectedStatement!.queryPlan.relOp;
      const child1 = root.children[0]; // leaf node
      planState.selectNode(child1);

      planState.navigateToFirstChild();

      expect(planState.state.selectedNode).toBe(child1); // unchanged
    });

    it('navigateToFirstChild does nothing when no node selected', () => {
      planState.selectNode(null);

      planState.navigateToFirstChild(); // should not throw

      expect(planState.state.selectedNode).toBeNull();
    });

    it('navigateToParent selects parent node', () => {
      const root = planState.state.selectedStatement!.queryPlan.relOp;
      const child1 = root.children[0];
      planState.selectNode(child1);

      planState.navigateToParent();

      expect(planState.state.selectedNode?.nodeId).toBe(0); // Nested Loops
    });

    it('navigateToParent does nothing at root', () => {
      const root = planState.state.selectedStatement!.queryPlan.relOp;
      planState.selectNode(root);

      planState.navigateToParent();

      expect(planState.state.selectedNode?.nodeId).toBe(0); // unchanged
    });

    it('navigateToSibling moves to next sibling', () => {
      const root = planState.state.selectedStatement!.queryPlan.relOp;
      planState.selectNode(root.children[0]); // Index Seek (nodeId=1)

      planState.navigateToSibling('next');

      expect(planState.state.selectedNode?.nodeId).toBe(2); // Index Scan
    });

    it('navigateToSibling moves to previous sibling', () => {
      const root = planState.state.selectedStatement!.queryPlan.relOp;
      planState.selectNode(root.children[1]); // Index Scan (nodeId=2)

      planState.navigateToSibling('prev');

      expect(planState.state.selectedNode?.nodeId).toBe(1); // Index Seek
    });

    it('navigateToSibling does not navigate past last sibling', () => {
      const root = planState.state.selectedStatement!.queryPlan.relOp;
      planState.selectNode(root.children[1]); // last sibling

      planState.navigateToSibling('next');

      expect(planState.state.selectedNode?.nodeId).toBe(2); // unchanged
    });

    it('navigateToSibling does not navigate before first sibling', () => {
      const root = planState.state.selectedStatement!.queryPlan.relOp;
      planState.selectNode(root.children[0]); // first sibling

      planState.navigateToSibling('prev');

      expect(planState.state.selectedNode?.nodeId).toBe(1); // unchanged
    });

    it('selectFirstNode selects root node', () => {
      planState.selectFirstNode();

      expect(planState.state.selectedNode?.nodeId).toBe(0);
      expect(planState.state.selectedNode?.physicalOp).toBe('Nested Loops');
    });
  });

  describe('comparison mode', () => {
    it('loads comparison plan', () => {
      planState.loadPlan(simpleXml);
      planState.loadComparisonPlan(nestedXml);

      expect(planState.state.comparisonPlan).not.toBeNull();
      expect(planState.state.comparisonMode).toBe(true);
    });

    it('auto-selects first comparison statement', () => {
      planState.loadPlan(simpleXml);
      planState.loadComparisonPlan(nestedXml);

      expect(planState.state.comparisonStatement).not.toBeNull();
      expect(planState.state.comparisonStatement?.statementType).toBe('SELECT');
    });

    it('toggleComparisonMode disables comparison when active', () => {
      planState.loadPlan(simpleXml);
      planState.loadComparisonPlan(simpleXml);
      expect(planState.state.comparisonMode).toBe(true);

      planState.toggleComparisonMode();

      expect(planState.state.comparisonMode).toBe(false);
      expect(planState.state.comparisonPlan).toBeNull();
      expect(planState.state.comparisonStatement).toBeNull();
    });

    it('toggleComparisonMode enables comparison when inactive', () => {
      planState.loadPlan(simpleXml);
      expect(planState.state.comparisonMode).toBe(false);

      planState.toggleComparisonMode();

      expect(planState.state.comparisonMode).toBe(true);
    });

    it('clearComparisonPlan removes comparison', () => {
      planState.loadPlan(simpleXml);
      planState.loadComparisonPlan(nestedXml);

      planState.clearComparisonPlan();

      expect(planState.state.comparisonPlan).toBeNull();
      expect(planState.state.comparisonStatement).toBeNull();
      expect(planState.state.comparisonMode).toBe(false);
    });

    it('comparisonStatements returns statements from comparison plan', () => {
      planState.loadPlan(simpleXml);
      planState.loadComparisonPlan(nestedXml);

      expect(planState.comparisonStatements.value).toHaveLength(1);
    });

    it('comparisonStatements returns empty when no comparison plan', () => {
      planState.loadPlan(simpleXml);

      expect(planState.comparisonStatements.value).toHaveLength(0);
    });

    it('sets error when comparison plan is invalid XML', () => {
      planState.loadPlan(simpleXml);
      planState.loadComparisonPlan('<invalid>xml</bad>');

      expect(planState.state.error).not.toBeNull();
      expect(planState.state.comparisonMode).toBe(false);
    });
  });
});
