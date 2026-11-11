import { api, errMsg, type Folder, type Playlist, type PlayerState, type Settings, type Tag, type Video } from './api';

export type Page = 'library' | 'folders' | 'collections' | 'settings';

export const DEFAULT_PRIMARY = '#d4a017';

export const app = $state({
  page: 'library' as Page,
  playerView: false,
  settings: { volume_max: '300', volume: '100', primary_color: DEFAULT_PRIMARY, embed_video: 'true', mpv_path: '' } as Settings,
  folders: [] as Folder[],
  tags: [] as Tag[],
  playlists: [] as Playlist[],
  videos: [] as Video[],
  player: {
    video_id: null, loaded: false, idle: true, paused: false, position: 0, duration: 0, volume: 100, muted: false,
  } as PlayerState,
  queue: [] as number[],
  toast: null as { text: string; error: boolean } | null,
  filter: {
    search: '',
    watched: null as boolean | null,
    tag_ids: [] as number[],
    playlist_id: null as number | null,
    folder_id: null as number | null,
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

export async function loadVideos() {
  const f = app.filter;
  const videos = await guarded(() =>
    api.listVideos({
      search: f.search,
      watched: f.watched,
      tag_ids: [...f.tag_ids],
      playlist_id: f.playlist_id,
      folder_id: f.folder_id,
    }),
  );
  if (videos) app.videos = videos;
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
  return id == null ? undefined : app.videos.find((v) => v.id === id);
}

/** Play `ids[index]` and remember `ids` as the queue for next/previous/auto-advance. */
export async function playQueue(ids: number[], index: number) {
  if (index < 0 || index >= ids.length) return;
  app.queue = ids;
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
  await playQueue(app.queue, i + delta);
}

export async function togglePause() {
  await guarded(() => api.setPause(!app.player.paused));
}

export async function stopPlayback() {
  await guarded(() => api.stop());
  app.playerView = false;
}

export async function toggleWatched(v: Video) {
  await guarded(async () => {
    await api.setWatched(v.id, !v.watched);
    v.watched = !v.watched;
    if (app.filter.watched !== null) await loadVideos();
  });
}
