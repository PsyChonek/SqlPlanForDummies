import { reactive, computed } from 'vue';
import type { ShowPlanXML, Statement, RelOp } from '../types/sqlplan';
import { parseSqlPlan, flattenRelOps, getTotalCost } from './sqlPlanParser';

// Edge selection type
export interface SelectedEdge {
  source: RelOp;
  target: RelOp;
}

// Application state
interface AppState {
  plan: ShowPlanXML | null;
  selectedStatement: Statement | null;
  selectedNode: RelOp | null;
  selectedEdge: SelectedEdge | null;
  revision: number;
  error: string | null;
  loading: boolean;
  // Comparison mode
  comparisonMode: boolean;
  comparisonPlan: ShowPlanXML | null;
  comparisonStatement: Statement | null;
}

const state = reactive<AppState>({
  plan: null,
  selectedStatement: null,
  selectedNode: null,
  selectedEdge: null,
  revision: 0,
  error: null,
  loading: false,
  comparisonMode: false,
  comparisonPlan: null,
  comparisonStatement: null,
});

export const usePlanState = () => {
  // Load a plan from XML string
  const loadPlan = (xmlString: string) => {
    state.loading = true;
    state.error = null;
    
    try {
      const plan = parseSqlPlan(xmlString);
      state.plan = plan;
      
      // Auto-select the first statement if available
      if (plan.batches.length > 0 && plan.batches[0].statements.length > 0) {
        state.selectedStatement = plan.batches[0].statements[0];
        console.log('[PlanState] Loaded plan with statement:', state.selectedStatement.statementText.substring(0, 50));
      } else {
        console.warn('[PlanState] No statements found in plan');
      }
      
      state.selectedNode = null;
      state.revision++;
    } catch (err) {
      state.error = err instanceof Error ? err.message : 'Failed to parse plan';
      state.plan = null;
      console.error('[PlanState] Failed to parse plan:', err);
    } finally {
      state.loading = false;
    }
  };

  // Select a statement for visualization
  const selectStatement = (statement: Statement | null) => {
    state.selectedStatement = statement;
    state.selectedNode = null;
    state.revision++;
  };

  // Select a node for detailed view
  const selectNode = (node: RelOp | null) => {
    state.selectedNode = node;
    state.selectedEdge = null;
  };

  // Select an edge for detailed view
  const selectEdge = (edge: SelectedEdge | null) => {
    state.selectedEdge = edge;
    state.selectedNode = null;
  };

  // Clear the current plan
  const clearPlan = () => {
    state.plan = null;
    state.selectedStatement = null;
    state.selectedNode = null;
    state.error = null;
    state.comparisonMode = false;
    state.comparisonPlan = null;
    state.comparisonStatement = null;
    state.revision++;
  };

  // Load a comparison plan
  const loadComparisonPlan = (xmlString: string) => {
    try {
      const plan = parseSqlPlan(xmlString);
      state.comparisonPlan = plan;
      state.comparisonMode = true;
      
      // Auto-select the first statement if available
      if (plan.batches.length > 0 && plan.batches[0].statements.length > 0) {
        state.comparisonStatement = plan.batches[0].statements[0];
      }
      
      state.revision++;
    } catch (err) {
      state.error = err instanceof Error ? err.message : 'Failed to parse comparison plan';
    }
  };

  // Select a statement for comparison
  const selectComparisonStatement = (statement: Statement | null) => {
    state.comparisonStatement = statement;
    state.revision++;
  };

  // Toggle comparison mode
  const toggleComparisonMode = () => {
    if (state.comparisonMode) {
      state.comparisonMode = false;
      state.comparisonPlan = null;
      state.comparisonStatement = null;
    } else {
      state.comparisonMode = true;
    }
    state.revision++;
  };

  // Clear comparison plan
  const clearComparisonPlan = () => {
    state.comparisonPlan = null;
    state.comparisonStatement = null;
    if (!state.plan) {
      state.comparisonMode = false;
    }
    state.revision++;
  };

  // Computed: comparison statements list
  const comparisonStatements = computed(() => {
    if (!state.comparisonPlan) return [];
    const result: Statement[] = [];
    for (const batch of state.comparisonPlan.batches) {
      result.push(...batch.statements);
    }
    return result;
  });

  // Computed: total plan cost
  const totalCost = computed(() => {
    return state.plan ? getTotalCost(state.plan) : 0;
  });

  // Computed: all nodes in current statement
  const allNodes = computed(() => {
    if (!state.selectedStatement) return [];
    return flattenRelOps(state.selectedStatement.queryPlan.relOp);
  });

  // Computed: list of all statements
  const statements = computed(() => {
    if (!state.plan) return [];
    const result: Statement[] = [];
    for (const batch of state.plan.batches) {
      result.push(...batch.statements);
    }
    return result;
  });

  // Get cost percentage for a node
  const getNodeCostPercentage = (node: RelOp): number => {
    if (totalCost.value === 0) return 0;
    return (node.estimatedTotalSubtreeCost / totalCost.value) * 100;
  };

  // Keyboard navigation helpers
  const navigateToParent = () => {
    if (!state.selectedNode || !state.selectedStatement) return;
    const nodes = allNodes.value;
    
    // Find parent node (node that has current as child)
    for (const node of nodes) {
      if (node.children.some(c => c.nodeId === state.selectedNode?.nodeId)) {
        state.selectedNode = node;
        return;
      }
    }
  };

  const navigateToFirstChild = () => {
    if (!state.selectedNode) return;
    if (state.selectedNode.children.length > 0) {
      state.selectedNode = state.selectedNode.children[0];
    }
  };

  const navigateToSibling = (direction: 'prev' | 'next') => {
    if (!state.selectedNode || !state.selectedStatement) return;
    const nodes = allNodes.value;
    
    // Find parent and then sibling
    let parentNode: RelOp | null = null;
    for (const node of nodes) {
      if (node.children.some(c => c.nodeId === state.selectedNode?.nodeId)) {
        parentNode = node;
        break;
      }
    }
    
    if (!parentNode) {
      // Current node is root, try to navigate between statements
      return;
    }
    
    const siblings = parentNode.children;
    const currentIndex = siblings.findIndex(s => s.nodeId === state.selectedNode?.nodeId);
    
    if (direction === 'prev' && currentIndex > 0) {
      state.selectedNode = siblings[currentIndex - 1];
    } else if (direction === 'next' && currentIndex < siblings.length - 1) {
      state.selectedNode = siblings[currentIndex + 1];
    }
  };

  const selectFirstNode = () => {
    if (!state.selectedStatement) return;
    state.selectedNode = state.selectedStatement.queryPlan.relOp;
  };

  return {
    // State (readonly access)
    state,
    
    // Actions
    loadPlan,
    selectStatement,
    selectNode,
    selectEdge,
    clearPlan,
    
    // Comparison
    loadComparisonPlan,
    selectComparisonStatement,
    toggleComparisonMode,
    clearComparisonPlan,
    comparisonStatements,
    
    // Computed
    totalCost,
    allNodes,
    statements,
    
    // Utilities
    getNodeCostPercentage,
    
    // Navigation
    navigateToParent,
    navigateToFirstChild,
    navigateToSibling,
    selectFirstNode,
  };
};

// Legacy support - map old Capsule type to RelOp
export interface Capsule {
  keystone: string;
  nomenclature: string;
  magnitude: number;
  temporalCost: number;
  densityFactor: number;
  offspring: Capsule[];
  annotations: Record<string, any>;
}

// Legacy central state for backwards compatibility
export const centralState = reactive({
  rootCapsule: null as Capsule | null,
  inspectedCapsule: null as Capsule | null,
  revision: 0,
});

