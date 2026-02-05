<script setup lang="ts">
import { ref } from 'vue';
import { usePlanState, type Capsule } from '../composables/planState';

const { injectRootCapsule } = usePlanState();

const fileInput = ref<HTMLInputElement>();
const dragEngaged = ref(false);
const transmissionLog = ref<string>('');

const engageFileSelector = () => {
  fileInput.value?.click();
};

const handleFileCapture = async (evt: Event) => {
  const inputElement = evt.target as HTMLInputElement;
  const capturedFile = inputElement.files?.[0];
  if (capturedFile) {
    await digestArtifact(capturedFile);
  }
};

const onDragEngagement = (evt: DragEvent) => {
  evt.preventDefault();
  dragEngaged.value = true;
};

const onDragDisengagement = () => {
  dragEngaged.value = false;
};

const onFileRelease = async (evt: DragEvent) => {
  evt.preventDefault();
  dragEngaged.value = false;
  
  const releasedFile = evt.dataTransfer?.files[0];
  if (releasedFile) {
    await digestArtifact(releasedFile);
  }
};

const digestArtifact = async (file: File) => {
  try {
    transmissionLog.value = 'Ingesting artifact stream...';
    const rawPayload = await file.text();
    
    transmissionLog.value = 'Decoding JSON structure...';
    const decodedPayload = JSON.parse(rawPayload);
    
    transmissionLog.value = 'Transmuting to capsule format...';
    const capsule = transmuteToCapsule(decodedPayload);
    
    injectRootCapsule(capsule);
    transmissionLog.value = 'Transmission complete!';
    
    setTimeout(() => {
      transmissionLog.value = '';
    }, 4000);
  } catch (err) {
    transmissionLog.value = `Transmission error: ${err instanceof Error ? err.message : 'Unknown failure'}`;
  }
};

const transmuteToCapsule = (raw: any): Capsule => {
  const synthesizeKey = () => `cap_${Math.random().toString(36).substring(2, 11)}_${Date.now().toString(36)}`;
  
  const recursiveTransmute = (obj: any): Capsule => {
    return {
      keystone: obj.id || obj.nodeId || obj.identifier || obj.key || synthesizeKey(),
      nomenclature: obj.operator || obj.operation || obj.nodeType || obj.type || obj.name || 'UNDEFINED_OPERATION',
      magnitude: obj.rows || obj.estimatedRows || obj.rowCount || obj.cardinality || obj.tuples || Math.floor(Math.random() * 100000),
      temporalCost: obj.time || obj.duration || obj.actualTime || obj.executionTime || obj.latency || Math.floor(Math.random() * 500),
      densityFactor: obj.cost || obj.weight || obj.priority || obj.complexity || obj.factor || Math.random(),
      offspring: (obj.children || obj.inputs || obj.subNodes || obj.dependencies || obj.steps || []).map(recursiveTransmute),
      annotations: {
        ...obj,
        children: undefined,
        inputs: undefined,
        subNodes: undefined,
        dependencies: undefined,
        steps: undefined
      }
    };
  };
  
  return recursiveTransmute(raw);
};

const materializePrototype = () => {
  const prototype: Capsule = {
    keystone: 'proto_root_alpha',
    nomenclature: 'DISTRIBUTED_HASH_AGGREGATE',
    magnitude: 42000,
    temporalCost: 580,
    densityFactor: 0.87,
    annotations: { distribution: 'hash_partitioned', parallelism: 8, memoryBuffer: '4.8MB' },
    offspring: [
      {
        keystone: 'proto_scan_beta',
        nomenclature: 'PARALLEL_TABLE_SCAN',
        magnitude: 160000,
        temporalCost: 280,
        densityFactor: 0.74,
        annotations: { relation: 'transactions', filterClause: 'timestamp >= 2024-01-01', scanMode: 'parallel' },
        offspring: []
      },
      {
        keystone: 'proto_index_gamma',
        nomenclature: 'COMPOSITE_INDEX_SEEK',
        magnitude: 26000,
        temporalCost: 155,
        densityFactor: 0.59,
        annotations: { indexStructure: 'orders_composite_idx', seekKeys: ['customer_id', 'order_date'] },
        offspring: [
          {
            keystone: 'proto_filter_delta',
            nomenclature: 'VECTORIZED_FILTER',
            magnitude: 9000,
            temporalCost: 52,
            densityFactor: 0.35,
            annotations: { predicateExpression: 'order_total > 5000 AND status = active', vectorized: true },
            offspring: []
          },
          {
            keystone: 'proto_sort_epsilon',
            nomenclature: 'EXTERNAL_MERGE_SORT',
            magnitude: 9000,
            temporalCost: 89,
            densityFactor: 0.51,
            annotations: { sortKeys: ['order_date DESC', 'priority ASC'], algorithm: 'external_merge', spillToDisk: false },
            offspring: []
          }
        ]
      }
    ]
  };
  
  injectRootCapsule(prototype);
  transmissionLog.value = 'Prototype materialized successfully!';
  setTimeout(() => {
    transmissionLog.value = '';
  }, 4000);
};
</script>

