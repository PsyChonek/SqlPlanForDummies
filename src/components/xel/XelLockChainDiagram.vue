<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, computed } from 'vue';
import * as d3 from 'd3';
import { useXelState } from '../../composables/useXelState';
import * as xelApi from '../../composables/xelTauriApi';
import type { XelEvent, BlockingAnalysis } from '../../types/xel';

const { state, selectSession } = useXelState();

const viewport = ref<HTMLDivElement>();
let resizeObserver: ResizeObserver | null = null;
const loading = ref(false);
const timeWindow = ref(30);

const hasSelection = computed(() => state.selectedEvent !== null);

interface LockNode {
  id: string;
  type: 'session' | 'resource' | 'transaction' | 'deadlock';
  label: string;
  sessionId?: number;
  eventCount: number;
  isBlocker: boolean;
  isVictim: boolean;
  isSelected: boolean;
  // Metadata for richer display
  username?: string;
  appName?: string;
  database?: string;
  sqlPreview?: string;
  hostname?: string;
  objectName?: string;
  resourceType?: string;
  waitTimeMs?: number;
}

const tooltip = ref<{ x: number; y: number; node: LockNode } | null>(null);

interface LockEdge {
  source: string;
  target: string;
  mode: string;
  relationship: 'holds' | 'waits' | 'transaction' | 'deadlock_victim' | 'deadlock_holder';
  count: number;
}

const buildSessionLabel = (sid: number, username?: string | null, appName?: string | null): string => {
  const parts: string[] = [];
  if (username) parts.push(username.replace(/^.*\\/, '')); // strip domain prefix
  if (appName) {
    // Shorten known app names
    let short = appName;
    if (short.length > 20) short = short.substring(0, 18) + '…';
    parts.push(short);
  }
  if (parts.length > 0) return `S${sid} · ${parts.join(' / ')}`;
  return `Session ${sid}`;
};

