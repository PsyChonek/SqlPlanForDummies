interface DeadlockSummary {
  victimObjects: string;
  causedBy: string;
}

const cache = new Map<number, DeadlockSummary>();

const GUID_RE = /^\{?[A-F0-9]{8}-[A-F0-9]{4}-[A-F0-9]{4}-[A-F0-9]{4}-[A-F0-9]{12}\}?$/i;

const stripAutoGenSuffix = (segment: string): string => {
  // SQL Server auto-generated constraint/index names end with a hex hash after
  // double underscores, e.g. PK__Table__3214EC077E234ABC, UQ__IA_Compa__E04C2F4A.
  // Also strip trailing GUID (with or without dashes) attached via underscore.
  let s = segment.replace(/__[A-F0-9]{6,}$/i, '');
  s = s.replace(/_\{?[A-F0-9]{8}-[A-F0-9]{4}-[A-F0-9]{4}-[A-F0-9]{4}-[A-F0-9]{12}\}?$/i, '');
  return s;
};

const cleanObjectName = (raw: string | null | undefined): string | null => {
  if (!raw) return null;
  const trimmed = raw.trim();
  if (!trimmed) return null;
  // Drop bare-GUID segments (e.g. Azure SQL dbs appear as a GUID in the db slot)
  const segments = trimmed
    .split('.')
    .map(s => s.replace(/^\[|\]$/g, '').trim())
    .filter(s => s.length > 0 && !GUID_RE.test(s))
    .map(stripAutoGenSuffix);
  const joined = segments.join('.');
  return joined.length > 0 ? joined : null;
};

const findNestedObjectName = (res: Element): string | null => {
  // Some resource wrappers (xactlock, exchangeEvent) don't carry objectname on the
  // outer element — walk descendants looking for the first real objectname.
  const nested = res.querySelectorAll('[objectname]');
  for (const el of Array.from(nested)) {
    const name = cleanObjectName(el.getAttribute('objectname'));
    if (name) return name;
  }
  return null;
};

const parseWaitResource = (waitResource: string | null | undefined): string | null => {
  if (!waitResource) return null;
  // KEY: 6:72057710852505600 (hash) — object is not named inline
  // PAGE: 6:1:12345 — not named
  // OBJECT: 6:12345 — object id
  // RID: 6:1:12345:0 — not named
  const objMatch = waitResource.match(/OBJECT:\s*\d+:(\d+)/i);
  if (objMatch) return `objectid=${objMatch[1]}`;
  return null;
};

const extractProcNameFromStack = (processEl: Element): string | null => {
  const frames = Array.from(processEl.querySelectorAll('executionStack > frame'));
  for (const f of frames) {
    const proc = cleanObjectName(f.getAttribute('procname'));
    if (proc && proc !== 'unknown' && proc !== 'adhoc') return proc;
  }
  return null;
};

export function summarizeDeadlock(eventId: number, xml: string | null): DeadlockSummary {
  if (!xml) return { victimObjects: '', causedBy: '' };
  const cached = cache.get(eventId);
  if (cached) return cached;

  const result: DeadlockSummary = { victimObjects: '', causedBy: '' };
  let doc: Document;
  try {
    doc = new DOMParser().parseFromString(xml, 'application/xml');
  } catch {
    return result;
  }
  if (doc.querySelector('parsererror')) return result;

  const victimIds = new Set<string>();
  doc.querySelectorAll('victim-list > victimProcess').forEach(el => {
    const id = el.getAttribute('id');
    if (id) victimIds.add(id);
  });

  interface ProcInfo {
    spid: string | null;
    waitResource: string | null;
    procName: string | null;
  }
  const processInfo = new Map<string, ProcInfo>();
  doc.querySelectorAll('process-list > process').forEach(el => {
    const id = el.getAttribute('id') || '';
    processInfo.set(id, {
      spid: el.getAttribute('spid'),
      waitResource: el.getAttribute('waitresource'),
      procName: extractProcNameFromStack(el),
    });
  });

  const victimObjSet = new Set<string>();
  const causerSet = new Set<string>();

  const resources = Array.from(doc.querySelectorAll('resource-list > *'));
  for (const res of resources) {
    const waiters = Array.from(res.querySelectorAll(':scope > waiter-list > waiter'));
    const owners = Array.from(res.querySelectorAll(':scope > owner-list > owner'));

    const victimWaiter = waiters.find(w => {
      const pid = w.getAttribute('id');
      return pid && victimIds.has(pid);
    });
    if (!victimWaiter) continue;

    const victimId = victimWaiter.getAttribute('id') || '';
    const victimProc = processInfo.get(victimId);

    const obj =
      cleanObjectName(res.getAttribute('objectname')) ??
      findNestedObjectName(res) ??
      parseWaitResource(victimProc?.waitResource) ??
      victimProc?.procName ??
      null;

    if (obj) victimObjSet.add(obj);

    for (const owner of owners) {
      const ownerId = owner.getAttribute('id');
      if (!ownerId || victimIds.has(ownerId)) continue;
      const info = processInfo.get(ownerId);
      const spid = info?.spid;
      causerSet.add(spid ? `SPID ${spid}` : ownerId);
    }
  }

  // Fallback: if no victim object captured, derive from each victim process
  if (victimObjSet.size === 0) {
    for (const vid of victimIds) {
      const info = processInfo.get(vid);
      const obj = parseWaitResource(info?.waitResource) ?? info?.procName;
      if (obj) victimObjSet.add(obj);
    }
  }

  result.victimObjects = Array.from(victimObjSet).join(', ');
  result.causedBy = Array.from(causerSet).join(', ');
  cache.set(eventId, result);
  return result;
}

export function clearDeadlockSummaryCache(): void {
  cache.clear();
}