<template>
  <div class="ingestion-terminal">
    <h3 class="terminal-header">📡 Artifact Ingestion Terminal</h3>
    
    <div 
      class="drop-receptor"
      :class="{ 'drag-engaged': dragEngaged }"
      @click="engageFileSelector"
      @dragover="onDragEngagement"
      @dragleave="onDragDisengagement"
      @drop="onFileRelease"
    >
      <div class="receptor-icon">📦</div>
      <p class="receptor-instruction">Release JSON artifact or engage selector</p>
      <p class="receptor-specification">SQL execution plan artifact</p>
    </div>
    
    <input
      ref="fileInput"
      type="file"
      accept=".json"
      style="display: none"
      @change="handleFileCapture"
    />
    
    <button class="prototype-materializer" @click="materializePrototype">
      ⚡ Materialize Prototype Schema
    </button>
    
    <div v-if="transmissionLog" class="transmission-feedback" :class="{ 'error-state': transmissionLog.includes('error') }">
      {{ transmissionLog }}
    </div>
  </div>
</template>

<style scoped>
.ingestion-terminal {
  background: linear-gradient(148deg, #2a1a5e 0%, #3d2471 100%);
  padding: 30px;
  border-radius: 22px;
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.7);
}

.terminal-header {
  margin: 0 0 26px 0;
  color: #ffffff;
  font-size: 25px;
  font-weight: 900;
}

.drop-receptor {
  background: rgba(255, 255, 255, 0.18);
  border: 5px dashed rgba(255, 255, 255, 0.4);
  border-radius: 20px;
  padding: 60px 30px;
  text-align: center;
  cursor: pointer;
  transition: all 0.4s cubic-bezier(0.34, 1.56, 0.64, 1);
  margin-bottom: 26px;
}

.drop-receptor:hover {
  background: rgba(255, 255, 255, 0.24);
  border-color: rgba(255, 255, 255, 0.6);
  transform: translateY(-6px) scale(1.02);
}

.drop-receptor.drag-engaged {
  background: rgba(130, 200, 255, 0.35);
  border-color: #82c8ff;
  transform: scale(1.06);
  box-shadow: 0 0 30px rgba(130, 200, 255, 0.5);
}

.receptor-icon {
  font-size: 74px;
  margin-bottom: 20px;
}

.receptor-instruction {
  color: #ffffff;
  font-size: 20px;
  font-weight: 800;
  margin: 0 0 13px 0;
}

.receptor-specification {
  color: rgba(255, 255, 255, 0.85);
  font-size: 17px;
  margin: 0;
}

.prototype-materializer {
  width: 100%;
  padding: 22px 40px;
  background: linear-gradient(135deg, #c62828 0%, #e53935 100%);
  border: none;
  border-radius: 16px;
  color: #ffffff;
  font-size: 19px;
  font-weight: 900;
  cursor: pointer;
  transition: all 0.4s ease;
  box-shadow: 0 8px 26px rgba(229, 57, 53, 0.65);
}

.prototype-materializer:hover {
  transform: translateY(-6px);
  box-shadow: 0 14px 36px rgba(229, 57, 53, 0.85);
}

.prototype-materializer:active {
  transform: translateY(-3px);
}

.transmission-feedback {
  margin-top: 22px;
  padding: 20px;
  background: rgba(76, 175, 80, 0.35);
  border-left: 8px solid #4caf50;
  border-radius: 12px;
  color: #ffffff;
  font-size: 17px;
  font-weight: 800;
}

.transmission-feedback.error-state {
  background: rgba(244, 67, 54, 0.35);
  border-left-color: #f44336;
}
</style>
