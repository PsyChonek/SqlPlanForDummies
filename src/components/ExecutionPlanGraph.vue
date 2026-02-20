<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue';
import * as d3 from 'd3';
import { usePlanState } from '../composables/planState';
import type { RelOp } from '../types/sqlplan';
import { getCostSeverity, getCostColor, formatTime, formatRows } from '../types/sqlplan';

const props = withDefaults(defineProps<{ showHeader?: boolean }>(), { showHeader: true });

const {
  state,
  selectNode,
  selectEdge,
  getNodeCostPercentage,
  getNodeOwnCostPercentage,
  allNodes,
  navigateToParent,
  navigateToFirstChild,
  navigateToSibling,
  selectFirstNode
} = usePlanState();

const viewport = ref<HTMLDivElement>();
const surfaceWidth = ref(1200);
const surfaceHeight = ref(800);

// Store zoom behavior and SVG element for control buttons
let currentZoom: d3.ZoomBehavior<SVGSVGElement, unknown> | null = null;
let currentSvg: d3.Selection<SVGSVGElement, unknown, null, undefined> | null = null;
let currentContentGroup: d3.Selection<SVGGElement, unknown, null, undefined> | null = null;

// Zoom control functions
const zoomIn = () => {
  if (currentSvg && currentZoom) {
    currentSvg.transition().duration(300).call(currentZoom.scaleBy, 1.3);
  }
};

const zoomOut = () => {
  if (currentSvg && currentZoom) {
    currentSvg.transition().duration(300).call(currentZoom.scaleBy, 0.7);
  }
};

const zoomFit = () => {
  if (currentSvg && currentZoom && currentContentGroup && viewport.value) {
    const node = currentContentGroup.node();
    const bounds = node?.getBBox();
    if (bounds && bounds.width > 0 && bounds.height > 0) {
      const fullWidth = viewport.value.clientWidth;
      const fullHeight = viewport.value.clientHeight - 40; // Account for header
      const padding = 100; // Padding around the graph for better visibility
      
      // Calculate scale to fit entire graph with padding
      const scaleX = fullWidth / (bounds.width + padding);
      const scaleY = fullHeight / (bounds.height + padding);
      const scale = Math.min(scaleX, scaleY);

      // Center the graph in the viewport
      const translateX = (fullWidth - bounds.width * scale) / 2 - bounds.x * scale;
      const translateY = (fullHeight - bounds.height * scale) / 2 - bounds.y * scale;

      currentSvg.transition()
        .duration(500)
        .call(currentZoom.transform, d3.zoomIdentity.translate(translateX, translateY).scale(scale));
    }
  }
};

// D3 hierarchy node type
interface TreeNode {
  relOp: RelOp;
  costPercentage: number;
  ownCostPercentage: number;
  timePercentage: number;
}

// Own elapsed time = node's elapsed minus all direct children's elapsed (clamped to 0)
const getNodeOwnElapsedMs = (op: RelOp): number => {
  const total = op.runtimeInfo?.actualElapsedMs ?? 0;
  const childrenTotal = op.children.reduce((sum, c) => sum + (c.runtimeInfo?.actualElapsedMs ?? 0), 0);
  return Math.max(0, total - childrenTotal);
};

// Convert RelOp tree to D3 hierarchy format
const buildHierarchy = (relOp: RelOp, maxOwnElapsedMs: number): d3.HierarchyNode<TreeNode> => {
  const toTreeNode = (op: RelOp): TreeNode => ({
    relOp: op,
    costPercentage: getNodeCostPercentage(op),
    ownCostPercentage: getNodeOwnCostPercentage(op),
    timePercentage: maxOwnElapsedMs > 0
      ? (getNodeOwnElapsedMs(op) / maxOwnElapsedMs) * 100
      : 0,
  });

  const buildChildren = (op: RelOp): any => ({
    ...toTreeNode(op),
    children: op.children.map(buildChildren),
  });

  return d3.hierarchy(buildChildren(relOp));
};

// Node dimensions and spacing (configurable via .env)
const NODE_WIDTH = Number(import.meta.env.VITE_NODE_WIDTH) || 360;
const NODE_HEIGHT = Number(import.meta.env.VITE_NODE_HEIGHT) || 155;
const NODE_MARGIN_X = Number(import.meta.env.VITE_NODE_MARGIN_X) || 10;
const NODE_MARGIN_Y = Number(import.meta.env.VITE_NODE_MARGIN_Y) || 15;

// Font sizes (configurable via .env)
const FONT_SIZE_LABEL = Number(import.meta.env.VITE_FONT_SIZE_LABEL) || 14;
const FONT_SIZE_SUBTITLE = Number(import.meta.env.VITE_FONT_SIZE_SUBTITLE) || 11;
const FONT_SIZE_DETAIL = Number(import.meta.env.VITE_FONT_SIZE_DETAIL) || 11;

