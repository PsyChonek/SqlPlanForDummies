/**
 * XML Parser for SQL Server Execution Plan (.sqlplan) files
 * Parses ShowPlan XML format into TypeScript structures
 */

import type {
  ShowPlanXML,
  Batch,
  Statement,
  QueryPlan,
  RelOp,
  RuntimeInfo,
  WaitStat,
  ColumnReference,
  OperationDetails,
  IndexScanDetails,
  ObjectReference,
  NestedLoopsDetails,
  MemoryGrantInfo,
  Parameter,
  SeekPredicate,
  DefinedValue,
} from '../types/sqlplan';

/**
 * Parse a .sqlplan XML string into a ShowPlanXML object
 */
export function parseSqlPlan(xmlString: string): ShowPlanXML {
  const parser = new DOMParser();
  const doc = parser.parseFromString(xmlString, 'text/xml');
  
  // Check for parsing errors
  const parseError = doc.querySelector('parsererror');
  if (parseError) {
    throw new Error(`XML parsing error: ${parseError.textContent}`);
  }
  
  const root = doc.documentElement;
  
  return {
    version: root.getAttribute('Version') || '',
    build: root.getAttribute('Build') || '',
    batches: parseBatches(root),
  };
}

/**
 * Parse all batches from the ShowPlanXML root
 */
function parseBatches(root: Element): Batch[] {
  const batches: Batch[] = [];
  const batchSequence = getChildElement(root, 'BatchSequence');
  
  if (!batchSequence) return batches;
  
  const batchElements = getChildElements(batchSequence, 'Batch');
  for (const batchEl of batchElements) {
    batches.push(parseBatch(batchEl));
  }
  
  return batches;
}

/**
 * Parse a single Batch element
 */
function parseBatch(batchEl: Element): Batch {
  const statements: Statement[] = [];
  const statementsEl = getChildElement(batchEl, 'Statements');
  
  if (statementsEl) {
    const stmtElements = getChildElements(statementsEl, 'StmtSimple');
    for (const stmtEl of stmtElements) {
      statements.push(parseStatement(stmtEl));
    }
  }
  
  return { statements };
}

/**
 * Parse a StmtSimple element
 */
function parseStatement(stmtEl: Element): Statement {
  const queryPlanEl = getChildElement(stmtEl, 'QueryPlan');
  
  return {
    statementId: parseInt(stmtEl.getAttribute('StatementId') || '0', 10),
    statementText: stmtEl.getAttribute('StatementText') || '',
    statementType: stmtEl.getAttribute('StatementType') || '',
    statementSubTreeCost: parseFloat(stmtEl.getAttribute('StatementSubTreeCost') || '0'),
    statementEstRows: parseFloat(stmtEl.getAttribute('StatementEstRows') || '0'),
    statementOptmLevel: stmtEl.getAttribute('StatementOptmLevel') || undefined,
    queryHash: stmtEl.getAttribute('QueryHash') || undefined,
    queryPlanHash: stmtEl.getAttribute('QueryPlanHash') || undefined,
    queryPlan: queryPlanEl ? parseQueryPlan(queryPlanEl) : createEmptyQueryPlan(),
  };
}

/**
 * Parse a QueryPlan element
 */
function parseQueryPlan(planEl: Element): QueryPlan {
  const relOpEl = getChildElement(planEl, 'RelOp');
  const memoryGrantEl = getChildElement(planEl, 'MemoryGrantInfo');
  const paramListEl = getChildElement(planEl, 'ParameterList');
  
  return {
    degreeOfParallelism: parseInt(planEl.getAttribute('DegreeOfParallelism') || '1', 10),
    cachedPlanSize: parseInt(planEl.getAttribute('CachedPlanSize') || '0', 10),
    compileTime: parseInt(planEl.getAttribute('CompileTime') || '0', 10),
    compileCPU: parseInt(planEl.getAttribute('CompileCPU') || '0', 10),
    compileMemory: parseInt(planEl.getAttribute('CompileMemory') || '0', 10),
    memoryGrant: memoryGrantEl ? parseMemoryGrant(memoryGrantEl) : undefined,
    relOp: relOpEl ? parseRelOp(relOpEl) : createEmptyRelOp(),
    parameters: paramListEl ? parseParameters(paramListEl) : undefined,
  };
}

/**
 * Parse MemoryGrantInfo element
 */
