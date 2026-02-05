<script setup lang="ts">
import { computed } from 'vue';
import { usePlanState, type Capsule } from '../composables/planState';

const { centralState } = usePlanState();

interface Diagnostic {
  urgency: 'critical' | 'warning' | 'advisory';
  title: string;
  analysis: string;
  severity: number;
}

const traverseHierarchy = (capsule: Capsule | null): Capsule[] => {
  if (!capsule) return [];
  const accumulator: Capsule[] = [capsule];
  capsule.offspring?.forEach(child => {
    accumulator.push(...traverseHierarchy(child));
  });
  return accumulator;
};

const diagnosticReports = computed((): Diagnostic[] => {
  if (!centralState.rootCapsule) return [];
  
  const hierarchy = traverseHierarchy(centralState.rootCapsule);
  const diagnostics: Diagnostic[] = [];
  
  hierarchy.forEach(cap => {
    if (cap.nomenclature.includes('TABLE') && cap.nomenclature.includes('SCAN') && cap.magnitude > 40000) {
      diagnostics.push({
        urgency: 'warning',
        title: 'Excessive Table Scanning Detected',
        analysis: `Operation "${cap.nomenclature}" (ID: ${cap.keystone}) processes ${cap.magnitude.toLocaleString()} records via sequential scanning. Strategic index deployment recommended for performance amplification.`,
        severity: Math.min(100, cap.magnitude / 1000)
      });
    }
    
    if (cap.temporalCost > 400) {
      diagnostics.push({
        urgency: 'critical',
        title: 'Critical Temporal Overhead',
        analysis: `Operation "${cap.nomenclature}" (ID: ${cap.keystone}) incurs ${cap.temporalCost}ms temporal cost. This operation represents a major bottleneck in execution pipeline.`,
        severity: Math.min(100, cap.temporalCost / 5)
      });
    }
    
    if ((cap.nomenclature.includes('HASH') || cap.nomenclature.includes('MERGE')) && cap.offspring?.length >= 2) {
      const totalMagnitude = cap.offspring.reduce((sum, child) => sum + child.magnitude, 0);
      if (totalMagnitude > 200000) {
        diagnostics.push({
          urgency: 'warning',
          title: 'High-Volume Coordination Operation',
          analysis: `Coordination operation "${cap.nomenclature}" (ID: ${cap.keystone}) manages ${totalMagnitude.toLocaleString()} records across offspring nodes. Consider alternative execution strategies for better resource utilization.`,
          severity: Math.min(100, totalMagnitude / 5000)
        });
      }
    }
    
    if (cap.densityFactor > 0.8) {
      diagnostics.push({
        urgency: 'advisory',
        title: 'Elevated Density Factor',
        analysis: `Operation "${cap.nomenclature}" (ID: ${cap.keystone}) exhibits density factor of ${cap.densityFactor.toFixed(2)}. Resource monitoring recommended for sustained operational loads.`,
        severity: cap.densityFactor * 70
      });
    }
  });
  
  return diagnostics.sort((a, b) => b.severity - a.severity);
});

const knowledgeRepository = computed(() => {
  if (!centralState.rootCapsule) return [];
  
  const compendium: Record<string, string> = {
    'TABLE_SCAN': 'Full table scanning processes every record sequentially without index acceleration. Optimal for small datasets or bulk operations requiring complete traversal. For selective queries on substantial datasets, strategic indexing fundamentally transforms performance characteristics.',
    'INDEX': 'Index-based operations leverage tree structures (commonly B+ trees) for logarithmic-time data access. Critical for selective query patterns. Statistical currency directly influences optimizer decision quality.',
    'HASH': 'Hash-based operations construct in-memory lookup structures enabling constant-time record matching. Superior for equality-based joins on large data volumes. Insufficient memory allocation triggers disk-based fallback with substantial performance degradation.',
    'MERGE': 'Merge operations assume pre-sorted input streams, eliminating memory pressure and sort overhead. Highly efficient when leveraging existing sort orders from index structures. Maintains linear computational complexity.',
    'SORT': 'Sorting operations reorder records by specified attributes. Can dominate total execution time under memory constraints. Leverage pre-sorted index streams or increase memory allocation to eliminate explicit sorting operations.',
    'FILTER': 'Filtering eliminates non-qualifying records early in the execution pipeline. Predicate pushdown maximizes efficiency by reducing data movement. Partial indexes embed common filter patterns directly in storage structures.',
    'AGGREGATE': 'Aggregation operations compute summary statistics across record groups. Hash-based aggregation provides optimal speed but requires substantial memory. Materialized views precompute frequently-accessed aggregations.',
    'PARALLEL': 'Parallel execution distributes computational workload across multiple processing threads. Excellent for CPU-intensive operations on large datasets. Adequate memory provisioning prevents degradation from disk spillage.',
    'DISTRIBUTED': 'Distributed operations partition workload across multiple nodes for horizontal scalability. Essential for processing datasets exceeding single-node capacity. Network latency and data shuffling represent key optimization targets.'
  };
  
  const rootCap = centralState.rootCapsule;
  const relevantKnowledge = Object.keys(compendium)
    .filter(keyword => rootCap.nomenclature.toUpperCase().includes(keyword))
    .map(keyword => ({
      topic: keyword,
      content: compendium[keyword]
    }));
  
  return relevantKnowledge;
});