// Colors (configurable via .env)
const COLOR_NODE_BG = import.meta.env.VITE_COLOR_NODE_BG || '#1e293b';
const COLOR_NODE_SELECTED = import.meta.env.VITE_COLOR_NODE_SELECTED || '#3b82f6';
const COLOR_TEXT_PRIMARY = import.meta.env.VITE_COLOR_TEXT_PRIMARY || '#f1f5f9';
const COLOR_TEXT_SECONDARY = import.meta.env.VITE_COLOR_TEXT_SECONDARY || '#94a3b8';
const COLOR_TEXT_MUTED = import.meta.env.VITE_COLOR_TEXT_MUTED || '#64748b';
const COLOR_TEXT_SELECTED = import.meta.env.VITE_COLOR_TEXT_SELECTED || '#ffffff';
const COLOR_TEXT_SELECTED_DIM = import.meta.env.VITE_COLOR_TEXT_SELECTED_DIM || '#e2e8f0';
const COLOR_LINK = import.meta.env.VITE_COLOR_LINK || '#475569';

// Edge settings
const EDGE_MIN_THICKNESS = Number(import.meta.env.VITE_EDGE_MIN_THICKNESS) || 2;
const EDGE_MAX_THICKNESS = Number(import.meta.env.VITE_EDGE_MAX_THICKNESS) || 12;
const SHOW_EDGE_LABELS = import.meta.env.VITE_SHOW_EDGE_LABELS === 'true';

// Color mode: 'cost' (default) or 'time' (actual elapsed ms)
const COLOR_MODE = import.meta.env.VITE_COLOR_MODE || 'cost';

// Abbreviate verbose wait type names to fit in a node
const abbreviateWaitType = (waitType: string): string => {
  if (waitType.startsWith('LCK_M_')) return `Lock(${waitType.slice(6)})`;
  if (waitType.startsWith('PAGEIOLATCH_')) return `PAGEIO(${waitType.slice(12)})`;
  if (waitType.startsWith('LATCH_')) return `Latch(${waitType.slice(6)})`;
  const abbrevs: Record<string, string> = {
    SOS_SCHEDULER_YIELD: 'CPU Yield',
    WRITELOG: 'WriteLog',
    ASYNC_IO_COMPLETION: 'AsyncIO',
    IO_COMPLETION: 'IO',
    NETWORK_IO: 'Net IO',
    SLEEP: 'Sleep',
  };
  return abbrevs[waitType] ?? waitType;
};

// Returns true if any wait is a lock wait (LCK_M_*)
const hasLockWait = (relOp: RelOp): boolean =>
  relOp.runtimeInfo?.waitStats?.some(w => w.waitType.startsWith('LCK_M_')) ?? false;

// Get a short subtitle for a node based on its operation details
const getNodeSubtitle = (relOp: RelOp): string => {
  const details = relOp.operationDetails;

  // Index operations: show table.index
  if (details.indexScan) {
    const obj = details.indexScan.object;
    if (obj.index) return `${obj.table}.${obj.index}`;
    return obj.table;
  }

  // Sort: show columns
  if (details.sort) {
    const cols = details.sort.orderBy.map(o => o.column.column).join(', ');
    return cols ? `ORDER BY ${cols}` : '';
  }

  // Filter: show truncated predicate
  if (details.filter?.predicate) {
    return details.filter.predicate;
  }

  // Joins / other: show logical op if different from physical
  if (relOp.logicalOp !== relOp.physicalOp) {
    return relOp.logicalOp;
  }

  return '';
};

