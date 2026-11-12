import { api, errMsg, type SortBy, type VideoFilter, type Folder, type Playlist, type PlayerState, type Settings, type Tag, type Video } from './api';

export type Page = 'library' | 'folders' | 'collections' | 'rules' | 'settings';

export const DEFAULT_PRIMARY = '#d4a017';

export const app = $state({
  page: 'library' as Page,
  playerView: false,
  settings: { volume_max: '300', volume: '100', primary_color: DEFAULT_PRIMARY, embed_video: 'true', mpv_path: '', library_view: 'grid', default_folder_id: '', escape_action: 'stop', grid_page_size: '60', table_page_size: '20', col_widths: '' } as Settings,
  folders: [] as Folder[],
  tags: [] as Tag[],
  playlists: [] as Playlist[],
  videos: [] as Video[],
  /// Pagination of the library list (page is 0-based).
  paging: { page: 0, total: 0 },
  player: {
    video_id: null, loaded: false, idle: true, paused: false, position: 0, duration: 0, volume: 100, muted: false, speed: 1,
  } as PlayerState,
  dnd: { dragging: null, target: null, after: false } as { dragging: number | null; target: number | null; after: boolean },
  indexing: { running: false, done: 0, total: 0 } as { running: boolean; done: number; total: number },
  queue: [] as number[],
  /// Videos of the current queue, shown in the player's side panel.
  queueVideos: [] as Video[],
  /// Set when the queue comes from a playlist (enables the side panel).
  queueTitle: null as string | null,
  toast: null as { text: string; error: boolean } | null,
  filter: {
    search: '',
    watched: null as boolean | null,
    tag_ids: [] as number[],
    playlist_id: null as number | null,
    folder_id: null as number | null,
    sort_by: 'path' as SortBy,
    sort_desc: false,
  },
});

let toastTimer: ReturnType<typeof setTimeout> | undefined;
export function toast(text: string, error = false) {
  app.toast = { text, error };
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (app.toast = null), error ? 6000 : 2500);
}

/** Run an async action, surfacing failures as an error toast. */
export async function guarded<T>(fn: () => Promise<T>): Promise<T | undefined> {
  try {
    return await fn();
  } catch (e) {
    toast(errMsg(e), true);
    return undefined;
  }
}

/** The folder marked as default in Folders (null = all folders). */
export const defaultFolderId = (): number | null => Number(app.settings.default_folder_id) || null;

export async function setDefaultFolder(id: number | null) {
  await guarded(async () => {
    const previous = defaultFolderId();
    await api.setDefaultFolder(id);
    app.settings.default_folder_id = id == null ? '' : String(id);
    // Move the library filter along if it was showing the old default.
    if (app.filter.folder_id === previous) app.filter.folder_id = id;
    app.folders = await api.listFolders();
  });
}

export const volumeMax = () => Math.max(100, Number(app.settings.volume_max) || 300);

export function applyTheme() {
  const root = document.documentElement;
  root.style.setProperty('--primary', app.settings.primary_color || DEFAULT_PRIMARY);
}

export async function loadSettings() {
  app.settings = { ...app.settings, ...(await api.getSettings()) };
  applyTheme();
}

export async function saveSetting(key: keyof Settings, value: string) {
  await guarded(async () => {
    await api.setSetting(key, value);
    app.settings[key] = value;
    if (key === 'primary_color') applyTheme();
  });
}

/** Items per page for the current library view (grid / table). */
export function pageSize(): number {
  const table = app.settings.library_view === 'table';
  const v = Number(table ? app.settings.table_page_size : app.settings.grid_page_size);
  return Math.min(1000, Math.max(1, Math.round(v) || (table ? 20 : 60)));
}

export function libraryFilter(): VideoFilter {
  const f = app.filter;
  return {
    search: f.search,
    watched: f.watched,
    tag_ids: [...f.tag_ids],
    playlist_id: f.playlist_id,
    folder_id: f.folder_id,
    sort_by: f.sort_by,
    sort_desc: f.sort_desc,
  };
}

