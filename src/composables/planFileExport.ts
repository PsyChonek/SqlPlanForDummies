import { tauriInvoke } from './tauriApi';

const pad = (n: number) => String(n).padStart(2, '0');

/** Default file name for an exported plan, e.g. plan-20261005-113600.sqlplan */
export const suggestedPlanFileName = (date = new Date()): string =>
  `plan-${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}`
  + `-${pad(date.getHours())}${pad(date.getMinutes())}${pad(date.getSeconds())}.sqlplan`;

/** Opens a save dialog and writes the plan XML. Resolves to the saved path, or null if cancelled. */
export const savePlanFile = (xml: string, suggestedName = suggestedPlanFileName()) =>
  tauriInvoke<string | null>('save_plan_file', { xml, suggestedName });