const renderGraph = () => {
  if (!viewport.value || !state.selectedStatement) return;

  // Clear previous SVG (only remove SVG elements, not Vue-managed DOM)
  d3.select(viewport.value).selectAll('svg').remove();

  const relOp = state.selectedStatement.queryPlan.relOp;
  const maxOwnElapsedMs = Math.max(0, ...allNodes.value.map(n => getNodeOwnElapsedMs(n)));
  const root = buildHierarchy(relOp, maxOwnElapsedMs);

  // Create SVG (full viewport, zoom handles the rest)
  const svg = d3.select(viewport.value)
    .append('svg')
    .attr('width', '100%')
    .attr('height', '100%');

  // Add zoom behavior
  const g = svg.append('g');

  const zoom = d3.zoom<SVGSVGElement, unknown>()
    .scaleExtent([0.01, Infinity])
    .on('zoom', (event) => {
      g.attr('transform', event.transform);
    });

  svg.call(zoom);

  // Store references for zoom controls
  currentSvg = svg;
  currentZoom = zoom;
  currentContentGroup = g;

  // Use nodeSize so spacing is consistent regardless of tree size
  const treeLayout = d3.tree<TreeNode>()
    .nodeSize([NODE_HEIGHT + NODE_MARGIN_Y, NODE_WIDTH + NODE_MARGIN_X])
    .separation((a, b) => (a.parent === b.parent ? 1 : 1.3));

  const treeData = treeLayout(root);

  // Draw links (edges) with SSMS-style routing
  const linkGroup = g.append('g').attr('class', 'links');
  const links = treeData.links();

  // Corner radius for smooth bends
  const CORNER_R = 12;

  // Compute max rows across all edges for thickness normalization
  const getEdgeRows = (target: d3.HierarchyPointNode<TreeNode>): number => {
    const op = target.data.relOp;
    return op.runtimeInfo?.actualRows ?? op.estimateRows;
  };
  const maxRows = Math.max(1, ...links.map(l => getEdgeRows(l.target)));

  // Compute stroke width for an edge based on row count
  const getEdgeThickness = (target: d3.HierarchyPointNode<TreeNode>): number => {
    const rows = getEdgeRows(target);
    if (maxRows <= 0) return EDGE_MIN_THICKNESS;
    return EDGE_MIN_THICKNESS + (rows / maxRows) * (EDGE_MAX_THICKNESS - EDGE_MIN_THICKNESS);
  };

  // Build a rounded H-V-H path from (sx,sy) to (tx,ty) via vertical at midX
  const buildRoundedHVH = (sx: number, sy: number, tx: number, ty: number, midX: number): string => {
    if (Math.abs(sy - ty) < 1) {
      // Straight horizontal line, no bends needed
      return `M${sx},${sy} H${tx}`;
    }
    const r = Math.min(CORNER_R, Math.abs(midX - sx), Math.abs(tx - midX), Math.abs(ty - sy) / 2);
    const vDir = ty > sy ? 1 : -1; // vertical direction

    return [
      `M${sx},${sy}`,
      `H${midX - r}`,                            // horizontal to first bend
      `Q${midX},${sy} ${midX},${sy + vDir * r}`, // rounded corner: turn into vertical
      `V${ty - vDir * r}`,                        // vertical segment
      `Q${midX},${ty} ${midX + r},${ty}`,         // rounded corner: turn into horizontal
      `H${tx}`,                                   // horizontal to target
    ].join(' ');
  };

  // Group links by source node
  const linksBySource = new Map<number, typeof links>();
  for (const link of links) {
    const sourceId = link.source.data.relOp.nodeId;
    if (!linksBySource.has(sourceId)) {
      linksBySource.set(sourceId, []);
    }
    linksBySource.get(sourceId)!.push(link);
  }

  // Edge path data structure
  interface EdgePath {
    d: string;
    thickness: number;
    source: RelOp;
    target: RelOp;
    rows: number;
    labelX: number;
    labelY: number;
  }

  const edgePaths: EdgePath[] = [];

  for (const [, groupLinks] of linksBySource) {
    const source = groupLinks[0].source;
    const sx = source.y + 50 + NODE_WIDTH / 2;   // right edge of parent
    const sy = source.x + 50;                     // center Y of parent
    const n = groupLinks.length;

    // Sort children by their Y position for consistent ordering
    const sorted = [...groupLinks].sort((a, b) => a.target.x - b.target.x);

    // Pre-compute thicknesses
    const thicknesses = sorted.map(l => getEdgeThickness(l.target));

    if (n === 1) {
      const link = sorted[0];
      const target = link.target;
      const tx = target.y + 50 - NODE_WIDTH / 2;
      const ty = target.x + 50;
      const midX = sx + NODE_MARGIN_X / 2;
      edgePaths.push({
        d: buildRoundedHVH(sx, sy, tx, ty, midX),
        thickness: thicknesses[0],
        source: source.data.relOp,
        target: target.data.relOp,
        rows: getEdgeRows(target),
        labelX: (midX + tx) / 2,
        labelY: ty - 6,
      });
    } else {
      // Calculate total width needed: sum of all thicknesses + gaps between them
      const edgeGap = 4; // minimum px gap between edges
      const totalThickness = thicknesses.reduce((a, b) => a + b, 0);
      const totalNeeded = totalThickness + (n - 1) * edgeGap;

      // Compute center positions for each edge's vertical segment in the gap
      // Pack them centered within the available gap
      const gapCenter = sx + NODE_MARGIN_X / 2;
      const midXPositions: number[] = [];
      let cursor = gapCenter - totalNeeded / 2 + thicknesses[0] / 2;
      for (let i = 0; i < n; i++) {
        midXPositions.push(cursor);
        if (i < n - 1) {
          cursor += thicknesses[i] / 2 + edgeGap + thicknesses[i + 1] / 2;
        }
      }

      // Similarly offset the exit points vertically from the parent center
      const syPositions: number[] = [];
      const totalYNeeded = totalThickness + (n - 1) * edgeGap;
      let yCursor = sy - totalYNeeded / 2 + thicknesses[0] / 2;
      for (let i = 0; i < n; i++) {
        // Clamp to within parent node height bounds
        const maxOffset = NODE_HEIGHT / 2 - 2;
        syPositions.push(Math.max(sy - maxOffset, Math.min(sy + maxOffset, yCursor)));
        if (i < n - 1) {
          yCursor += thicknesses[i] / 2 + edgeGap + thicknesses[i + 1] / 2;
        }
      }

      sorted.forEach((link, i) => {
        const target = link.target;
        const tx = target.y + 50 - NODE_WIDTH / 2;
        const ty = target.x + 50;
        edgePaths.push({
          d: buildRoundedHVH(sx, syPositions[i], tx, ty, midXPositions[i]),
          thickness: thicknesses[i],
          source: source.data.relOp,
          target: target.data.relOp,
          rows: getEdgeRows(target),
          labelX: (midXPositions[i] + tx) / 2,
          labelY: ty - 6,
        });
      });
    }
  }

  // Draw interactive edge paths
  const edgeElements = linkGroup.selectAll('.edge-path')
    .data(edgePaths)
    .join('path')
    .attr('class', 'edge-path')
    .attr('d', d => d.d)
    .attr('fill', 'none')
    .attr('stroke', COLOR_LINK)
    .attr('stroke-width', d => d.thickness)
    .attr('stroke-opacity', 0.6)
    .attr('stroke-linecap', 'round');

  // Tooltip container (created early so event handlers can reference it;
  // will be raised above nodes after they are drawn)
  const tooltipGroup = g.append('g')
    .attr('class', 'edge-tooltip')
    .attr('visibility', 'hidden')
    .style('pointer-events', 'none');
  tooltipGroup.append('rect')
    .attr('fill', COLOR_NODE_BG)
    .attr('stroke', COLOR_LINK)
    .attr('stroke-width', 1)
    .attr('rx', 6);

  // Invisible wider hit-area paths for easier hover/click
  const hitAreaGroup = g.append('g').attr('class', 'edge-hit-areas');
  hitAreaGroup.selectAll('path')
    .data(edgePaths)
    .join('path')
    .attr('d', d => d.d)
    .attr('fill', 'none')
    .attr('stroke', 'transparent')
    .attr('stroke-width', d => Math.max(14, d.thickness + 8))
    .style('cursor', 'pointer')
    .on('click', (event, d) => {
      event.stopPropagation();
      selectEdge({ source: d.source, target: d.target });
    })
    .on('mouseenter', function(event, d) {
      // Highlight the corresponding visible edge
      edgeElements.filter(e => e === d)
        .attr('stroke', '#93c5fd')
        .attr('stroke-opacity', 0.9);

      // Show tooltip
      const op = d.target;
      const runtime = op.runtimeInfo;
      const tooltipLines = [
        `${d.source.physicalOp} \u2192 ${d.target.physicalOp}`,
        `Rows: ${formatRows(d.rows)}`,
      ];
      if (runtime?.actualRowsRead != null) {
        tooltipLines.push(`Rows Read: ${formatRows(runtime.actualRowsRead)}`);
      }
      if (runtime?.actualLogicalReads != null) {
        tooltipLines.push(`Logical Reads: ${runtime.actualLogicalReads}`);
      }
      if (runtime?.actualPhysicalReads != null) {
        tooltipLines.push(`Physical Reads: ${runtime.actualPhysicalReads}`);
      }
      if (runtime?.actualExecutions != null) {
        tooltipLines.push(`Executions: ${runtime.actualExecutions}`);
      }

      // Position tooltip near cursor in SVG coordinates
      const [mx, my] = d3.pointer(event, g.node());
      tooltipGroup
        .attr('transform', `translate(${mx + 15}, ${my - 10})`)
        .attr('visibility', 'visible');

      const lineHeight = 18;
      const padding = 10;
      const maxTextWidth = Math.max(...tooltipLines.map(t => t.length)) * 7.5;

      tooltipGroup.select('rect')
        .attr('width', maxTextWidth + padding * 2)
        .attr('height', tooltipLines.length * lineHeight + padding * 2)
        .attr('rx', 6);

      tooltipGroup.selectAll('text').remove();
      tooltipLines.forEach((line, i) => {
        tooltipGroup.append('text')
          .attr('x', padding)
          .attr('y', padding + 13 + i * lineHeight)
          .attr('fill', COLOR_TEXT_SECONDARY)
          .attr('font-size', '12px')
          .text(line);
      });
    })
    .on('mouseleave', function(_event, d) {
      // Restore edge style (check if it's the selected edge)
      const isSelected = state.selectedEdge &&
        state.selectedEdge.source.nodeId === d.source.nodeId &&
        state.selectedEdge.target.nodeId === d.target.nodeId;
      edgeElements.filter(e => e === d)
        .attr('stroke', isSelected ? '#93c5fd' : COLOR_LINK)
        .attr('stroke-opacity', isSelected ? 0.9 : 0.6);

      tooltipGroup.attr('visibility', 'hidden');
    });

  // Optional inline edge labels (drawn before nodes so labels don't cover nodes)
  if (SHOW_EDGE_LABELS) {
    const labelGroup = g.append('g').attr('class', 'edge-labels');
    labelGroup.selectAll('text')
      .data(edgePaths)
      .join('text')
      .attr('x', d => d.labelX)
      .attr('y', d => d.labelY)
      .attr('text-anchor', 'middle')
      .attr('font-size', `${FONT_SIZE_DETAIL}px`)
      .attr('fill', COLOR_TEXT_MUTED)
      .text(d => formatRows(d.rows));
  }

  // Draw nodes
  const nodeGroup = g.append('g').attr('class', 'nodes');

  const nodes = nodeGroup.selectAll('g')
    .data(treeData.descendants())
    .join('g')
    .attr('transform', d => `translate(${d.y + 50}, ${d.x + 50})`)
    .style('cursor', 'pointer')
    .on('click', (event, d) => {
      event.stopPropagation();
      selectNode(d.data.relOp);
    });

  // Node background
  nodes.append('rect')
    .attr('x', -NODE_WIDTH / 2)
    .attr('y', -NODE_HEIGHT / 2)
    .attr('width', NODE_WIDTH)
    .attr('height', NODE_HEIGHT)
    .attr('rx', 8)
    .attr('ry', 8)
    .attr('fill', d => {
      return d.data.relOp.nodeId === state.selectedNode?.nodeId
        ? COLOR_NODE_SELECTED
        : COLOR_NODE_BG;
    })
    .attr('stroke', d => {
      const pct = COLOR_MODE === 'time' ? d.data.timePercentage : d.data.costPercentage;
      return getCostColor(getCostSeverity(pct));
    })
    .attr('stroke-width', d => d.data.relOp.nodeId === state.selectedNode?.nodeId ? 3 : 2);

  // Cost indicator bar (at bottom of node)
  nodes.append('rect')
    .attr('x', -NODE_WIDTH / 2)
    .attr('y', NODE_HEIGHT / 2 - 4)
    .attr('width', d => {
      const pct = COLOR_MODE === 'time' ? d.data.timePercentage : d.data.costPercentage;
      return Math.max(4, (pct / 100) * NODE_WIDTH);
    })
    .attr('height', 4)
    .attr('fill', d => {
      const pct = COLOR_MODE === 'time' ? d.data.timePercentage : d.data.costPercentage;
      return getCostColor(getCostSeverity(pct));
    })
    .attr('rx', 2);

  // Operator name
  nodes.append('text')
    .attr('class', 'node-label')
    .attr('x', -NODE_WIDTH / 2 + 15)
    .attr('y', -36)
    .attr('font-size', `${FONT_SIZE_LABEL}px`)
    .attr('font-weight', 600)
    .attr('fill', COLOR_TEXT_PRIMARY)
    .text(d => truncateText(d.data.relOp.physicalOp, Math.floor((NODE_WIDTH - 30) / 9)));

  // Subtitle — up to two wrapped lines (table.index, join type, etc.)
  // Always occupies two line slots so rows below stay at fixed y positions.
  const SUBTITLE_X = -NODE_WIDTH / 2 + 15;
  // ~8px per char at 11px font (conservative for mixed-case SQL names); scales with node width
  const SUBTITLE_CHARS = Math.floor((NODE_WIDTH - 30) / 8);
  const SUBTITLE_LINE_H = 14; // px between baseline of line 1 and line 2
  nodes.append('text')
    .attr('class', 'node-subtitle')
    .attr('x', SUBTITLE_X)
    .attr('y', -18)
    .attr('font-size', `${FONT_SIZE_SUBTITLE}px`)
    .attr('fill', COLOR_TEXT_SECONDARY)
    .each(function(d) {
      const subtitle = getNodeSubtitle(d.data.relOp);
      const el = d3.select(this);
      if (subtitle.length <= SUBTITLE_CHARS) {
        // Single line — render as plain text, second slot stays empty
        el.text(subtitle);
      } else {
        // Find a clean break point near the char limit (prefer '.', '_', '-', ' ')
        // Search up to 20 chars back so we find a separator in CamelCase-heavy names
        let splitAt = SUBTITLE_CHARS;
        for (let i = SUBTITLE_CHARS; i > SUBTITLE_CHARS - 20; i--) {
          if ('._ -'.includes(subtitle[i] ?? '')) { splitAt = i + 1; break; }
        }
        el.append('tspan')
          .attr('x', SUBTITLE_X).attr('dy', 0)
          .text(truncateText(subtitle.substring(0, splitAt), SUBTITLE_CHARS));
        el.append('tspan')
          .attr('x', SUBTITLE_X).attr('dy', SUBTITLE_LINE_H)
          .text(truncateText(subtitle.substring(splitAt), SUBTITLE_CHARS));
      }
    });

  // Own cost % (CPU+IO relative to whole plan)
  nodes.append('text')
    .attr('class', 'node-cost')
    .attr('x', -NODE_WIDTH / 2 + 15)
    .attr('y', 14)
    .attr('font-size', `${FONT_SIZE_DETAIL}px`)
    .attr('fill', COLOR_TEXT_SECONDARY)
    .text(d => `Cost: ${d.data.ownCostPercentage.toFixed(1)}%`);

  // Subtree cost % (accumulated)
  nodes.append('text')
    .attr('class', 'node-subtree-cost')
    .attr('x', -NODE_WIDTH / 2 + 125)
    .attr('y', 14)
    .attr('font-size', `${FONT_SIZE_DETAIL}px`)
    .attr('fill', COLOR_TEXT_MUTED)
    .text(d => `Sub: ${d.data.costPercentage.toFixed(1)}%`);

  // Own time (node only, excluding children)
  nodes.append('text')
    .attr('class', 'node-time')
    .attr('x', -NODE_WIDTH / 2 + 15)
    .attr('y', 28)
    .attr('font-size', `${FONT_SIZE_DETAIL}px`)
    .attr('fill', COLOR_TEXT_SECONDARY)
    .text(d => {
      const own = getNodeOwnElapsedMs(d.data.relOp);
      return d.data.relOp.runtimeInfo ? `Time: ${formatTime(own)}` : '';
    });

  // Subtree total time
  nodes.append('text')
    .attr('class', 'node-time-sub')
    .attr('x', -NODE_WIDTH / 2 + 125)
    .attr('y', 28)
    .attr('font-size', `${FONT_SIZE_DETAIL}px`)
    .attr('fill', COLOR_TEXT_MUTED)
    .text(d => {
      const sub = d.data.relOp.runtimeInfo?.actualElapsedMs;
      return sub !== undefined ? `Sub: ${formatTime(sub)}` : '';
    });

  // Rows
  nodes.append('text')
    .attr('class', 'node-rows')
    .attr('x', -NODE_WIDTH / 2 + 15)
    .attr('y', 42)
    .attr('font-size', `${FONT_SIZE_DETAIL}px`)
    .attr('fill', COLOR_TEXT_MUTED)
    .text(d => `Rows: ${formatRows(d.data.relOp.estimateRows)}`);

  // IO: logical reads (left)
  nodes.append('text')
    .attr('class', 'node-io')
    .attr('x', -NODE_WIDTH / 2 + 15)
    .attr('y', 56)
    .attr('font-size', `${FONT_SIZE_DETAIL}px`)
    .attr('fill', COLOR_TEXT_MUTED)
    .text(d => {
      const rt = d.data.relOp.runtimeInfo;
      if (!rt) return '';
      const lr = rt.actualLogicalReads ?? 0;
      const pr = rt.actualPhysicalReads ?? 0;
      if (lr === 0 && pr === 0) return '';
      const parts = [`LR: ${formatRows(lr)}`];
      if (pr > 0) parts.push(`PR: ${formatRows(pr)}`);
      return parts.join('  ');
    });

  // Top wait (right of IO row) — amber for regular waits, red for lock waits
  nodes.append('text')
    .attr('class', 'node-wait')
    .attr('x', -NODE_WIDTH / 2 + 215)
    .attr('y', 56)
    .attr('font-size', `${FONT_SIZE_DETAIL}px`)
    .attr('fill', d => {
      const topWait = d.data.relOp.runtimeInfo?.waitStats?.[0];
      if (!topWait) return COLOR_TEXT_MUTED;
      return topWait.waitType.startsWith('LCK_M_') ? '#ef4444' : '#f59e0b';
    })
    .text(d => {
      const topWait = d.data.relOp.runtimeInfo?.waitStats?.[0];
      if (!topWait) return '';
      return abbreviateWaitType(topWait.waitType);
    });

  // Lock warning badge (top-right corner) — red circle with "!" when lock waits exist
  const lockBadgeG = nodes.append('g')
    .attr('class', 'node-lock-badge')
    .attr('transform', `translate(${NODE_WIDTH / 2 - 14}, ${-NODE_HEIGHT / 2 + 14})`)
    .attr('visibility', d => hasLockWait(d.data.relOp) ? 'visible' : 'hidden');

  lockBadgeG.append('circle')
    .attr('r', 9)
    .attr('fill', '#ef4444')
    .attr('stroke', '#1e293b')
    .attr('stroke-width', 1.5);

  lockBadgeG.append('text')
    .attr('text-anchor', 'middle')
    .attr('dominant-baseline', 'central')
    .attr('font-size', '10px')
    .attr('font-weight', 700)
    .attr('fill', '#ffffff')
    .text('🔒');

  // Raise tooltip and hit-areas above nodes so they render on top
  hitAreaGroup.raise();
  tooltipGroup.raise();

  // Clear selection when clicking on background
  svg.on('click', () => {
    selectNode(null);
  });

  // Initial zoom to fit - use nextTick to ensure DOM is fully rendered
  nextTick(() => {
    const bounds = g.node()?.getBBox();
    if (bounds && bounds.width > 0 && bounds.height > 0 && viewport.value) {
      const fullWidth = viewport.value.clientWidth;
      const fullHeight = viewport.value.clientHeight;
      const padding = 100;

      const scaleX = fullWidth / (bounds.width + padding);
      const scaleY = fullHeight / (bounds.height + padding);
      const scale = Math.min(scaleX, scaleY);

      const translateX = (fullWidth - bounds.width * scale) / 2 - bounds.x * scale;
      const translateY = (fullHeight - bounds.height * scale) / 2 - bounds.y * scale;

      svg.call(zoom.transform, d3.zoomIdentity.translate(translateX, translateY).scale(scale));
    }
  });
};