const urgencySymbol = (urgency: string) => {
  if (urgency === 'critical') return '🔴';
  if (urgency === 'warning') return '⚠️';
  return '💡';
};

const urgencyTheme = (urgency: string) => {
  if (urgency === 'critical') return 'critical-diagnostic';
  if (urgency === 'warning') return 'warning-diagnostic';
  return 'advisory-diagnostic';
};
</script>

<template>
  <div class="intelligence-hub">
    <div class="hub-sector">
      <h3 class="sector-title">🔬 Diagnostic Analysis</h3>
      
      <div v-if="diagnosticReports.length === 0" class="nominal-state">
        <span class="nominal-symbol">✓</span>
        <p>All systems operating within nominal parameters</p>
      </div>
      
      <div v-else class="diagnostic-collection">
        <div 
          v-for="(diagnostic, idx) in diagnosticReports" 
          :key="idx"
          class="diagnostic-card"
          :class="urgencyTheme(diagnostic.urgency)"
        >
          <div class="diagnostic-header">
            <span class="urgency-indicator">{{ urgencySymbol(diagnostic.urgency) }}</span>
            <span class="diagnostic-title">{{ diagnostic.title }}</span>
          </div>
          <p class="diagnostic-analysis">{{ diagnostic.analysis }}</p>
          <div class="severity-visualizer">
            <div class="severity-label">Severity Level:</div>
            <div class="severity-track">
              <div 
                class="severity-beam"
                :style="{ width: `${diagnostic.severity}%` }"
              ></div>
            </div>
          </div>
        </div>
      </div>
    </div>
    
    <div class="hub-sector">
      <h3 class="sector-title">📚 Knowledge Repository</h3>
      
      <div v-if="knowledgeRepository.length === 0" class="nominal-state">
        <p>Select an operation to access knowledge repository</p>
      </div>
      
      <div v-else class="knowledge-collection">
        <div 
          v-for="(entry, idx) in knowledgeRepository" 
          :key="idx"
          class="knowledge-card"
        >
          <div class="knowledge-topic">{{ entry.topic }}</div>
          <p class="knowledge-content">{{ entry.content }}</p>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.intelligence-hub {
  background: linear-gradient(174deg, #1a1a2e 0%, #16213e 100%);
  border-radius: 22px;
  padding: 34px;
  color: #f0f0f0;
  height: 100%;
  overflow-y: auto;
}

.hub-sector {
  margin-bottom: 44px;
}

.hub-sector:last-child {
  margin-bottom: 0;
}

.sector-title {
  margin: 0 0 26px 0;
  font-size: 28px;
  font-weight: 900;
  color: #29b6f6;
}

.nominal-state {
  text-align: center;
  padding: 50px;
  opacity: 0.65;
}

.nominal-symbol {
  font-size: 80px;
  display: block;
  margin-bottom: 20px;
  color: #00e676;
}

.diagnostic-collection, .knowledge-collection {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.diagnostic-card {
  background: rgba(255, 255, 255, 0.07);
  border-left: 8px solid;
  border-radius: 16px;
  padding: 24px;
  transition: transform 0.3s, box-shadow 0.3s;
}

.diagnostic-card:hover {
  transform: translateX(8px);
  box-shadow: 0 8px 28px rgba(0, 0, 0, 0.5);
}

.critical-diagnostic {
  border-left-color: #ff1744;
  background: rgba(255, 23, 68, 0.14);
}

.warning-diagnostic {
  border-left-color: #ffc400;
  background: rgba(255, 196, 0, 0.14);
}

.advisory-diagnostic {
  border-left-color: #00e5ff;
  background: rgba(0, 229, 255, 0.14);
}

.diagnostic-header {
  display: flex;
  align-items: center;
  gap: 13px;
  margin-bottom: 13px;
}

.urgency-indicator {
  font-size: 28px;
}

.diagnostic-title {
  font-size: 20px;
  font-weight: 900;
  color: #ffffff;
}

.diagnostic-analysis {
  margin: 0 0 20px 0;
  font-size: 18px;
  line-height: 1.85;
  color: #c0c0c0;
}

.severity-visualizer {
  display: flex;
  align-items: center;
  gap: 20px;
}

.severity-label {
  font-size: 15px;
  font-weight: 900;
  text-transform: uppercase;
  letter-spacing: 2px;
  opacity: 0.95;
}

.severity-track {
  flex: 1;
  height: 13px;
  background: rgba(255, 255, 255, 0.17);
  border-radius: 8px;
  overflow: hidden;
}

.severity-beam {
  height: 100%;
  background: linear-gradient(90deg, #00e676 0%, #ffc400 50%, #ff1744 100%);
  transition: width 0.8s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.knowledge-card {
  background: rgba(41, 182, 246, 0.14);
  border-left: 8px solid #29b6f6;
  border-radius: 16px;
  padding: 24px;
}

.knowledge-topic {
  font-size: 18px;
  font-weight: 900;
  text-transform: uppercase;
  letter-spacing: 2px;
  color: #29b6f6;
  margin-bottom: 13px;
}

.knowledge-content {
  margin: 0;
  font-size: 18px;
  line-height: 1.95;
  color: #c0c0c0;
}
</style>