function parseMemoryGrant(el: Element): MemoryGrantInfo {
  return {
    serialRequiredMemory: parseInt(el.getAttribute('SerialRequiredMemory') || '0', 10),
    serialDesiredMemory: parseInt(el.getAttribute('SerialDesiredMemory') || '0', 10),
    grantedMemory: parseInt(el.getAttribute('GrantedMemory') || '0', 10),
    maxUsedMemory: parseInt(el.getAttribute('MaxUsedMemory') || '0', 10),
  };
}

/**
 * Parse ParameterList element
 */
function parseParameters(paramListEl: Element): Parameter[] {
  const params: Parameter[] = [];
  const colRefs = getChildElements(paramListEl, 'ColumnReference');
  
  for (const colRef of colRefs) {
    params.push({
      column: colRef.getAttribute('Column') || '',
      dataType: colRef.getAttribute('ParameterDataType') || '',
      compiledValue: colRef.getAttribute('ParameterCompiledValue') || undefined,
      runtimeValue: colRef.getAttribute('ParameterRuntimeValue') || undefined,
    });
  }
  
  return params;
}

/**
 * Parse a RelOp element (recursive)
 */
function parseRelOp(relOpEl: Element): RelOp {
  const physicalOp = relOpEl.getAttribute('PhysicalOp') || 'Unknown';
  const children = findChildRelOps(relOpEl);
  
  return {
    nodeId: parseInt(relOpEl.getAttribute('NodeId') || '0', 10),
    physicalOp: physicalOp,
    logicalOp: relOpEl.getAttribute('LogicalOp') || '',
    estimateRows: parseFloat(relOpEl.getAttribute('EstimateRows') || '0'),
    estimatedRowsRead: parseFloat(relOpEl.getAttribute('EstimatedRowsRead') || '0') || undefined,
    estimateCPU: parseFloat(relOpEl.getAttribute('EstimateCPU') || '0'),
    estimateIO: parseFloat(relOpEl.getAttribute('EstimateIO') || '0'),
    estimatedTotalSubtreeCost: parseFloat(relOpEl.getAttribute('EstimatedTotalSubtreeCost') || '0'),
    avgRowSize: parseInt(relOpEl.getAttribute('AvgRowSize') || '0', 10),
    parallel: relOpEl.getAttribute('Parallel') === 'true',
    estimatedExecutionMode: (relOpEl.getAttribute('EstimatedExecutionMode') as 'Row' | 'Batch') || undefined,
    estimateRebinds: parseFloat(relOpEl.getAttribute('EstimateRebinds') || '0') || undefined,
    estimateRewinds: parseFloat(relOpEl.getAttribute('EstimateRewinds') || '0') || undefined,
    tableCardinality: parseFloat(relOpEl.getAttribute('TableCardinality') || '0') || undefined,
    attributes: collectAttributes(relOpEl),
    outputColumns: parseOutputColumns(relOpEl),
    runtimeInfo: parseRuntimeInfo(relOpEl),
    children: children.map(parseRelOp),
    operationDetails: parseOperationDetails(relOpEl, physicalOp),
  };
}

/**
 * Find all direct child RelOp elements (they can be nested in operation-specific elements).
 * Recursively descends into non-RelOp wrapper elements (NestedLoops, Filter, Predicate,
 * Subquery, etc.) but stops at each RelOp boundary — so only the immediate child operators
 * are returned, not their descendants.  This handles arbitrary nesting depths such as
 * Filter > Predicate > ScalarOperator > Subquery > RelOp.
 */
function findChildRelOps(parentEl: Element): Element[] {
  const children: Element[] = [];

  function search(el: Element) {
    for (const child of el.children) {
      if (child.localName === 'RelOp') {
        children.push(child);
        // Stop here — this RelOp's own children belong to its subtree.
      } else {
        search(child);
      }
    }
  }

  search(parentEl);
  return children;
}

/**
 * Parse OutputList columns
 */
function parseOutputColumns(relOpEl: Element): ColumnReference[] {
  const columns: ColumnReference[] = [];
  const outputListEl = getChildElement(relOpEl, 'OutputList');
  
  if (outputListEl) {
    const colRefs = getChildElements(outputListEl, 'ColumnReference');
    for (const colRef of colRefs) {
      columns.push(parseColumnReference(colRef));
    }
  }
  
  return columns;
}

/**
 * Parse a ColumnReference element
 */
