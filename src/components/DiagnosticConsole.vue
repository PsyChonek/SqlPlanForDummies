<script setup lang="ts">
import { computed } from 'vue';
import { usePlanState } from '../composables/planState';

const { centralState } = usePlanState();

const telemetryReadings = computed(() => {
  if (!centralState.inspectedCapsule) return [];
  
  const cap = centralState.inspectedCapsule;
  return [
    { 
      sensor: 'Magnitude', 
      value: cap.magnitude.toLocaleString(), 
      spectrum: '#00e0ff' 
    },
    { 
      sensor: 'Temporal Cost', 
      value: `${cap.temporalCost}ms`, 
      spectrum: '#ff6500' 
    },
    { 
      sensor: 'Density Factor', 
      value: cap.densityFactor.toFixed(3), 
      spectrum: '#a800ff' 
    },
    { 
      sensor: 'Offspring Count', 
      value: cap.offspring?.length || 0, 
      spectrum: '#00e666' 
    }
  ];
});

const annotationPairs = computed(() => {
  if (!centralState.inspectedCapsule?.annotations) return [];
  
  const excludeFields = ['keystone', 'nomenclature', 'magnitude', 'temporalCost', 'densityFactor', 'offspring', 'annotations'];
  
  return Object.entries(centralState.inspectedCapsule.annotations)
    .filter(([k]) => !excludeFields.includes(k))
    .map(([k, v]) => ({
      field: k,
      content: typeof v === 'object' ? JSON.stringify(v, null, 2) : String(v)
    }));
});

const efficiencyGrade = computed(() => {
  if (!centralState.inspectedCapsule) return 'UNKNOWN';
  
  const cap = centralState.inspectedCapsule;
  const ratio = cap.magnitude > 0 ? cap.temporalCost / cap.magnitude : cap.temporalCost;
  
  if (ratio < 0.001) return 'STELLAR';
  if (ratio < 0.01) return 'ROBUST';
  if (ratio < 0.1) return 'ADEQUATE';
  return 'SUBOPTIMAL';
});

const gradeSpectrum = computed(() => {
  const spectrumMap: Record<string, string> = {
    'STELLAR': '#00e666',
    'ROBUST': '#00e0ff',
    'ADEQUATE': '#ffc000',
    'SUBOPTIMAL': '#ff1744'
  };
  return spectrumMap[efficiencyGrade.value] || '#757575';
});

const calculateHealthScore = computed(() => {
  if (!centralState.inspectedCapsule) return 0;
  
  const cap = centralState.inspectedCapsule;
  let score = 100;
  
  if (cap.temporalCost > 300) score -= 30;
  else if (cap.temporalCost > 150) score -= 15;
  
  if (cap.magnitude > 50000) score -= 25;
  else if (cap.magnitude > 20000) score -= 10;
  
  if (cap.densityFactor > 0.8) score -= 20;
  else if (cap.densityFactor > 0.6) score -= 10;
  
  return Math.max(0, score);
});

const healthColor = computed(() => {
  const score = calculateHealthScore.value;
  if (score >= 80) return '#00e676';
  if (score >= 60) return '#64d8ff';
  if (score >= 40) return '#ffb300';
  return '#ff1744';
});
</script>