const buildResourceLabel = (
  rawLabel: string,
  objectNameLookup: Map<string, { name: string; db?: string; resType?: string }>,
  database?: string | null,
): string => {
  // Try to extract associated_object_id from raw labels like "OBJECT [470225792]" or "KEY: 7:470225792:1"
  const bracketMatch = rawLabel.match(/\[(\d+)\]/);
  const colonMatch = rawLabel.match(/(\d+):(\d+):(\d+)/);

  // Try all number-like IDs found in the label against the lookup
  const candidates: string[] = [];
  if (bracketMatch) candidates.push(bracketMatch[1]);
  if (colonMatch) candidates.push(colonMatch[1], colonMatch[2], colonMatch[3]);

  for (const objId of candidates) {
    if (objectNameLookup.has(objId)) {
      const info = objectNameLookup.get(objId)!;
      const prefix = info.db ? `${info.db}.` : '';
      return `${prefix}${info.name}`;
    }
  }

  // No resolved name — show db + resource type instead of raw IDs
  const resType = rawLabel.split(/[:\s\[]/)[0]?.trim() || 'Resource';
  if (database) return `${database} · ${resType}`;

  // Fallback: clean up the raw label
  if (colonMatch) return `${resType} [${colonMatch[0]}]`;
  return rawLabel;
};

const buildGraph = (events: XelEvent[], anchorEvent: XelEvent | null, analysis: BlockingAnalysis | null) => {
  const nodeMap = new Map<string, LockNode>();
  const edgeMap = new Map<string, LockEdge>();

  const anchorSid = anchorEvent?.sessionId;
  const anchorTxId = anchorEvent?.extraFields?.['transaction_id'];

  // Track which sessions have ACTUAL lock relationships
  const sessionsWithLockData = new Set<number>();

  // Build a map from associated_object_id to human-readable names from events
  const objectNameLookup = new Map<string, { name: string; db?: string; resType?: string }>();
  for (const event of events) {
    const assocObj = event.extraFields?.['associated_object_id'];
    if (!assocObj) continue;
    const key = String(assocObj);
    if (objectNameLookup.has(key)) continue;
    const resolvedObj = event.extraFields?.['resolved_object'] as string | undefined;
    const name = event.objectName || resolvedObj;
    if (name) {
      objectNameLookup.set(key, {
        name,
        db: event.databaseName || undefined,
        resType: event.resourceType || undefined,
      });
    }
  }

  // 1. Build nodes/edges from blocking analysis (most reliable source)
  if (analysis) {
    // From blocking chain
    for (const link of analysis.blockingChain) {
      const sessionKey = `s_${link.sessionId}`;
      sessionsWithLockData.add(link.sessionId);
      const sessionLabel = buildSessionLabel(link.sessionId, link.username, link.appName);
      nodeMap.set(sessionKey, {
        id: sessionKey,
        type: 'session',
        label: sessionLabel,
        sessionId: link.sessionId,
        eventCount: link.eventIds.length,
        isBlocker: link.role === 'root_blocker',
        isVictim: link.role === 'victim',
        isSelected: link.sessionId === anchorSid,
        username: link.username || undefined,
        appName: link.appName || undefined,
        database: link.database || undefined,
        sqlPreview: link.sqlPreview || undefined,
      });

      if (link.waitResource) {
        const resKey = `r_${link.waitResource}`;
        if (!nodeMap.has(resKey)) {
          const resLabel = buildResourceLabel(link.waitResource, objectNameLookup, link.database);
          nodeMap.set(resKey, {
            id: resKey, type: 'resource',
            label: resLabel.length > 30 ? resLabel.substring(0, 30) + '…' : resLabel,
            eventCount: 0, isBlocker: false, isVictim: false, isSelected: false,
            resourceType: link.waitResource.split(/[:\s]/)[0] || undefined,
          });
        }

        if (link.role === 'victim' || link.role === 'intermediate') {
          const ek = `${sessionKey}-${resKey}-waits`;
          edgeMap.set(ek, { source: sessionKey, target: resKey, mode: link.lockMode || '', relationship: 'waits', count: 1 });
        }
        if (link.blockedBySession) {
          const blockerKey = `s_${link.blockedBySession}`;
          const ek = `${blockerKey}-${resKey}-holds`;
          if (!edgeMap.has(ek)) {
            edgeMap.set(ek, { source: blockerKey, target: resKey, mode: '', relationship: 'holds', count: 1 });
          }
        }
      }
    }

    // From BPRs (may have data not in the chain)
    for (const bpr of analysis.blockedProcessReports) {
      if (bpr.blockedSpid && bpr.blockingSpid) {
        sessionsWithLockData.add(bpr.blockedSpid);
        sessionsWithLockData.add(bpr.blockingSpid);

        const victimKey = `s_${bpr.blockedSpid}`;
        const blockerKey = `s_${bpr.blockingSpid}`;

        if (!nodeMap.has(victimKey)) {
          nodeMap.set(victimKey, {
            id: victimKey, type: 'session',
            label: buildSessionLabel(bpr.blockedSpid, bpr.blockedLoginName, bpr.blockedAppName),
            sessionId: bpr.blockedSpid, eventCount: 0,
            isBlocker: false, isVictim: true, isSelected: bpr.blockedSpid === anchorSid,
            username: bpr.blockedLoginName || undefined,
            appName: bpr.blockedAppName || undefined,
            database: bpr.blockedDatabase || undefined,
            sqlPreview: bpr.blockedInputBuffer || undefined,
            hostname: bpr.blockedHostname || undefined,
            waitTimeMs: bpr.blockedWaitTimeMs || undefined,
          });
        }
        if (!nodeMap.has(blockerKey)) {
          nodeMap.set(blockerKey, {
            id: blockerKey, type: 'session',
            label: buildSessionLabel(bpr.blockingSpid, bpr.blockingLoginName, bpr.blockingAppName),
            sessionId: bpr.blockingSpid, eventCount: 0,
            isBlocker: true, isVictim: false, isSelected: bpr.blockingSpid === anchorSid,
            username: bpr.blockingLoginName || undefined,
            appName: bpr.blockingAppName || undefined,
            database: bpr.blockingDatabase || undefined,
            sqlPreview: bpr.blockingInputBuffer || undefined,
            hostname: bpr.blockingHostname || undefined,
          });
        }

        if (bpr.blockedWaitResource) {
          const resKey = `r_${bpr.blockedWaitResource}`;
          if (!nodeMap.has(resKey)) {
            const resLabel = buildResourceLabel(bpr.blockedWaitResource, objectNameLookup, bpr.blockedDatabase);
            nodeMap.set(resKey, {
              id: resKey, type: 'resource',
              label: resLabel.length > 30 ? resLabel.substring(0, 30) + '…' : resLabel,
              eventCount: 0, isBlocker: false, isVictim: false, isSelected: false,
              resourceType: bpr.blockedWaitResource.split(/[:\s]/)[0] || undefined,
            });
          }
          edgeMap.set(`${victimKey}-${resKey}-waits`, { source: victimKey, target: resKey, mode: bpr.blockedLockMode || '', relationship: 'waits', count: 1 });
          edgeMap.set(`${blockerKey}-${resKey}-holds`, { source: blockerKey, target: resKey, mode: '', relationship: 'holds', count: 1 });
        }
      }
    }

    // From deadlocks
    for (const dl of analysis.deadlocks) {
      const dlKey = `dl_${dl.eventId}`;
      nodeMap.set(dlKey, {
        id: dlKey, type: 'deadlock', label: 'DEADLOCK',
        eventCount: dl.processes.length, isBlocker: false, isVictim: false, isSelected: false,
      });

      for (const proc of dl.processes) {
        if (proc.spid) {
          sessionsWithLockData.add(proc.spid);
          const sessionKey = `s_${proc.spid}`;
          if (!nodeMap.has(sessionKey)) {
            nodeMap.set(sessionKey, {
              id: sessionKey, type: 'session',
              label: buildSessionLabel(proc.spid, proc.loginName, proc.appName),
              sessionId: proc.spid, eventCount: 0,
              isBlocker: !proc.isVictim, isVictim: proc.isVictim,
              isSelected: proc.spid === anchorSid,
              username: proc.loginName || undefined,
              appName: proc.appName || undefined,
              database: proc.databaseName || undefined,
              sqlPreview: proc.inputBuffer || undefined,
              hostname: proc.hostname || undefined,
            });
          } else {
            const node = nodeMap.get(sessionKey)!;
            if (proc.isVictim) node.isVictim = true;
            else node.isBlocker = true;
          }

          const rel = proc.isVictim ? 'deadlock_victim' : 'deadlock_holder';
          edgeMap.set(`${sessionKey}-${dlKey}-${rel}`, {
            source: sessionKey, target: dlKey, mode: proc.lockMode || '',
            relationship: rel, count: 1,
          });
        }
      }

      // Deadlock resources
      for (const res of dl.resources) {
        const resNameParts = [res.databaseName, res.objectName, res.indexName].filter(Boolean);
        const resLabel = resNameParts.join('.') || res.resourceType;
        const resKey = `dlr_${dl.eventId}_${resLabel}`;
        nodeMap.set(resKey, {
          id: resKey, type: 'resource',
          label: resLabel.length > 30 ? resLabel.substring(0, 30) + '…' : resLabel,
          eventCount: 0, isBlocker: false, isVictim: false, isSelected: false,
          objectName: res.objectName || undefined,
          database: res.databaseName || undefined,
          resourceType: res.resourceType || undefined,
        });
        for (const h of res.holders) {
          edgeMap.set(`${h.processId}-${resKey}-holds`, { source: h.processId, target: resKey, mode: h.mode || res.mode || '', relationship: 'holds', count: 1 });
        }
        for (const w of res.waiters) {
          edgeMap.set(`${w.processId}-${resKey}-waits`, { source: w.processId, target: resKey, mode: w.mode || '', relationship: 'waits', count: 1 });
        }
      }
    }
  }

  // 2. Add lock events from XEL data (only if they have actual lock relationships)
  for (const event of events) {
    const sid = event.sessionId;
    if (sid === null || sid === undefined) continue;

    const resType = event.resourceType || (event.extraFields?.['resource_type'] as string);
    const lockMode = event.lockMode || (event.extraFields?.['mode'] as string);
    const assocObj = event.extraFields?.['associated_object_id'];

    if (resType && lockMode && assocObj) {
      sessionsWithLockData.add(sid);
      const sessionKey = `s_${sid}`;
      if (!nodeMap.has(sessionKey)) {
        nodeMap.set(sessionKey, {
          id: sessionKey, type: 'session',
          label: buildSessionLabel(sid, event.username, event.clientAppName),
          sessionId: sid, eventCount: 0, isBlocker: false, isVictim: false,
          isSelected: sid === anchorSid,
          username: event.username || undefined,
          appName: event.clientAppName || undefined,
          database: event.databaseName || undefined,
        });
      } else {
        // Enrich existing node with metadata if missing
        const existing = nodeMap.get(sessionKey)!;
        if (!existing.username && event.username) {
          existing.username = event.username;
          existing.appName = existing.appName || event.clientAppName || undefined;
          existing.database = existing.database || event.databaseName || undefined;
          existing.label = buildSessionLabel(sid, existing.username, existing.appName);
        }
      }
      nodeMap.get(sessionKey)!.eventCount++;

      const resKey = `r_${assocObj}`;
      if (!nodeMap.has(resKey)) {
        // Try to get a human-readable name
        const lookup = objectNameLookup.get(String(assocObj));
        const resolvedObj = event.extraFields?.['resolved_object'] as string | undefined;
        let resLabel: string;
        let objName: string | undefined;
        if (lookup) {
          objName = lookup.name;
          resLabel = lookup.db ? `${lookup.db}.${lookup.name}` : lookup.name;
        } else if (resolvedObj) {
          objName = resolvedObj;
          resLabel = event.databaseName ? `${event.databaseName}.${resolvedObj}` : resolvedObj;
        } else if (event.objectName) {
          objName = event.objectName;
          resLabel = event.databaseName ? `${event.databaseName}.${event.objectName}` : event.objectName;
        } else {
          // No resolved name — show db + type instead of raw ID
          const db = event.databaseName;
          resLabel = db ? `${db} · ${resType}` : `${resType} [${assocObj}]`;
        }
        nodeMap.set(resKey, {
          id: resKey, type: 'resource',
          label: resLabel.length > 30 ? resLabel.substring(0, 30) + '…' : resLabel,
          eventCount: 0, isBlocker: false, isVictim: false, isSelected: false,
          objectName: objName || undefined,
          database: event.databaseName || undefined,
          resourceType: resType,
        });
      }
      nodeMap.get(resKey)!.eventCount++;

      const isWait = event.eventName.includes('timeout') || event.eventName.includes('blocked') || event.eventName.includes('wait');
      const rel = isWait ? 'waits' : 'holds';
      const node = nodeMap.get(sessionKey)!;
      if (isWait) node.isVictim = true;
      else node.isBlocker = true;

      const edgeKey = `${sessionKey}-${resKey}-${rel}`;
      if (!edgeMap.has(edgeKey)) {
        edgeMap.set(edgeKey, { source: sessionKey, target: resKey, mode: lockMode as string, relationship: rel, count: 0 });
      }
      edgeMap.get(edgeKey)!.count++;
    }

    // Transaction edges — only if session has lock data
    if (sessionsWithLockData.has(sid)) {
      const txId = event.extraFields?.['transaction_id'];
      if (txId && txId !== 0 && txId !== '0') {
        const sessionKey = `s_${sid}`;
        const txKey = `tx_${txId}`;
        if (!nodeMap.has(txKey)) {
          nodeMap.set(txKey, {
            id: txKey, type: 'transaction', label: `TX ${txId}`,
            eventCount: 0, isBlocker: false, isVictim: false,
            isSelected: String(txId) === String(anchorTxId),
          });
        }
        const txEdgeKey = `${sessionKey}-${txKey}`;
        if (!edgeMap.has(txEdgeKey)) {
          edgeMap.set(txEdgeKey, { source: sessionKey, target: txKey, mode: '', relationship: 'transaction', count: 0 });
        }
        edgeMap.get(txEdgeKey)!.count++;
      }
    }
  }

  // Remove orphan transaction nodes (only connected to 1 session)
  for (const [key, node] of nodeMap) {
    if (node.type === 'transaction') {
      const edgeCount = Array.from(edgeMap.values()).filter(e => e.source === key || e.target === key).length;
      if (edgeCount <= 1) {
        nodeMap.delete(key);
        for (const [ek] of edgeMap) {
          if (ek.includes(key)) edgeMap.delete(ek);
        }
      }
    }
  }

  // Remove session nodes that have NO edges at all (no lock relationships)
  for (const [key, node] of nodeMap) {
    if (node.type === 'session') {
      const hasEdge = Array.from(edgeMap.values()).some(e => e.source === key || e.target === key);
      if (!hasEdge && !node.isSelected) {
        nodeMap.delete(key);
      }
    }
  }

  // Remove edges referencing deleted nodes
  for (const [ek, edge] of edgeMap) {
    if (!nodeMap.has(edge.source as string) || !nodeMap.has(edge.target as string)) {
      edgeMap.delete(ek);
    }
  }

  return { nodes: Array.from(nodeMap.values()), edges: Array.from(edgeMap.values()) };
};

const renderGraph = async () => {
  if (!viewport.value) return;
  loading.value = true;
  tooltip.value = null;

  try {
    let events: XelEvent[];
    let analysis: BlockingAnalysis | null = null;

    if (state.selectedEvent) {
      // Fetch both related events and blocking analysis in parallel
      const [evts, anal] = await Promise.all([
        xelApi.getRelatedEvents(state.selectedEvent.id, timeWindow.value * 1000, 2000),
        xelApi.analyzeBlocking(state.selectedEvent.id, timeWindow.value * 1000).catch(() => null),
      ]);
      events = evts;
      analysis = anal;
    } else {
      const response = await xelApi.queryEvents({
        filter: {
          ...state.filter,
          eventNames: state.filter.eventNames.length > 0 ? state.filter.eventNames : [
            'lock_acquired', 'lock_timeout_greater_than_0', 'lock_escalation',
            'blocked_process_report', 'rpc_completed', 'sql_batch_completed',
          ],
        },
        offset: 0, limit: 2000, sortBy: 'timestamp', sortDesc: false,
      });
      events = response.events;
    }

    const { nodes, edges } = buildGraph(events, state.selectedEvent, analysis);

    const container = viewport.value;
    d3.select(container).selectAll('*').remove();

    if (nodes.length === 0) {
      d3.select(container).append('div')
        .attr('class', 'flex items-center justify-center h-full text-slate-500 text-sm')
        .text(state.selectedEvent ? 'No lock relationships found for this event' : 'Select an event to see lock relationships');
      return;
    }

    const width = container.clientWidth;
    const height = container.clientHeight;

    const svg = d3.select(container).append('svg').attr('width', width).attr('height', height);

    svg.append('defs').selectAll('marker')
      .data(['holds', 'waits', 'transaction', 'deadlock_victim', 'deadlock_holder'])
      .join('marker')
      .attr('id', d => `arrow-${d}`)
      .attr('viewBox', '0 0 10 6').attr('refX', 10).attr('refY', 3)
      .attr('markerWidth', 8).attr('markerHeight', 6).attr('orient', 'auto')
      .append('path').attr('d', 'M0,0 L10,3 L0,6 Z')
      .attr('fill', d => d === 'holds' || d === 'deadlock_holder' ? '#22c55e' : d === 'waits' || d === 'deadlock_victim' ? '#ef4444' : '#6366f1');

    const g = svg.append('g');
    const zoom = d3.zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.1, 5])
      .on('zoom', (event) => { g.attr('transform', event.transform); tooltip.value = null; });
    svg.call(zoom);

    type SimNode = LockNode & d3.SimulationNodeDatum;
    type SimEdge = LockEdge & { source: SimNode | string; target: SimNode | string };

    const simNodes: SimNode[] = nodes.map(n => ({ ...n }));
    const simEdges: SimEdge[] = edges.map(e => ({ ...e }));

    const simulation = d3.forceSimulation<SimNode>(simNodes)
      .force('link', d3.forceLink<SimNode, SimEdge>(simEdges).id(d => d.id).distance(120))
      .force('charge', d3.forceManyBody().strength(-150))
      .force('center', d3.forceCenter(width / 2, height / 2))
      .force('collision', d3.forceCollide<SimNode>().radius(d => d.type === 'resource' ? Math.max(35, d.label.length * 3 + 8) : 35))
      .alphaDecay(0.05);

    const link = g.selectAll('.link').data(simEdges).join('line')
      .attr('stroke', d => {
        if (d.relationship === 'holds' || d.relationship === 'deadlock_holder') return '#22c55e';
        if (d.relationship === 'waits' || d.relationship === 'deadlock_victim') return '#ef4444';
        return '#6366f1';
      })
      .attr('stroke-width', d => Math.min(4, 1 + Math.log2(d.count + 1)))
      .attr('stroke-dasharray', d =>
        d.relationship === 'waits' || d.relationship === 'deadlock_victim' ? '5,3'
        : d.relationship === 'transaction' ? '2,4' : 'none')
      .attr('marker-end', d => `url(#arrow-${d.relationship})`)
      .attr('opacity', 0.7);

    const linkLabels = g.selectAll('.link-label').data(simEdges.filter(e => e.mode)).join('text')
      .attr('fill', '#94a3b8').attr('font-size', '9px').attr('text-anchor', 'middle')
      .text(d => `${d.mode}${d.count > 1 ? ` x${d.count}` : ''}`);

    let dragged = false;

    const node = g.selectAll('.node').data(simNodes).join('g')
      .attr('class', 'node').style('cursor', 'pointer')
      .on('click', (mouseEvent: MouseEvent, d) => {
        if (dragged) return;
        const rect = (viewport.value as HTMLElement).getBoundingClientRect();
        tooltip.value = {
          x: mouseEvent.clientX - rect.left + 10,
          y: mouseEvent.clientY - rect.top - 10,
          node: d,
        };
      })
      .call(d3.drag<SVGGElement, SimNode>()
        .on('start', (_event, d) => { dragged = false; d.fx = d.x; d.fy = d.y; })
        .on('drag', (event, d) => { dragged = true; if (!event.active) simulation.alphaTarget(0.3).restart(); d.fx = event.x; d.fy = event.y; })
        .on('end', (event, d) => { if (!event.active) simulation.alphaTarget(0); d.fx = null; d.fy = null; }) as any);

    node.each(function(d) {
      const el = d3.select(this);
      if (d.type === 'session') {
        const r = 18 + Math.min(12, Math.log2(d.eventCount + 1) * 3);
        el.append('circle').attr('r', r)
          .attr('fill', d.isSelected ? '#3b82f6' : d.isBlocker && d.isVictim ? '#f59e0b' : d.isBlocker ? '#ef4444' : d.isVictim ? '#f97316' : '#475569')
          .attr('stroke', d.isSelected ? '#93c5fd' : '#1e293b')
          .attr('stroke-width', d.isSelected ? 3 : 2);
      } else if (d.type === 'deadlock') {
        el.append('circle').attr('r', 22)
          .attr('fill', '#7f1d1d').attr('stroke', '#ef4444').attr('stroke-width', 2);
        el.append('text').attr('text-anchor', 'middle').attr('dominant-baseline', 'middle')
          .attr('fill', '#ef4444').attr('font-size', '14px')
          .text('\u2620'); // skull emoji
      } else if (d.type === 'resource') {
        const labelLen = d.label.length;
        const w = Math.max(70, labelLen * 6 + 16);
        el.append('rect').attr('x', -w / 2).attr('y', -14).attr('width', w).attr('height', 28).attr('rx', 4)
          .attr('fill', '#334155').attr('stroke', '#475569').attr('stroke-width', 1);
      } else {
        // Transaction
        el.append('rect').attr('x', -25).attr('y', -10).attr('width', 50).attr('height', 20).attr('rx', 10)
          .attr('fill', d.isSelected ? '#312e81' : '#1e1b4b').attr('stroke', '#4f46e5').attr('stroke-width', 1);
      }
    });

    // Labels
    node.append('text')
      .attr('dy', d => d.type === 'session' ? (20 + Math.min(12, Math.log2(d.eventCount + 1) * 3) + 2) : d.type === 'deadlock' ? 32 : 1)
      .attr('text-anchor', 'middle')
      .attr('dominant-baseline', d => d.type === 'session' || d.type === 'deadlock' ? 'auto' : 'middle')
      .attr('fill', d => d.type === 'deadlock' ? '#ef4444' : '#e2e8f0')
      .attr('font-size', d => d.type === 'session' || d.type === 'deadlock' ? '11px' : '8px')
      .attr('font-weight', d => d.isSelected ? 'bold' : 'normal')
      .text(d => {
        if (d.type === 'session') return d.label;
        if (d.type === 'deadlock') return 'DEADLOCK';
        if (d.type === 'transaction') return d.label;
        return d.label;
      });

    // Event count inside session circles
    node.filter(d => d.type === 'session').append('text')
      .attr('text-anchor', 'middle').attr('dominant-baseline', 'middle')
      .attr('fill', '#fff').attr('font-size', '10px').attr('font-weight', 'bold')
      .text(d => d.eventCount > 0 ? d.eventCount.toString() : '');

    simulation.on('tick', () => {
      link.attr('x1', d => (d.source as SimNode).x!).attr('y1', d => (d.source as SimNode).y!)
          .attr('x2', d => (d.target as SimNode).x!).attr('y2', d => (d.target as SimNode).y!);
      linkLabels
        .attr('x', d => ((d.source as SimNode).x! + (d.target as SimNode).x!) / 2)
        .attr('y', d => ((d.source as SimNode).y! + (d.target as SimNode).y!) / 2 - 6);
      node.attr('transform', d => `translate(${d.x}, ${d.y})`);
    });
  } finally {
    loading.value = false;
  }
};