function parseColumnReference(colRef: Element): ColumnReference {
  return {
    database: colRef.getAttribute('Database')?.replace(/[\[\]]/g, '') || undefined,
    schema: colRef.getAttribute('Schema')?.replace(/[\[\]]/g, '') || undefined,
    table: colRef.getAttribute('Table')?.replace(/[\[\]]/g, '') || undefined,
    alias: colRef.getAttribute('Alias') || undefined,
    column: colRef.getAttribute('Column')?.replace(/[\[\]]/g, '') || '',
  };
}

/**
 * Parse RunTimeInformation
 */
function parseRuntimeInfo(relOpEl: Element): RuntimeInfo | undefined {
  const runtimeEl = getChildElement(relOpEl, 'RunTimeInformation');
  if (!runtimeEl) return undefined;

  const counterEl = getChildElement(runtimeEl, 'RunTimeCountersPerThread');
  if (!counterEl) return undefined;

  // WaitStats can appear as child of RunTimeCountersPerThread or RunTimeInformation
  const waitStatsEl = getChildElement(counterEl, 'WaitStats') ?? getChildElement(runtimeEl, 'WaitStats');

  return {
    threadId: parseInt(counterEl.getAttribute('Thread') || '0', 10),
    actualRows: parseInt(counterEl.getAttribute('ActualRows') || '0', 10),
    actualRowsRead: parseInt(counterEl.getAttribute('ActualRowsRead') || '0', 10) || undefined,
    actualExecutions: parseInt(counterEl.getAttribute('ActualExecutions') || '0', 10),

    attributes: collectAttributes(counterEl),

    actualEndOfScans: parseInt(counterEl.getAttribute('ActualEndOfScans') || '0', 10) || undefined,
    actualElapsedMs: parseFloat(counterEl.getAttribute('ActualElapsedms') || '0'),
    actualCPUMs: parseFloat(counterEl.getAttribute('ActualCPUms') || '0'),
    actualScans: parseInt(counterEl.getAttribute('ActualScans') || '0', 10) || undefined,
    actualLogicalReads: parseInt(counterEl.getAttribute('ActualLogicalReads') || '0', 10) || undefined,
    actualPhysicalReads: parseInt(counterEl.getAttribute('ActualPhysicalReads') || '0', 10) || undefined,
    actualReadAheads: parseInt(counterEl.getAttribute('ActualReadAheads') || '0', 10) || undefined,
    actualLobLogicalReads: parseInt(counterEl.getAttribute('ActualLobLogicalReads') || '0', 10) || undefined,
    actualLobPhysicalReads: parseInt(counterEl.getAttribute('ActualLobPhysicalReads') || '0', 10) || undefined,
    actualLobReadAheads: parseInt(counterEl.getAttribute('ActualLobReadAheads') || '0', 10) || undefined,
    batches: parseInt(counterEl.getAttribute('Batches') || '0', 10) || undefined,
    executionMode: (counterEl.getAttribute('ActualExecutionMode') as 'Row' | 'Batch') || undefined,
    waitStats: waitStatsEl ? parseWaitStats(waitStatsEl) : undefined,
  };
}

/**
 * Parse WaitStats element
 */
function parseWaitStats(waitStatsEl: Element): WaitStat[] {
  const waits: WaitStat[] = [];
  for (const waitEl of getChildElements(waitStatsEl, 'Wait')) {
    const waitTimeMs = parseFloat(waitEl.getAttribute('WaitTimeMs') || '0');
    if (waitTimeMs > 0) {
      waits.push({
        waitType: waitEl.getAttribute('WaitType') || '',
        waitTimeMs,
        waitCount: parseInt(waitEl.getAttribute('WaitCount') || '0', 10),
      });
    }
  }
  // Sort descending by wait time so the top wait is first
  return waits.sort((a, b) => b.waitTimeMs - a.waitTimeMs);
}

/**
 * Parse operation-specific details
 */