// Helper to truncate text
const truncateText = (text: string, maxLength: number): string => {
  if (text.length <= maxLength) return text;
  return text.substring(0, maxLength - 2) + '..';
};

// Watch for changes — re-render on revision change (statement switch, plan load)
// renderGraph() already does zoom-to-fit internally via nextTick
watch(() => state.revision, () => {
  renderGraph();
});

watch(() => state.selectedNode, () => {
  // Update node styling without re-rendering (preserves zoom/pan position)
  if (!currentContentGroup) return;
  const isSelected = (d: any) => d.data.relOp.nodeId === state.selectedNode?.nodeId;

  // Background rect
  currentContentGroup.selectAll('.nodes g rect:first-child')
    .attr('fill', (d: any) => isSelected(d) ? COLOR_NODE_SELECTED : COLOR_NODE_BG)
    .attr('stroke-width', (d: any) => isSelected(d) ? 3 : 2);

  // Operator name
  currentContentGroup.selectAll('.nodes g .node-label')
    .attr('fill', (d: any) => isSelected(d) ? COLOR_TEXT_SELECTED : COLOR_TEXT_PRIMARY);

  // Subtitle, cost, time
  currentContentGroup.selectAll('.nodes g .node-subtitle, .nodes g .node-cost, .nodes g .node-time')
    .attr('fill', (d: any) => isSelected(d) ? COLOR_TEXT_SELECTED_DIM : COLOR_TEXT_SECONDARY);

  // Subtree cost (dimmer when not selected)
  currentContentGroup.selectAll('.nodes g .node-subtree-cost')
    .attr('fill', (d: any) => isSelected(d) ? COLOR_TEXT_SELECTED_DIM : COLOR_TEXT_MUTED);

  // Rows, IO
  currentContentGroup.selectAll('.nodes g .node-rows, .nodes g .node-io')
    .attr('fill', (d: any) => isSelected(d) ? COLOR_TEXT_SELECTED_DIM : COLOR_TEXT_MUTED);

  // Wait label — keep severity color (red/amber) even when selected
  currentContentGroup.selectAll('.nodes g .node-wait')
    .attr('fill', (d: any) => {
      const topWait = d.data.relOp.runtimeInfo?.waitStats?.[0];
      if (!topWait) return isSelected(d) ? COLOR_TEXT_SELECTED_DIM : COLOR_TEXT_MUTED;
      return topWait.waitType.startsWith('LCK_M_') ? '#ef4444' : '#f59e0b';
    });
});

