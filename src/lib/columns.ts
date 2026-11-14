import type { SortBy } from './api';
import { t, type Key } from './i18n.svelte';

export interface ColumnDef {
  id: string;
  width: number;
  sort?: SortBy;
  num?: boolean;
}

/** Table columns in their default order. */
export const COLUMNS: ColumnDef[] = [
  { id: 'play', width: 70 },
  { id: 'watched', width: 90 },
  { id: 'rating', width: 120, sort: 'rating' },
  { id: 'title', width: 320, sort: 'title' },
  { id: 'created', width: 170, sort: 'created' },
  { id: 'duration', width: 100, sort: 'duration', num: true },
  { id: 'size', width: 100, sort: 'size', num: true },
  { id: 'path', width: 220, sort: 'path' },
  { id: 'tags', width: 220 },
  { id: 'actions', width: 90 },
];

/** Width of the drag handle column (shown only while a playlist can be reordered). */
export const DRAG_WIDTH = 40;

export interface ColumnPref { id: string; visible: boolean }

/** Parse the saved `table_columns` setting; unknown ids are dropped and missing ones appended (visible). */
export function resolveColumns(raw: string): ColumnPref[] {
  let saved: ColumnPref[] = [];
  try {
    const parsed = JSON.parse(raw || '[]');
    if (Array.isArray(parsed)) saved = parsed.filter((c) => c && typeof c.id === 'string');
  } catch { /* fall back to defaults */ }
  const known = new Set(COLUMNS.map((c) => c.id));
  const out: ColumnPref[] = [];
  for (const c of saved) {
    if (known.has(c.id) && !out.some((o) => o.id === c.id)) out.push({ id: c.id, visible: c.visible !== false });
  }
  for (const c of COLUMNS) if (!out.some((o) => o.id === c.id)) out.push({ id: c.id, visible: true });
  return out;
}

export const columnDef = (id: string) => COLUMNS.find((c) => c.id === id)!;

/** Name of a column in Settings. */
export const colName = (id: string) => t(`col.${id}.name` as Key);
/** Header text of a column in the table. */
export const colLabel = (id: string) => t(`col.${id}.label` as Key);
