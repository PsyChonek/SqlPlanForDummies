/**
 * Human-readable descriptions for SQL Server wait types
 * shown in the Wait Statistics panel.
 */

// Exact-match descriptions for common wait types
const waitTypeDescriptions: Record<string, string> = {
  // Parallelism (CX*) waits
  CXPACKET:
    'Parallelism: threads of a parallel query synchronizing at exchange operators. Some CXPACKET is normal for parallel plans; high values usually mean uneven work distribution between threads (often caused by skewed data or bad estimates), or a too-high degree of parallelism.',
  CXCONSUMER:
    'Parallelism: a consumer thread waiting for rows from producer threads. Usually benign - it just means the producers (e.g. a scan) are the slow part.',
  CXSYNC_PORT:
    'Parallelism: waiting to open or close an exchange port when parallel threads start or finish. Normally short; long waits suggest one thread finished much later than the others (skewed work).',
  CXSYNC_CONSUMER:
    'Parallelism: waiting for consumer threads of an exchange to synchronize. Usually accompanies CXPACKET on skewed parallel queries.',
  CXROWSET_SYNC:
    'Parallelism: threads synchronizing during a parallel range scan of the same table. High values suggest threads are fighting over scan ranges.',
  SESSION_WAIT_STATS_CHILDREN:
    'Parallelism bookkeeping: the parent task collecting wait statistics from its parallel child threads. Informational, not a performance problem by itself.',

  // CPU / scheduling
  RESOURCE_GOVERNOR_IDLE:
    'CPU throttling: the task had CPU work to do but Resource Governor capped the available CPU. On Azure SQL Database this means the query hit the CPU limit of the service tier - the database is undersized for this workload, or the query needs less CPU.',
  EXECSYNC:
    'Parallelism: a parallel worker waiting for another worker to finish building a shared structure (e.g. an index build phase or a spool). Long waits mean one thread did the work while the others idled.',
  SOS_SCHEDULER_YIELD:
    'CPU pressure: the task voluntarily gave up its CPU quantum and waited to get scheduled again. Sustained high values indicate CPU-bound workload.',
  THREADPOOL:
    'Worker thread starvation: the server had no free worker threads to run the task. Often a symptom of massive blocking or too many concurrent requests.',

  // Memory
  RESOURCE_SEMAPHORE:
    'Memory grant wait: the query waited for enough workspace memory (for sorts/hashes) to start. Indicates memory pressure from concurrent memory-hungry queries.',
  MEMORY_ALLOCATION_EXT:
    'Waiting on a memory allocation from the OS or an internal pool. Individual waits are microseconds; usually noise even with high counts.',
  RESERVED_MEMORY_ALLOCATION_EXT:
    'Waiting on a large-page memory allocation. Like MEMORY_ALLOCATION_EXT, usually noise.',

  // IO
  IO_COMPLETION:
    'Waiting for non-data-page I/O to finish (e.g. sort/hash spill reads and writes, allocation structures). High values with spill warnings point to tempdb spills.',
  ASYNC_IO_COMPLETION:
    'Waiting for an asynchronous I/O operation to finish (backups, bulk operations, data file growth).',
  WRITELOG:
    'Waiting for the transaction log flush to disk (commit). High values mean log-disk latency or very frequent small commits.',
  ASYNC_NETWORK_IO:
    'Waiting for the client to consume result rows. Usually the client application reading too slowly or row-by-row, not a server problem.',
};

// Prefix-match descriptions for wait type families
const waitTypePrefixes: Array<{ prefix: string; description: string }> = [
  {
    prefix: 'LCK_M_',
    description:
      'Lock wait: the query was blocked waiting for a lock held by another session. The plan itself may be fine - investigate what was blocking (the suffix tells the requested lock mode, e.g. S = shared, U = update, X = exclusive, SCH_M = schema modify).',
  },
  {
    prefix: 'PAGEIOLATCH_',
    description:
      'Data page read from disk: waiting for a page to be brought into the buffer pool. High values mean the query is I/O-bound - slow storage, cold cache, or reading more pages than necessary (missing index).',
  },
  {
    prefix: 'PAGELATCH_',
    description:
      'In-memory page contention (not I/O): multiple threads modifying the same page in the buffer pool. Classic causes are tempdb allocation contention and last-page insert hotspots.',
  },
  {
    prefix: 'LATCH_',
    description:
      'Non-page latch contention on internal structures (e.g. parallel scan range enumerators). Often accompanies parallelism waits.',
  },
  {
    prefix: 'RESOURCE_SEMAPHORE_QUERY_COMPILE',
    description:
      'Waiting for memory to compile the query. Indicates many large query compilations at once.',
  },
  {
    prefix: 'HADR_',
    description: 'Availability Group synchronization wait.',
  },
  {
    prefix: 'BACKUP',
    description: 'Waiting on a backup operation.',
  },
];

/**
 * Get a human-readable description for a wait type,
 * or undefined for uncommon types with no entry.
 */
export function getWaitTypeDescription(waitType: string): string | undefined {
  const exact = waitTypeDescriptions[waitType];
  if (exact) return exact;

  const byPrefix = waitTypePrefixes.find(p => waitType.startsWith(p.prefix));
  return byPrefix?.description;
}