<template>
  <div class="diagnostic-chamber">
    <div v-if="!centralState.inspectedCapsule" class="standby-mode">
      <div class="standby-symbol">⚡</div>
      <p class="standby-text">Awaiting capsule selection</p>
    </div>
    
    <div v-else class="active-diagnostics">
      <div class="title-bar">
        <h2 class="nomenclature-display">{{ centralState.inspectedCapsule.nomenclature }}</h2>
        <span class="keystone-badge">{{ centralState.inspectedCapsule.keystone }}</span>
      </div>
      
      <div class="telemetry-grid">
        <div 
          v-for="reading in telemetryReadings" 
          :key="reading.sensor"
          class="telemetry-tile"
          :style="{ borderTopColor: reading.spectrum }"
        >
          <div class="sensor-label">{{ reading.sensor }}</div>
          <div class="sensor-reading">{{ reading.value }}</div>
        </div>
      </div>
      
      <div class="health-monitor">
        <div class="monitor-label">System Health Score</div>
        <div class="health-bar-container">
          <div 
            class="health-bar-fill"
            :style="{ width: `${calculateHealthScore}%`, backgroundColor: healthColor }"
          ></div>
        </div>
        <div class="health-percentage" :style="{ color: healthColor }">
          {{ calculateHealthScore }}%
        </div>
      </div>
      
      <div class="efficiency-panel">
        <div class="panel-label">Efficiency Grade</div>
        <div class="efficiency-display" :style="{ color: gradeSpectrum }">
          {{ efficiencyGrade }}
        </div>
      </div>
      
      <div class="annotations-panel" v-if="annotationPairs.length > 0">
        <div class="panel-label">Annotation Data</div>
        <div class="annotation-list">
          <div 
            v-for="pair in annotationPairs" 
            :key="pair.field"
            class="annotation-entry"
          >
            <span class="annotation-field">{{ pair.field }}</span>
            <span class="annotation-content">{{ pair.content }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.diagnostic-chamber {
  background: linear-gradient(162deg, #1c1c30 0%, #0d0d1a 100%);
  border-radius: 22px;
  padding: 34px;
  color: #f0f0f0;
  height: 100%;
  overflow-y: auto;
  box-shadow: inset 0 7px 21px rgba(0, 0, 0, 0.8);
}

.standby-mode {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  opacity: 0.5;
}

.standby-symbol {
  font-size: 96px;
  margin-bottom: 22px;
  animation: flicker 2.5s ease-in-out infinite;
}

@keyframes flicker {
  0%, 100% { opacity: 0.5; transform: scale(1); }
  50% { opacity: 1; transform: scale(1.1); }
}

.standby-text {
  font-size: 19px;
  color: #9e9e9e;
}

.active-diagnostics {
  display: flex;
  flex-direction: column;
  gap: 30px;
}

.title-bar {
  border-bottom: 4px solid #3a5f8b;
  padding-bottom: 18px;
}

.nomenclature-display {
  margin: 0 0 14px 0;
  font-size: 34px;
  font-weight: 900;
  color: #29b6f6;
  letter-spacing: -1.5px;
}

.keystone-badge {
  background: #3a5f8b;
  padding: 9px 20px;
  border-radius: 36px;
  font-size: 16px;
  font-family: 'SF Mono', 'Monaco', monospace;
  color: #b0d9ff;
}

.telemetry-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 22px;
}

.telemetry-tile {
  background: #16162a;
  padding: 24px;
  border-radius: 16px;
  border-top: 6px solid;
  transition: transform 0.3s, box-shadow 0.3s;
}

.telemetry-tile:hover {
  transform: translateY(-6px);
  box-shadow: 0 12px 36px rgba(0, 0, 0, 0.6);
}

.sensor-label {
  font-size: 13px;
  text-transform: uppercase;
  letter-spacing: 2.5px;
  opacity: 0.8;
  margin-bottom: 11px;
  font-weight: 900;
}

.sensor-reading {
  font-size: 28px;
  font-weight: 900;
  letter-spacing: -1.5px;
}

.health-monitor, .efficiency-panel, .annotations-panel {
  background: #16162a;
  padding: 26px;
  border-radius: 16px;
}

.monitor-label, .panel-label {
  font-size: 16px;
  font-weight: 900;
  text-transform: uppercase;
  letter-spacing: 2.5px;
  margin-bottom: 20px;
  opacity: 0.9;
  color: #9fa8da;
}

.health-bar-container {
  height: 16px;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  overflow: hidden;
  margin-bottom: 14px;
}

.health-bar-fill {
  height: 100%;
  transition: width 0.8s cubic-bezier(0.4, 0, 0.2, 1), background-color 0.5s;
  border-radius: 8px;
}

.health-percentage {
  font-size: 32px;
  font-weight: 900;
  text-align: center;
}

.efficiency-display {
  font-size: 44px;
  font-weight: 900;
  text-align: center;
  padding: 22px;
  background: rgba(255, 255, 255, 0.08);
  border-radius: 16px;
  text-shadow: 0 4px 10px rgba(0, 0, 0, 0.6);
  letter-spacing: 3px;
}

.annotation-list {
  display: flex;
  flex-direction: column;
  gap: 13px;
}

.annotation-entry {
  display: flex;
  justify-content: space-between;
  padding: 17px;
  background: rgba(255, 255, 255, 0.06);
  border-radius: 9px;
  font-size: 16px;
  gap: 22px;
}

.annotation-field {
  font-weight: 900;
  color: #64d8ff;
  min-width: 140px;
}

.annotation-content {
  font-family: 'SF Mono', 'Monaco', monospace;
  color: #d8d8d8;
  text-align: right;
  word-break: break-word;
  flex: 1;
}
</style>
