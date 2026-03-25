export interface XelEvent {
  id: number;
  sourceFile: string;
  eventName: string;
  timestamp: string;
  sessionId: number | null;
  durationUs: number | null;
  cpuTimeUs: number | null;
  logicalReads: number | null;
  physicalReads: number | null;
  writes: number | null;
  result: string | null;
  statement: string | null;
  sqlText: string | null;
  objectName: string | null;
  clientAppName: string | null;
  username: string | null;
  databaseName: string | null;
  resourceType: string | null;
  lockMode: string | null;
  resourceDescription: string | null;
  waitType: string | null;
  waitDurationMs: number | null;
  blockedProcessReport: string | null;
  deadlockGraph: string | null;
  extraFields: Record<string, unknown>;
}

export interface XelFilter {
  eventNames: string[];
  timeFrom: string | null;
  timeTo: string | null;
  sessionIds: number[];
  objectNameContains: string | null;
  sqlTextContains: string | null;
  username: string | null;
  clientAppName: string | null;
  databaseName: string | null;
  minDurationUs: number | null;
  maxDurationUs: number | null;
  sourceFile: string | null;
  textSearch: string | null;
  result: string | null;
  errorsOnly: boolean;
}

export interface XelQueryRequest {
  filter: XelFilter;
  offset: number;
  limit: number;
  sortBy: string | null;
  sortDesc: boolean;
}

export interface XelQueryResponse {
  events: XelEvent[];
  totalCount: number;
  offset: number;
  limit: number;
}

export interface XelEventSummary {
  id: number;
  eventName: string;
  timestamp: string;
  durationUs: number | null;
  logicalReads: number | null;
  statementPreview: string | null;
  sessionId: number | null;
}

export interface XelSessionStats {
  totalEvents: number;
  eventTypeCounts: Record<string, number>;
  timeRangeStart: string | null;
  timeRangeEnd: string | null;
  uniqueSessions: number[];
  uniqueDatabases: string[];
  uniqueUsers: string[];
  uniqueApps: string[];
  filesLoaded: string[];
  topByDuration: XelEventSummary[];
  topByReads: XelEventSummary[];
}

export interface XelLoadProgress {
  fileName: string;
  eventsParsed: number;
  bytesProcessed: number;
  totalBytes: number;
  phase: 'starting' | 'checkingPowerShell' | 'parsing' | 'indexing' | 'complete' | 'error';
}

export interface PowerShellStatus {
  available: boolean;
  sqlServerModule: boolean;
  dbatoolsModule: boolean;
  message: string;
}

export interface XelLoadRequest {
  filePaths: string[];
  append: boolean;
}

export interface TimelineBucket {
  bucketStart: string;
  bucketEnd: string;
  eventCount: number;
  avgDurationUs: number | null;
  maxDurationUs: number | null;
  totalLogicalReads: number;
  eventTypeCounts: Record<string, number>;
}

export interface TimelineRequest {
  filter: XelFilter;
  bucketCount: number;
}

export interface XelFile {
  path: string;
  name: string;
  sizeBytes: number;
  eventCount: number;
}

export interface BlockingAnalysis {
  anchorEventId: number;
  summary: string;
  blockedProcessReports: ParsedBlockedProcessReport[];
  blockingChain: BlockingChainLink[];
  blockerEvents: XelEvent[];
  lockEscalations: XelEvent[];
  waitEvents: XelEvent[];
  waitStats: WaitTypeStat[];
  deadlocks: ParsedDeadlockGraph[];
  diagnosis: string;
  recommendations: string[];
}

export interface ParsedDeadlockGraph {
  eventId: number;
  timestamp: string;
  processes: DeadlockProcess[];
  resources: DeadlockResource[];
}

export interface DeadlockProcess {
  id: string;
  spid: number | null;
  isVictim: boolean;
  lockMode: string | null;
  waitResource: string | null;
  waitTimeMs: number | null;
  transactionName: string | null;
  logUsed: number | null;
  inputBuffer: string | null;
  databaseName: string | null;
  hostname: string | null;
  appName: string | null;
  loginName: string | null;
  isolationLevel: string | null;
  status: string | null;
}

export interface DeadlockResource {
  resourceType: string;
  databaseName: string | null;
  objectName: string | null;
  indexName: string | null;
  mode: string | null;
  holders: DeadlockResourceOwner[];
  waiters: DeadlockResourceOwner[];
}

export interface DeadlockResourceOwner {
  processId: string;
  mode: string | null;
}

export interface WaitTypeStat {
  waitType: string;
  count: number;
  totalDurationUs: number;
  maxDurationUs: number;
  avgDurationUs: number;
  category: string;
}