function parseOperationDetails(relOpEl: Element, _physicalOp: string): OperationDetails {
  const details: OperationDetails = { raw: {} };
  
  // Index Scan/Seek operations
  const indexScanEl = getChildElement(relOpEl, 'IndexScan');
  if (indexScanEl) {
    details.indexScan = parseIndexScan(indexScanEl);
  }
  
  // Nested Loops
  const nestedLoopsEl = getChildElement(relOpEl, 'NestedLoops');
  if (nestedLoopsEl) {
    details.nestedLoops = parseNestedLoops(nestedLoopsEl);
  }
  
  // Compute Scalar
  const computeScalarEl = getChildElement(relOpEl, 'ComputeScalar');
  if (computeScalarEl) {
    const definedValuesEl = getChildElement(computeScalarEl, 'DefinedValues');
    if (definedValuesEl) {
      details.computeScalar = {
        definedValues: parseDefinedValues(definedValuesEl),
      };
    }
  }
  
  // Stream Aggregate
  const streamAggEl = getChildElement(relOpEl, 'StreamAggregate');
  if (streamAggEl) {
    const definedValuesEl = getChildElement(streamAggEl, 'DefinedValues');
    if (definedValuesEl) {
      details.streamAggregate = {
        definedValues: parseDefinedValues(definedValuesEl),
      };
    }
  }
  
  // Filter
  const filterEl = getChildElement(relOpEl, 'Filter');
  if (filterEl) {
    const predicateEl = getChildElement(filterEl, 'Predicate');
    const scalarOpEl = predicateEl ? getChildElement(predicateEl, 'ScalarOperator') : null;
    details.filter = {
      predicate: scalarOpEl?.getAttribute('ScalarString') || '',
      startupExpression: filterEl.getAttribute('StartupExpression') === 'true',
    };
  }
  
  // Assert
  const assertEl = getChildElement(relOpEl, 'Assert');
  if (assertEl) {
    const predicateEl = getChildElement(assertEl, 'Predicate');
    const scalarOpEl = predicateEl ? getChildElement(predicateEl, 'ScalarOperator') : null;
    details.filter = {
      predicate: scalarOpEl?.getAttribute('ScalarString') || '',
      startupExpression: assertEl.getAttribute('StartupExpression') === 'true',
    };
  }
  
  return details;
}

/**
 * Parse IndexScan element
 */
function parseIndexScan(indexScanEl: Element): IndexScanDetails {
  const objectEl = getChildElement(indexScanEl, 'Object');
  const seekPredicatesEl = getChildElement(indexScanEl, 'SeekPredicates');
  const definedValuesEl = getChildElement(indexScanEl, 'DefinedValues');
  
  return {
    ordered: indexScanEl.getAttribute('Ordered') === 'true',
    scanDirection: (indexScanEl.getAttribute('ScanDirection') as 'FORWARD' | 'BACKWARD') || undefined,
    forcedIndex: indexScanEl.getAttribute('ForcedIndex') === 'true',
    forceSeek: indexScanEl.getAttribute('ForceSeek') === 'true',
    forceScan: indexScanEl.getAttribute('ForceScan') === 'true',
    noExpandHint: indexScanEl.getAttribute('NoExpandHint') === 'true',
    storage: (indexScanEl.getAttribute('Storage') as 'RowStore' | 'ColumnStore') || 'RowStore',
    object: objectEl ? parseObjectReference(objectEl) : { table: '' },
    seekPredicates: seekPredicatesEl ? parseSeekPredicates(seekPredicatesEl) : undefined,
    definedValues: definedValuesEl ? parseDefinedValues(definedValuesEl) : undefined,
  };
}

/**
 * Parse Object reference
 */
function parseObjectReference(objectEl: Element): ObjectReference {
  return {
    database: objectEl.getAttribute('Database')?.replace(/[\[\]]/g, '') || undefined,
    schema: objectEl.getAttribute('Schema')?.replace(/[\[\]]/g, '') || undefined,
    table: objectEl.getAttribute('Table')?.replace(/[\[\]]/g, '') || '',
    index: objectEl.getAttribute('Index')?.replace(/[\[\]]/g, '') || undefined,
    indexKind: objectEl.getAttribute('IndexKind') as ObjectReference['indexKind'] || undefined,
    storage: objectEl.getAttribute('Storage') || undefined,
    alias: objectEl.getAttribute('Alias') || undefined,
  };
}

/**
 * Parse SeekPredicates
 */