watch(() => state.selectedEdge, () => {
  // Update edge highlighting without re-rendering
  if (!currentContentGroup) return;
  currentContentGroup.selectAll('.edge-path')
    .attr('stroke', (d: any) => {
      const isEdgeSelected = state.selectedEdge &&
        state.selectedEdge.source.nodeId === d.source.nodeId &&
        state.selectedEdge.target.nodeId === d.target.nodeId;
      return isEdgeSelected ? '#93c5fd' : COLOR_LINK;
    })
    .attr('stroke-opacity', (d: any) => {
      const isEdgeSelected = state.selectedEdge &&
        state.selectedEdge.source.nodeId === d.source.nodeId &&
        state.selectedEdge.target.nodeId === d.target.nodeId;
      return isEdgeSelected ? 0.9 : 0.6;
    });
});

onMounted(() => {
  // Get container dimensions
  if (viewport.value) {
    surfaceWidth.value = viewport.value.clientWidth;
    surfaceHeight.value = viewport.value.clientHeight;
  }
  renderGraph();
});

// Resize observer
onMounted(() => {
  if (!viewport.value) return;
  
  const resizeObserver = new ResizeObserver(() => {
    if (viewport.value) {
      surfaceWidth.value = viewport.value.clientWidth;
      surfaceHeight.value = viewport.value.clientHeight;
      renderGraph();
    }
  });
  
  resizeObserver.observe(viewport.value);
});