/** Load the current page: filtering, sorting and slicing all happen in the backend. */
export async function loadVideos() {
  const res = await guarded(() => api.listVideosPage(libraryFilter(), app.paging.page, pageSize()));
  if (res) {
    app.videos = res.items;
    app.paging.page = res.page; // the backend clamps it to the last page
    app.paging.total = res.total;
  }
}

export function setPage(page: number) {
  app.paging.page = Math.max(0, page);
  loadVideos();
}

export async function loadMeta() {
  const [folders, tags, playlists] = await Promise.all([api.listFolders(), api.listTags(), api.listPlaylists()]);
  app.folders = folders;
  app.tags = tags;
  app.playlists = playlists;
}

export async function refreshAll() {
  await guarded(async () => {
    await loadMeta();
    await loadVideos();
  });
}

export function currentVideo(): Video | undefined {
  const id = app.player.video_id;
  // The playing video may be on another library page: fall back to the queue.
  return id == null ? undefined : app.videos.find((v) => v.id === id) ?? app.queueVideos.find((v) => v.id === id);
}

/** Play `ids[index]` and remember `ids` as the queue for next/previous/auto-advance. */
export async function playQueue(videos: Video[], index: number, title: string | null = null) {
  if (index < 0 || index >= videos.length) return;
  const ids = videos.map((v) => v.id);
  app.queue = ids;
  app.queueVideos = videos;
  app.queueTitle = title;
  const ok = await guarded(async () => {
    await api.playVideo(ids[index]);
    return true;
  });
  if (ok) app.playerView = true;
}

export function queueIndex(): number {
  return app.player.video_id == null ? -1 : app.queue.indexOf(app.player.video_id);
}

export async function playNext(delta = 1) {
  const i = queueIndex();
  if (i < 0) return;
  await playQueue(app.queueVideos, i + delta, app.queueTitle);
}

export async function togglePause() {
  await guarded(() => api.setPause(!app.player.paused));
}

export async function stopPlayback() {
  await guarded(() => api.stop());
  app.playerView = false;
}

/** Click a column to sort by it; click again to flip direction. */
export function setSort(key: SortBy) {
  if (app.filter.sort_by === key) {
    app.filter.sort_desc = !app.filter.sort_desc;
  } else {
    app.filter.sort_by = key;
    app.filter.sort_desc = key === 'created' || key === 'size' || key === 'duration' || key === 'rating'; // newest / biggest / longest / best first
  }
}

export async function setRating(v: Video, rating: number) {
  await guarded(async () => {
    await api.setRating(v.id, rating);
    v.rating = rating;
    const q = app.queueVideos.find((x) => x.id === v.id);
    if (q) q.rating = rating;
  });
}

/** Move `draggedId` next to `targetId` in the playlist's full order (works while other filters hide items). */
export async function moveInPlaylist(playlistId: number, draggedId: number, targetId: number, after: boolean) {
  await guarded(async () => {
    const full = await api.listVideos({ playlist_id: playlistId, sort_by: 'default' });
    const ids = full.map((v) => v.id).filter((id) => id !== draggedId);
    const at = ids.indexOf(targetId);
    if (at < 0) return;
    ids.splice(after ? at + 1 : at, 0, draggedId);
    await api.reorderPlaylist(playlistId, ids);
    await loadVideos();
  });
}

export async function removeFromPlaylist(playlistId: number, videoId: number) {
  await guarded(async () => {
    await api.removeFromPlaylist(playlistId, videoId);
    await refreshAll();
  });
}

export async function toggleWatched(v: Video) {
  await guarded(async () => {
    await api.setWatched(v.id, !v.watched);
    v.watched = !v.watched;
    if (app.filter.watched !== null) await loadVideos();
  });
}