export interface ParsedBlockedProcessReport {
  eventId: number;
  timestamp: string;
  blockedSpid: number | null;
  blockedXactId: string | null;
  blockedWaitResource: string | null;
  blockedWaitTimeMs: number | null;
  blockedLockMode: string | null;
  blockedInputBuffer: string | null;
  blockedDatabase: string | null;
  blockedHostname: string | null;
  blockedAppName: string | null;
  blockedLoginName: string | null;
  blockingSpid: number | null;
  blockingXactId: string | null;
  blockingInputBuffer: string | null;
  blockingDatabase: string | null;
  blockingHostname: string | null;
  blockingAppName: string | null;
  blockingLoginName: string | null;
  blockingStatus: string | null;
  blockingLastBatchStarted: string | null;
}

export interface BlockingChainLink {
  sessionId: number;
  role: string;
  waitResource: string | null;
  lockMode: string | null;
  sqlPreview: string | null;
  appName: string | null;
  username: string | null;
  database: string | null;
  eventIds: number[];
  blockedBySession: number | null;
}

export interface XelProblemStats {
  deadlockCount: number;
  errorCount: number;
  timeoutCount: number;
  blockedProcessCount: number;
  lockWaitCount: number;
  topWaitTypes: WaitTypeStat[];
  errorSessions: SessionProblemStat[];
  waitSessions: SessionProblemStat[];
}

export interface SessionProblemStat {
  sessionId: number;
  count: number;
  totalDurationUs: number;
  sampleEventName: string;
  sampleObjectName: string | null;
}

export interface XelEnrichResult {
  databasesResolved: number;
  objectsResolved: number;
  queryTextsResolved: number;
  uniqueDatabases: number;
  uniqueObjects: number;
  uniqueQueries: number;
  errors: string[];
}

export interface TransactionObject {
  objectName: string;
  resourceType: string | null;
  lockModes: string[];
  eventCount: number;
  sampleEventId: number;
}

export type EventSeverity = 'normal' | 'warning' | 'error' | 'lock' | 'deadlock';

export function getEventSeverity(event: XelEvent): EventSeverity {
  if (event.eventName.includes('deadlock')) return 'deadlock';
  if (event.result === 'Error' || event.result === 'Abort') return 'error';
  if (event.eventName.startsWith('lock_') || event.eventName === 'blocked_process_report') return 'lock';
  if (event.durationUs && event.durationUs > 5_000_000) return 'warning';
  return 'normal';
}

export function getEventSeverityColor(severity: EventSeverity): string {
  switch (severity) {
    case 'deadlock': return '#ef4444';
    case 'error': return '#ef4444';
    case 'warning': return '#f97316';
    case 'lock': return '#eab308';
    case 'normal': return '#94a3b8';
  }
}

export function getEventSeverityBg(severity: EventSeverity): string {
  switch (severity) {
    case 'deadlock': return 'bg-red-900/30 border-red-700/50';
    case 'error': return 'bg-red-900/20 border-red-800/30';
    case 'warning': return 'bg-orange-900/20 border-orange-800/30';
    case 'lock': return 'bg-yellow-900/20 border-yellow-800/30';
    case 'normal': return '';
  }
}

export function getEventIcon(eventName: string): string {
  if (eventName.includes('deadlock')) return 'fa-skull-crossbones';
  if (eventName.includes('lock_acquired')) return 'fa-lock';
  if (eventName.includes('lock_released')) return 'fa-lock-open';
  if (eventName.includes('lock_escalation')) return 'fa-arrow-up';
  if (eventName.includes('lock_timeout')) return 'fa-clock';
  if (eventName.includes('blocked_process')) return 'fa-ban';
  if (eventName.includes('rpc_completed')) return 'fa-server';
  if (eventName.includes('sql_batch_completed')) return 'fa-code';
  if (eventName.includes('wait_completed')) return 'fa-hourglass';
  return 'fa-circle-dot';
}

export function formatDuration(us: number | null): string {
  if (us === null) return '-';
  if (us < 1000) return `${us}\u00B5s`;
  if (us < 1_000_000) return `${(us / 1000).toFixed(1)}ms`;
  return `${(us / 1_000_000).toFixed(2)}s`;
}

export function formatNumber(n: number | null): string {
  if (n === null) return '-';
  return n.toLocaleString();
}

export function formatTimestamp(ts: string): string {
  const d = new Date(ts);
  const m = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  const time = d.toLocaleTimeString('en-US', { hour12: false });
  const ms = String(d.getMilliseconds()).padStart(3, '0');
  return `${m}-${day} ${time}.${ms}`;
}

export function formatTimestampFull(ts: string): string {
  const d = new Date(ts);
  const date = d.toLocaleDateString('en-US');
  const time = d.toLocaleTimeString('en-US', { hour12: false });
  const ms = String(d.getMilliseconds()).padStart(3, '0');
  return `${date} ${time}.${ms}`;
}

export function emptyFilter(): XelFilter {
  return {
    eventNames: [],
    timeFrom: null,
    timeTo: null,
    sessionIds: [],
    objectNameContains: null,
    sqlTextContains: null,
    username: null,
    clientAppName: null,
    databaseName: null,
    minDurationUs: null,
    maxDurationUs: null,
    sourceFile: null,
    textSearch: null,
    result: null,
    errorsOnly: false,
  };
}