// Keyboard navigation
const handleKeyDown = (event: KeyboardEvent) => {
  if (!state.selectedStatement) return;
  
  // If no node selected, select the root node on first key press
  if (!state.selectedNode) {
    if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Enter', ' '].includes(event.key)) {
      selectFirstNode();
      event.preventDefault();
      return;
    }
  }
  
  switch (event.key) {
    case 'ArrowLeft':
      // Go to parent (left in tree layout means parent)
      navigateToParent();
      event.preventDefault();
      break;
    case 'ArrowRight':
      // Go to first child
      navigateToFirstChild();
      event.preventDefault();
      break;
    case 'ArrowUp':
      // Go to previous sibling
      navigateToSibling('prev');
      event.preventDefault();
      break;
    case 'ArrowDown':
      // Go to next sibling
      navigateToSibling('next');
      event.preventDefault();
      break;
    case 'Escape':
      // Deselect node
      selectNode(null);
      event.preventDefault();
      break;
    case '+':
    case '=':
      zoomIn();
      event.preventDefault();
      break;
    case '-':
      zoomOut();
      event.preventDefault();
      break;
    case '0':
      zoomFit();
      event.preventDefault();
      break;
  }
};

onMounted(() => {
  window.addEventListener('keydown', handleKeyDown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown);
});
</script>

