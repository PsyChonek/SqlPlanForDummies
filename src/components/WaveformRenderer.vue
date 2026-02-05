<script setup lang="ts">
import { ref, onMounted, watch } from 'vue';
import * as d3 from 'd3';
import { usePlanState, type Capsule } from '../composables/planState';

const { centralState, setInspectedCapsule } = usePlanState();

const viewport = ref<HTMLDivElement>();
const surfaceWidth = ref(980);
const surfaceHeight = ref(760);

let renderContext: any = null;
let activeKeystone: string | null = null;

interface WavePoint {
  amplitude: number;
  phase: number;
  frequency: number;
  capsule: Capsule;
  depth: number;
}

const generateWaveform = (cap: Capsule, depthLevel = 0, lateralShift = 0): WavePoint[] => {
  const wavePoints: WavePoint[] = [];
  const phaseShift = (lateralShift * Math.PI) / 180;
  
  wavePoints.push({
    amplitude: 50 + cap.densityFactor * 40,
    phase: phaseShift,
    frequency: 0.5 + depthLevel * 0.3,
    capsule: cap,
    depth: depthLevel
  });
  
  if (cap.offspring && cap.offspring.length > 0) {
    const spreadAngle = 360 / cap.offspring.length;
    cap.offspring.forEach((child, idx) => {
      const childShift = lateralShift + (idx - (cap.offspring.length - 1) / 2) * spreadAngle;
      const childPoints = generateWaveform(child, depthLevel + 1, childShift);
      wavePoints.push(...childPoints);
    });
  }
  
  return wavePoints;
};

const calculatePolarCoords = (phase: number, depth: number) => {
  const radius = 100 + depth * 110;
  const angle = phase + Math.sin(depth * 0.5) * 0.3;
  return {
    x: surfaceWidth.value / 2 + Math.cos(angle) * radius,
    y: surfaceHeight.value / 2 + Math.sin(angle) * radius
  };
};

