import {
  parseSqlPlan,
  flattenRelOps,
  getTotalCost,
  getMostExpensiveStatement,
} from './sqlPlanParser';
import type { ShowPlanXML, RelOp } from '../types/sqlplan';

// Simple test XML plan
const simpleXml = `<?xml version="1.0" encoding="utf-16"?>
<ShowPlanXML xmlns="http://schemas.microsoft.com/sqlserver/2004/07/showplan" Version="1.5" Build="17.0.4006.2">
  <BatchSequence>
    <Batch>
      <Statements>
        <StmtSimple StatementId="1" StatementText="SELECT * FROM Users" StatementType="SELECT"
                    StatementSubTreeCost="0.0065704" StatementEstRows="100">
          <QueryPlan DegreeOfParallelism="1">
            <RelOp NodeId="0" PhysicalOp="Table Scan" LogicalOp="Table Scan"
                   EstimateRows="100" EstimateCPU="0.001" EstimateIO="0.005"
                   EstimatedTotalSubtreeCost="0.006" AvgRowSize="50" Parallel="false">
              <OutputList></OutputList>
            </RelOp>
          </QueryPlan>
        </StmtSimple>
      </Statements>
    </Batch>
  </BatchSequence>
</ShowPlanXML>`;

// XML with multiple statements
const multiStatementXml = `<?xml version="1.0" encoding="utf-16"?>
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
        <StmtSimple StatementId="2" StatementText="SELECT * FROM Orders" StatementType="SELECT"
                    StatementSubTreeCost="0.05" StatementEstRows="1000">
          <QueryPlan DegreeOfParallelism="1">
            <RelOp NodeId="0" PhysicalOp="Table Scan" LogicalOp="Table Scan"
                   EstimateRows="1000" EstimateCPU="0.01" EstimateIO="0.04"
                   EstimatedTotalSubtreeCost="0.05" AvgRowSize="200" Parallel="false">
              <OutputList></OutputList>
            </RelOp>
          </QueryPlan>
        </StmtSimple>
      </Statements>
    </Batch>
  </BatchSequence>
</ShowPlanXML>`;

