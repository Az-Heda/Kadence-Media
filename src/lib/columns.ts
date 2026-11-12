import type { SortBy } from './api';

export interface ColumnDef {
  id: string;
  /** Name in Settings. */
  name: string;
  /** Header text in the table. */
  label: string;
  width: number;
  sort?: SortBy;
  num?: boolean;
}

/** Table columns in their default order. */
export const COLUMNS: ColumnDef[] = [
  { id: 'play', name: 'Play button', label: '', width: 70 },
  { id: 'watched', name: 'Watched', label: 'Watched', width: 90 },
  { id: 'rating', name: 'Rating', label: 'Rating', width: 120, sort: 'rating' },
  { id: 'title', name: 'Title', label: 'Title', width: 320, sort: 'title' },
  { id: 'created', name: 'Created', label: 'Created', width: 170, sort: 'created' },
  { id: 'duration', name: 'Duration', label: 'Duration', width: 100, sort: 'duration', num: true },
  { id: 'size', name: 'Size', label: 'Size', width: 100, sort: 'size', num: true },
  { id: 'path', name: 'Folder', label: 'Folder', width: 220, sort: 'path' },
  { id: 'tags', name: 'Tags', label: 'Tags', width: 220 },
  { id: 'actions', name: 'Tags & playlists button', label: '', width: 90 },
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