<template>
  <div class="relative w-full h-full bg-slate-800 rounded-2xl shadow-xl">
    <!-- Header bar -->
    <div v-if="props.showHeader" class="absolute top-0 left-0 right-0 z-10 flex items-center justify-between px-4 py-3 bg-slate-700 border-b border-slate-600 rounded-t-2xl">
      <div class="flex items-center gap-2">
        <i class="fa-solid fa-diagram-project text-blue-400"></i>
        <span class="text-sm font-semibold text-slate-200">Execution Plan</span>
      </div>
      <div v-if="state.selectedStatement" class="text-xs text-slate-400">
        <span>Total Cost: {{ state.selectedStatement.statementSubTreeCost.toFixed(6) }}</span>
      </div>
    </div>

    <!-- Graph container -->
    <div
      ref="viewport"
      class="w-full h-full overflow-hidden"
      :class="[props.showHeader ? 'pt-10' : '', { 'flex items-center justify-center': !state.selectedStatement }]"
    >
      <!-- Empty state -->
      <div v-if="!state.selectedStatement" class="text-center text-slate-500">
        <i class="fa-solid fa-file-circle-question text-4xl mb-3"></i>
        <p class="text-sm">Load a .sqlplan file to visualize the execution plan</p>
      </div>
    </div>
    
    <!-- Zoom controls -->
    <div v-if="state.selectedStatement" class="absolute bottom-6 right-6 z-50 flex flex-col gap-2">
      <button 
        class="w-10 h-10 bg-slate-700 hover:bg-slate-600 rounded-lg flex items-center justify-center text-slate-300 transition-colors shadow-lg border border-slate-600"
        title="Zoom In (+)"
        @click="zoomIn"
      >
        <i class="fa-solid fa-plus text-sm"></i>
      </button>
      <button 
        class="w-10 h-10 bg-slate-700 hover:bg-slate-600 rounded-lg flex items-center justify-center text-slate-300 transition-colors shadow-lg border border-slate-600"
        title="Zoom Out (-)"
        @click="zoomOut"
      >
        <i class="fa-solid fa-minus text-sm"></i>
      </button>
      <button 
        class="w-10 h-10 bg-slate-700 hover:bg-slate-600 rounded-lg flex items-center justify-center text-slate-300 transition-colors shadow-lg border border-slate-600"
        title="Fit to View (0)"
        @click="zoomFit"
      >
        <i class="fa-solid fa-expand text-sm"></i>
      </button>
    </div>
  </div>
</template>