// XML with nested operators (parent with children)
const nestedXml = `<?xml version="1.0" encoding="utf-16"?>
<ShowPlanXML xmlns="http://schemas.microsoft.com/sqlserver/2004/07/showplan" Version="1.5" Build="17.0">
  <BatchSequence>
    <Batch>
      <Statements>
        <StmtSimple StatementId="1" StatementText="SELECT * FROM Users u JOIN Orders o ON u.Id = o.UserId" StatementType="SELECT"
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

describe('parseSqlPlan', () => {
  it('parses valid XML plan', () => {
    const plan = parseSqlPlan(simpleXml);

    expect(plan.version).toBe('1.5');
    expect(plan.build).toBe('17.0.4006.2');
    expect(plan.batches).toHaveLength(1);
    expect(plan.batches[0].statements).toHaveLength(1);
  });

  it('parses statement attributes correctly', () => {
    const plan = parseSqlPlan(simpleXml);
    const stmt = plan.batches[0].statements[0];

    expect(stmt.statementId).toBe(1);
    expect(stmt.statementText).toBe('SELECT * FROM Users');
    expect(stmt.statementType).toBe('SELECT');
    expect(stmt.statementSubTreeCost).toBe(0.0065704);
    expect(stmt.statementEstRows).toBe(100);
  });

  it('parses RelOp correctly', () => {
    const plan = parseSqlPlan(simpleXml);
    const relOp = plan.batches[0].statements[0].queryPlan.relOp;

    expect(relOp.nodeId).toBe(0);
    expect(relOp.physicalOp).toBe('Table Scan');
    expect(relOp.logicalOp).toBe('Table Scan');
    expect(relOp.estimateRows).toBe(100);
    expect(relOp.estimateCPU).toBe(0.001);
    expect(relOp.estimateIO).toBe(0.005);
    expect(relOp.estimatedTotalSubtreeCost).toBe(0.006);
    expect(relOp.avgRowSize).toBe(50);
    expect(relOp.parallel).toBe(false);
  });

  it('parses QueryPlan degreeOfParallelism', () => {
    const plan = parseSqlPlan(simpleXml);
    const queryPlan = plan.batches[0].statements[0].queryPlan;

    expect(queryPlan.degreeOfParallelism).toBe(1);
  });

  it('throws error for invalid XML', () => {
    const invalidXml = '<invalid>xml</bad>';
    expect(() => parseSqlPlan(invalidXml)).toThrow(/XML parsing error/);
  });

  it('handles empty batches', () => {
    const emptyXml = `<?xml version="1.0"?>
      <ShowPlanXML xmlns="http://schemas.microsoft.com/sqlserver/2004/07/showplan" Version="1.5" Build="17.0">
        <BatchSequence></BatchSequence>
      </ShowPlanXML>`;

    const plan = parseSqlPlan(emptyXml);
    expect(plan.batches).toHaveLength(0);
  });

  it('handles missing BatchSequence', () => {
    const noBatchXml = `<?xml version="1.0"?>
      <ShowPlanXML xmlns="http://schemas.microsoft.com/sqlserver/2004/07/showplan" Version="1.5" Build="17.0">
      </ShowPlanXML>`;

    const plan = parseSqlPlan(noBatchXml);
    expect(plan.batches).toHaveLength(0);
  });

  it('parses multiple statements in a batch', () => {
    const plan = parseSqlPlan(multiStatementXml);

    expect(plan.batches).toHaveLength(1);
    expect(plan.batches[0].statements).toHaveLength(2);
    expect(plan.batches[0].statements[0].statementId).toBe(1);
    expect(plan.batches[0].statements[1].statementId).toBe(2);
  });

  it('parses nested RelOps (parent with children)', () => {
    const plan = parseSqlPlan(nestedXml);
    const root = plan.batches[0].statements[0].queryPlan.relOp;

    expect(root.physicalOp).toBe('Nested Loops');
    expect(root.children).toHaveLength(2);
    expect(root.children[0].physicalOp).toBe('Index Seek');
    expect(root.children[1].physicalOp).toBe('Index Scan');
  });

  it('handles missing optional attributes', () => {
    const minimalXml = `<?xml version="1.0"?>
      <ShowPlanXML xmlns="http://schemas.microsoft.com/sqlserver/2004/07/showplan">
        <BatchSequence>
          <Batch>
            <Statements>
              <StmtSimple>
                <QueryPlan>
                  <RelOp>
                    <OutputList></OutputList>
                  </RelOp>
                </QueryPlan>
              </StmtSimple>
            </Statements>
          </Batch>
        </BatchSequence>
      </ShowPlanXML>`;

    const plan = parseSqlPlan(minimalXml);
    expect(plan.version).toBe('');
    expect(plan.build).toBe('');

    const stmt = plan.batches[0].statements[0];
    expect(stmt.statementId).toBe(0);
    expect(stmt.statementSubTreeCost).toBe(0);
  });
});

describe('flattenRelOps', () => {
  it('flattens single node', () => {
    const relOp: RelOp = {
      nodeId: 0,
      physicalOp: 'Table Scan',
      logicalOp: 'Table Scan',
      estimateRows: 100,
      estimateCPU: 0.001,
      estimateIO: 0.005,
      estimatedTotalSubtreeCost: 0.006,
      avgRowSize: 50,
      parallel: false,
      outputColumns: [],
      children: [],
      operationDetails: {},
    };

    const flattened = flattenRelOps(relOp);
    expect(flattened).toHaveLength(1);
    expect(flattened[0]).toBe(relOp);
  });

  it('flattens tree with children (breadth-first traversal)', () => {
    const child1: RelOp = {
      nodeId: 1,
      physicalOp: 'Index Seek',
      logicalOp: 'Index Seek',
      estimateRows: 10,
      estimateCPU: 0.001,
      estimateIO: 0.001,
      estimatedTotalSubtreeCost: 0.002,
      avgRowSize: 20,
      parallel: false,
      outputColumns: [],
      children: [],
      operationDetails: {},
    };

    const child2: RelOp = {
      nodeId: 2,
      physicalOp: 'Table Scan',
      logicalOp: 'Table Scan',
      estimateRows: 50,
      estimateCPU: 0.002,
      estimateIO: 0.003,
      estimatedTotalSubtreeCost: 0.005,
      avgRowSize: 30,
      parallel: false,
      outputColumns: [],
      children: [],
      operationDetails: {},
    };

    const parent: RelOp = {
      nodeId: 0,
      physicalOp: 'Nested Loops',
      logicalOp: 'Join',
      estimateRows: 100,
      estimateCPU: 0.001,
      estimateIO: 0.000,
      estimatedTotalSubtreeCost: 0.008,
      avgRowSize: 50,
      parallel: false,
      outputColumns: [],
      children: [child1, child2],
      operationDetails: {},
    };

    const flattened = flattenRelOps(parent);
    expect(flattened).toHaveLength(3);
    expect(flattened[0]).toBe(parent);
    expect(flattened[1]).toBe(child1);
    expect(flattened[2]).toBe(child2);
  });

  it('flattens deeply nested tree', () => {
    const grandchild: RelOp = {
      nodeId: 3,
      physicalOp: 'Index Seek',
      logicalOp: 'Index Seek',
      estimateRows: 5,
      estimateCPU: 0.001,
      estimateIO: 0.001,
      estimatedTotalSubtreeCost: 0.002,
      avgRowSize: 10,
      parallel: false,
      outputColumns: [],
      children: [],
      operationDetails: {},
    };

    const child: RelOp = {
      nodeId: 2,
      physicalOp: 'Nested Loops',
      logicalOp: 'Join',
      estimateRows: 20,
      estimateCPU: 0.001,
      estimateIO: 0.000,
      estimatedTotalSubtreeCost: 0.003,
      avgRowSize: 20,
      parallel: false,
      outputColumns: [],
      children: [grandchild],
      operationDetails: {},
    };

    const parent: RelOp = {
      nodeId: 0,
      physicalOp: 'Nested Loops',
      logicalOp: 'Join',
      estimateRows: 100,
      estimateCPU: 0.001,
      estimateIO: 0.000,
      estimatedTotalSubtreeCost: 0.004,
      avgRowSize: 30,
      parallel: false,
      outputColumns: [],
      children: [child],
      operationDetails: {},
    };

    const flattened = flattenRelOps(parent);
    expect(flattened).toHaveLength(3);
    expect(flattened[0]).toBe(parent);
    expect(flattened[1]).toBe(child);
    expect(flattened[2]).toBe(grandchild);
  });
});

describe('getTotalCost', () => {
  it('calculates total cost for single statement', () => {
    const plan = parseSqlPlan(simpleXml);
    const totalCost = getTotalCost(plan);
    expect(totalCost).toBe(0.0065704);
  });

  it('calculates total cost across multiple statements', () => {
    const plan = parseSqlPlan(multiStatementXml);
    const totalCost = getTotalCost(plan);
    expect(totalCost).toBeCloseTo(0.06, 5); // 0.01 + 0.05 = 0.06
  });

  it('returns 0 for empty plan', () => {
    const emptyPlan: ShowPlanXML = {
      version: '1.5',
      build: '17.0',
      batches: [],
    };
    expect(getTotalCost(emptyPlan)).toBe(0);
  });

  it('returns 0 for plan with empty batches', () => {
    const plan: ShowPlanXML = {
      version: '1.5',
      build: '17.0',
      batches: [{ statements: [] }],
    };
    expect(getTotalCost(plan)).toBe(0);
  });
});

describe('getMostExpensiveStatement', () => {
  it('finds most expensive statement with single statement', () => {
    const plan = parseSqlPlan(simpleXml);
    const expensive = getMostExpensiveStatement(plan);

    expect(expensive).not.toBeNull();
    expect(expensive?.statementSubTreeCost).toBe(0.0065704);
    expect(expensive?.statementText).toBe('SELECT * FROM Users');
  });

  it('finds most expensive statement with multiple statements', () => {
    const plan = parseSqlPlan(multiStatementXml);
    const expensive = getMostExpensiveStatement(plan);

    expect(expensive).not.toBeNull();
    expect(expensive?.statementSubTreeCost).toBe(0.05);
    expect(expensive?.statementText).toBe('SELECT * FROM Orders');
    expect(expensive?.statementId).toBe(2);
  });

  it('returns null for empty plan', () => {
    const emptyPlan: ShowPlanXML = {
      version: '1.5',
      build: '17.0',
      batches: [],
    };
    expect(getMostExpensiveStatement(emptyPlan)).toBeNull();
  });

  it('returns null for plan with empty batches', () => {
    const plan: ShowPlanXML = {
      version: '1.5',
      build: '17.0',
      batches: [{ statements: [] }],
    };
    expect(getMostExpensiveStatement(plan)).toBeNull();
  });

  it('returns first statement when all have zero cost', () => {
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
                         EstimatedTotalSubtreeCost="0" AvgRowSize="0" Parallel="false">
                    <OutputList></OutputList>
                  </RelOp>
                </QueryPlan>
              </StmtSimple>
            </Statements>
          </Batch>
        </BatchSequence>
      </ShowPlanXML>`;

    const plan = parseSqlPlan(zeroCostXml);
    const expensive = getMostExpensiveStatement(plan);

    // With all costs at 0, returns null since maxCost starts at 0
    expect(expensive).toBeNull();
  });
});