function parseSeekPredicates(seekPredicatesEl: Element): SeekPredicate[] {
  const predicates: SeekPredicate[] = [];
  const seekPredNewEls = getChildElements(seekPredicatesEl, 'SeekPredicateNew');
  
  for (const spEl of seekPredNewEls) {
    const seekKeysEl = getChildElement(spEl, 'SeekKeys');
    if (seekKeysEl) {
      const prefixEl = getChildElement(seekKeysEl, 'Prefix');
      if (prefixEl) {
        const rangeColumnsEl = getChildElement(prefixEl, 'RangeColumns');
        const rangeExpressionsEl = getChildElement(prefixEl, 'RangeExpressions');
        
        predicates.push({
          prefix: {
            scanType: prefixEl.getAttribute('ScanType') || '',
            rangeColumns: rangeColumnsEl ? 
              getChildElements(rangeColumnsEl, 'ColumnReference').map(parseColumnReference) : [],
            rangeExpressions: rangeExpressionsEl ?
              getChildElements(rangeExpressionsEl, 'ScalarOperator').map(
                el => el.getAttribute('ScalarString') || ''
              ) : [],
          },
        });
      }
    }
  }
  
  return predicates;
}

/**
 * Parse DefinedValues
 */
function parseDefinedValues(definedValuesEl: Element): DefinedValue[] {
  const values: DefinedValue[] = [];
  const defValueEls = getChildElements(definedValuesEl, 'DefinedValue');
  
  for (const dvEl of defValueEls) {
    const colRefEl = getChildElement(dvEl, 'ColumnReference');
    const scalarOpEl = getChildElement(dvEl, 'ScalarOperator');
    
    if (colRefEl) {
      values.push({
        column: parseColumnReference(colRefEl),
        expression: scalarOpEl?.getAttribute('ScalarString') || undefined,
      });
    }
  }
  
  return values;
}

/**
 * Parse NestedLoops
 */
function parseNestedLoops(nestedLoopsEl: Element): NestedLoopsDetails {
  const outerRefsEl = getChildElement(nestedLoopsEl, 'OuterReferences');
  
  return {
    optimized: nestedLoopsEl.getAttribute('Optimized') === 'true',
    outerReferences: outerRefsEl ? 
      getChildElements(outerRefsEl, 'ColumnReference').map(parseColumnReference) : undefined,
  };
}

// Helper functions for namespace-agnostic element access

function getChildElement(parent: Element, localName: string): Element | null {
  // Search direct children only
  for (const child of parent.children) {
    if (child.localName === localName) return child;
  }
  
  return null;
}

function getChildElements(parent: Element, localName: string): Element[] {
  const results: Element[] = [];
  
  // Search direct children
  for (const child of parent.children) {
    if (child.localName === localName) {
      results.push(child);
    }
  }
  
  return results;
}

function createEmptyQueryPlan(): QueryPlan {
  return {
    degreeOfParallelism: 1,
    relOp: createEmptyRelOp(),
  };
}

function createEmptyRelOp(): RelOp {
  return {
    nodeId: 0,
    physicalOp: 'Unknown',
    logicalOp: '',
    estimateRows: 0,
    estimateCPU: 0,
    estimateIO: 0,
    estimatedTotalSubtreeCost: 0,
    avgRowSize: 0,
    parallel: false,
    attributes: {},
    outputColumns: [],
    children: [],
    operationDetails: {},
  };
}

/**
 * Collect all attributes from an element
 */
function collectAttributes(el: Element): Record<string, string> {
  const attrs: Record<string, string> = {};
  for (let i = 0; i < el.attributes.length; i++) {
    const attr = el.attributes[i];
    attrs[attr.name] = attr.value;
  }
  return attrs;
}

/**
 * Flatten the RelOp tree into an array for easier iteration
 */
export function flattenRelOps(relOp: RelOp): RelOp[] {
  const result: RelOp[] = [relOp];
  for (const child of relOp.children) {
    result.push(...flattenRelOps(child));
  }
  return result;
}

/**
 * Calculate the total cost of the plan for percentage calculations
 */
export function getTotalCost(plan: ShowPlanXML): number {
  let total = 0;
  for (const batch of plan.batches) {
    for (const stmt of batch.statements) {
      total += stmt.statementSubTreeCost;
    }
  }
  return total;
}

/**
 * Get the statement with the highest cost
 */
export function getMostExpensiveStatement(plan: ShowPlanXML): Statement | null {
  let maxCost = 0;
  let result: Statement | null = null;
  
  for (const batch of plan.batches) {
    for (const stmt of batch.statements) {
      if (stmt.statementSubTreeCost > maxCost) {
        maxCost = stmt.statementSubTreeCost;
        result = stmt;
      }
    }
  }
  
  return result;
}