const paintSurface = () => {
  if (!viewport.value || !centralState.rootCapsule) return;
  
  d3.select(viewport.value).selectAll('*').remove();
  
  const canvas = d3.select(viewport.value)
    .append('svg')
    .attr('width', surfaceWidth.value)
    .attr('height', surfaceHeight.value);
  
  const defs = canvas.append('defs');
  const gradientId = 'bg-gradient';
  const gradient = defs.append('radialGradient')
    .attr('id', gradientId);
  gradient.append('stop').attr('offset', '0%').attr('stop-color', '#0a1929');
  gradient.append('stop').attr('offset', '100%').attr('stop-color', '#001e3c');
  
  canvas.append('rect')
    .attr('width', surfaceWidth.value)
    .attr('height', surfaceHeight.value)
    .attr('fill', `url(#${gradientId})`);
  
  const wavePoints = generateWaveform(centralState.rootCapsule);
  
  const edgeLayer = canvas.append('g').attr('class', 'edge-layer');
  const nodeLayer = canvas.append('g').attr('class', 'node-layer');
  
  wavePoints.forEach(point => {
    const coords = calculatePolarCoords(point.phase, point.depth);
    
    if (point.capsule.offspring && point.capsule.offspring.length > 0) {
      point.capsule.offspring.forEach(child => {
        const childPoint = wavePoints.find(wp => wp.capsule.keystone === child.keystone);
        if (childPoint) {
          const childCoords = calculatePolarCoords(childPoint.phase, childPoint.depth);
          
          const dx = childCoords.x - coords.x;
          const dy = childCoords.y - coords.y;
          const distance = Math.sqrt(dx * dx + dy * dy);
          
          const controlOffset = distance * 0.25;
          const perpX = -dy / distance;
          const perpY = dx / distance;
          
          const curve = d3.path();
          curve.moveTo(coords.x, coords.y);
          curve.bezierCurveTo(
            coords.x + dx * 0.33 + perpX * controlOffset,
            coords.y + dy * 0.33 + perpY * controlOffset,
            coords.x + dx * 0.67 - perpX * controlOffset,
            coords.y + dy * 0.67 - perpY * controlOffset,
            childCoords.x,
            childCoords.y
          );
          
          edgeLayer.append('path')
            .attr('d', curve.toString())
            .attr('stroke', `hsla(${195 + child.densityFactor * 90}, 75%, 60%, ${0.4 + child.densityFactor * 0.3})`)
            .attr('stroke-width', 2.2 + child.magnitude / 15000)
            .attr('fill', 'none')
            .attr('stroke-dasharray', `${5 + child.temporalCost / 50}, ${3 + child.temporalCost / 100}`)
            .style('animation', `dash-flow ${2 + Math.random()}s linear infinite`);
        }
      });
    }
  });
  
  wavePoints.forEach(point => {
    const coords = calculatePolarCoords(point.phase, point.depth);
    
    const nodeGroup = nodeLayer.append('g')
      .attr('transform', `translate(${coords.x}, ${coords.y})`)
      .style('cursor', 'pointer')
      .on('click', () => {
        activeKeystone = point.capsule.keystone;
        setInspectedCapsule(point.capsule);
        refreshActiveState();
      });
    
    const nodeRadius = 28 + point.capsule.densityFactor * 18;
    const hueValue = 260 - point.capsule.temporalCost * 0.4;
    const isActive = point.capsule.keystone === activeKeystone;
    
    nodeGroup.append('circle')
      .attr('r', nodeRadius)
      .attr('fill', `hsla(${hueValue}, 70%, 55%, 0.9)`)
      .attr('stroke', isActive ? '#ffeb3b' : '#29b6f6')
      .attr('stroke-width', isActive ? 5 : 2.5)
      .attr('class', 'capsule-orb')
      .attr('data-keystone', point.capsule.keystone)
      .style('filter', `drop-shadow(0 0 ${isActive ? 12 : 6}px ${isActive ? '#ffeb3b' : '#29b6f6'})`);
    
    const labelFragment = point.capsule.nomenclature.substring(0, 4).toUpperCase();
    
    nodeGroup.append('text')
      .text(labelFragment)
      .attr('text-anchor', 'middle')
      .attr('dy', '.35em')
      .attr('fill', '#ffffff')
      .attr('font-size', '11px')
      .attr('font-weight', '900')
      .style('pointer-events', 'none')
      .style('text-shadow', '0 2px 4px rgba(0,0,0,0.8)');
    
    nodeGroup.append('circle')
      .attr('r', nodeRadius + 5)
      .attr('fill', 'none')
      .attr('stroke', `hsla(${hueValue}, 70%, 55%, 0.3)`)
      .attr('stroke-width', 1)
      .style('animation', `pulse-ring ${1.5 + Math.random()}s ease-in-out infinite`);
  });
  
  const style = canvas.append('style');
  style.text(`
    @keyframes pulse-ring {
      0%, 100% { r: ${28}; opacity: 0.3; }
      50% { r: ${38}; opacity: 0.1; }
    }
    @keyframes dash-flow {
      to { stroke-dashoffset: -30; }
    }
  `);
  
  renderContext = { canvas, wavePoints };
};

const refreshActiveState = () => {
  if (!renderContext) return;
  
  d3.selectAll('.capsule-orb')
    .attr('stroke', function() {
      const keystone = d3.select(this).attr('data-keystone');
      return keystone === activeKeystone ? '#ffeb3b' : '#29b6f6';
    })
    .attr('stroke-width', function() {
      const keystone = d3.select(this).attr('data-keystone');
      return keystone === activeKeystone ? 5 : 2.5;
    })
    .style('filter', function() {
      const keystone = d3.select(this).attr('data-keystone');
      const isActive = keystone === activeKeystone;
      return `drop-shadow(0 0 ${isActive ? 12 : 6}px ${isActive ? '#ffeb3b' : '#29b6f6'})`;
    });
};

onMounted(() => {
  paintSurface();
});

watch(() => centralState.revision, () => {
  paintSurface();
});
</script>

<template>
  <div ref="viewport" class="waveform-surface"></div>
</template>

<style scoped>
.waveform-surface {
  width: 100%;
  height: 100%;
  overflow: hidden;
  border-radius: 22px;
  box-shadow: 0 16px 64px rgba(0, 0, 0, 0.8);
}
</style>