let debounceHandle: ReturnType<typeof setTimeout> | null = null;
const scheduleRender = () => {
  if (debounceHandle) clearTimeout(debounceHandle);
  debounceHandle = setTimeout(renderGraph, 50);
};

watch(() => state.selectedEvent?.id, scheduleRender);
watch(() => state.revision, scheduleRender);
watch(timeWindow, scheduleRender);
watch(() => state.activeView, (v) => { if (v === 'lockchain') scheduleRender(); });

onMounted(() => {
  if (viewport.value) {
    resizeObserver = new ResizeObserver(() => {
      if (!loading.value) renderGraph();
    });
    resizeObserver.observe(viewport.value);
  }
  renderGraph();
});

onUnmounted(() => { resizeObserver?.disconnect(); });
</script>

<template>
  <div class="relative h-full flex flex-col">
    <div v-if="loading" class="absolute inset-0 flex items-center justify-center bg-slate-800/50 z-20">
      <i class="fa-solid fa-spinner fa-spin text-indigo-400 text-xl"></i>
    </div>

    <!-- Controls -->
    <div class="absolute top-2 left-2 z-10 flex items-center gap-2">
      <div class="flex items-center gap-1 bg-slate-800/90 rounded-lg border border-slate-600 px-2 py-1 text-xs text-slate-400">
        <span>Window:</span>
        <select v-model.number="timeWindow" class="bg-slate-700 text-slate-300 border border-slate-600 rounded px-1 py-0.5 outline-none text-xs appearance-none cursor-pointer">
          <option :value="5" class="bg-slate-700 text-slate-300">5s</option>
          <option :value="15" class="bg-slate-700 text-slate-300">15s</option>
          <option :value="30" class="bg-slate-700 text-slate-300">30s</option>
          <option :value="60" class="bg-slate-700 text-slate-300">1min</option>
          <option :value="300" class="bg-slate-700 text-slate-300">5min</option>
        </select>
      </div>
    </div>

    <!-- Legend -->
    <div class="absolute top-2 right-2 z-10 flex gap-2 text-xs text-slate-400 bg-slate-800/90 px-3 py-1.5 rounded-lg border border-slate-600">
      <span><span class="inline-block w-3 h-3 rounded-full bg-red-500 mr-0.5"></span>Blocker</span>
      <span><span class="inline-block w-3 h-3 rounded-full bg-orange-500 mr-0.5"></span>Victim</span>
      <span><span class="inline-block w-3 h-3 rounded-full bg-blue-500 mr-0.5"></span>Selected</span>
      <span class="border-l border-slate-600 pl-2">
        <span class="text-green-400">--</span>Holds
        <span class="text-red-400 ml-1">- -</span>Waits
        <span class="text-indigo-400 ml-1">..</span>TX
      </span>
    </div>

    <!-- Empty state -->
    <div v-if="!loading && !hasSelection" class="flex flex-col items-center justify-center h-full text-slate-500">
      <i class="fa-solid fa-diagram-project text-3xl mb-3 text-slate-600"></i>
      <p class="text-sm">Select an event in the table</p>
      <p class="text-xs mt-1">Lock chain shows sessions competing for resources around the selected event</p>
    </div>

    <div ref="viewport" class="flex-1 overflow-hidden" @click.self="tooltip = null"></div>

    <!-- Tooltip -->
    <div
      v-if="tooltip"
      class="absolute z-30 bg-slate-800 border border-slate-600 rounded-lg shadow-xl px-3 py-2 text-xs max-w-xs pointer-events-auto"
      :style="{ left: tooltip.x + 'px', top: tooltip.y + 'px' }"
    >
      <button @click="tooltip = null" class="absolute top-1 right-1.5 text-slate-500 hover:text-slate-300">
        <i class="fa-solid fa-xmark"></i>
      </button>
      <div class="font-medium text-slate-200 mb-1">{{ tooltip.node.label }}</div>
      <div class="grid grid-cols-[auto_1fr] gap-x-2 gap-y-0.5 text-slate-400">
        <span>Type</span><span class="text-slate-300 capitalize">{{ tooltip.node.type }}</span>
        <template v-if="tooltip.node.eventCount > 0">
          <span>Events</span><span class="text-slate-300">{{ tooltip.node.eventCount }}</span>
        </template>
        <template v-if="tooltip.node.isBlocker">
          <span>Role</span><span class="text-red-400">Blocker</span>
        </template>
        <template v-if="tooltip.node.isVictim">
          <span>Role</span><span class="text-orange-400">Victim</span>
        </template>
        <template v-if="tooltip.node.username">
          <span>Login</span><span class="text-slate-300">{{ tooltip.node.username }}</span>
        </template>
        <template v-if="tooltip.node.appName">
          <span>App</span><span class="text-slate-300">{{ tooltip.node.appName }}</span>
        </template>
        <template v-if="tooltip.node.database">
          <span>Database</span><span class="text-slate-300">{{ tooltip.node.database }}</span>
        </template>
        <template v-if="tooltip.node.hostname">
          <span>Host</span><span class="text-slate-300">{{ tooltip.node.hostname }}</span>
        </template>
        <template v-if="tooltip.node.resourceType">
          <span>Lock Type</span><span class="text-slate-300">{{ tooltip.node.resourceType }}</span>
        </template>
        <template v-if="tooltip.node.waitTimeMs">
          <span>Wait</span><span class="text-slate-300">{{ (tooltip.node.waitTimeMs / 1000).toFixed(1) }}s</span>
        </template>
      </div>
      <div
        v-if="tooltip.node.sqlPreview"
        class="mt-1.5 p-1.5 bg-slate-900 rounded text-[10px] font-mono text-slate-400 max-h-16 overflow-y-auto whitespace-pre-wrap break-all"
      >{{ tooltip.node.sqlPreview.substring(0, 200) }}{{ tooltip.node.sqlPreview.length > 200 ? '…' : '' }}</div>
      <button
        v-if="tooltip.node.type === 'session' && tooltip.node.sessionId"
        @click="selectSession(tooltip.node.sessionId!); tooltip = null"
        class="mt-2 w-full px-2 py-1 bg-indigo-600 hover:bg-indigo-500 text-white rounded text-xs transition-colors"
      >
        <i class="fa-solid fa-filter mr-1"></i>Filter by Session {{ tooltip.node.sessionId }}
      </button>
    </div>
  </div>
</template>
