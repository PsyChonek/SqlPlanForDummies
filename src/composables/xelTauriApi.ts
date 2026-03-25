import { tauriInvoke } from './tauriApi';
import type {
  PowerShellStatus,
  XelLoadRequest,
  XelSessionStats,
  XelQueryRequest,
  XelQueryResponse,
  XelEvent,
  XelFilter,
  TimelineRequest,
  TimelineBucket,
  BlockingAnalysis,
  XelProblemStats,
  XelEnrichResult,
  TransactionObject,
} from '../types/xel';

export function pickFiles(): Promise<string[]> {
  return tauriInvoke<string[]>('xel_pick_files');
}

export function checkPowerShell(): Promise<PowerShellStatus> {
  return tauriInvoke<PowerShellStatus>('xel_check_powershell');
}

export function loadXelFiles(request: XelLoadRequest): Promise<XelSessionStats> {
  return tauriInvoke<XelSessionStats>('xel_load_files', { request });
}

export function queryEvents(request: XelQueryRequest): Promise<XelQueryResponse> {
  return tauriInvoke<XelQueryResponse>('xel_query_events', { request });
}

export function getRelatedEvents(eventId: number, timeWindowMs?: number, limit?: number): Promise<XelEvent[]> {
  return tauriInvoke<XelEvent[]>('xel_get_related_events', {
    eventId,
    timeWindowMs: timeWindowMs ?? 30000,
    limit: limit ?? 2000,
  });
}

export function getEvent(id: number): Promise<XelEvent | null> {
  return tauriInvoke<XelEvent | null>('xel_get_event', { id });
}

export function getStats(filter: XelFilter): Promise<XelSessionStats> {
  return tauriInvoke<XelSessionStats>('xel_get_stats', { filter });
}

export function getTimeline(request: TimelineRequest): Promise<TimelineBucket[]> {
  return tauriInvoke<TimelineBucket[]>('xel_get_timeline', { request });
}

export function getColumns(): Promise<string[]> {
  return tauriInvoke<string[]>('xel_get_columns');
}

export function getDistinctValues(field: string): Promise<string[]> {
  return tauriInvoke<string[]>('xel_get_distinct_values', { field });
}

export function getProblemStats(filter: XelFilter): Promise<XelProblemStats> {
  return tauriInvoke<XelProblemStats>('xel_get_problem_stats', { filter });
}

export function analyzeBlocking(eventId: number, timeWindowMs?: number): Promise<BlockingAnalysis> {
  return tauriInvoke<BlockingAnalysis>('xel_analyze_blocking', {
    eventId,
    timeWindowMs: timeWindowMs ?? 60000,
  });
}

export function enrichFromDb(): Promise<XelEnrichResult> {
  return tauriInvoke<XelEnrichResult>('xel_enrich_from_db');
}

export function getTransactionObjects(eventId: number): Promise<TransactionObject[]> {
  return tauriInvoke<TransactionObject[]>('xel_get_transaction_objects', { eventId });
}

export function clearXelData(): Promise<void> {
  return tauriInvoke<void>('xel_clear');
}
