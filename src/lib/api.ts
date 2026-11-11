import { invoke } from '@tauri-apps/api/core';

export interface Folder { id: number; path: string; video_count: number; is_default: boolean }
export interface VideoTag { id: number; name: string; color: string }
export interface Tag extends VideoTag { video_count: number }
export interface Playlist { id: number; name: string; video_count: number }
export interface Video {
  id: number; path: string; folder_id: number; title: string;
  size: number; modified: number; watched: boolean;
  tags: VideoTag[]; playlist_ids: number[];
}
export interface VideoFilter {
  search?: string; tag_ids?: number[]; playlist_id?: number | null;
  watched?: boolean | null; folder_id?: number | null;
}
export interface ScanReport { added: number; removed: number; total: number; rule_matches: number }
export interface Rule {
  id: number; name: string; pattern: string; case_insensitive: boolean; enabled: boolean;
  tag_ids: number[]; playlist_ids: number[];
}
export type RuleInput = Omit<Rule, 'id'>;
export interface PlayerState {
  video_id: number | null; loaded: boolean; idle: boolean; paused: boolean;
  position: number; duration: number; volume: number; muted: boolean;
}
export type Settings = {
  volume_max: string; volume: string; primary_color: string; embed_video: string; mpv_path: string;
  library_view: string; default_folder_id: string;
};

export const api = {
  listFolders: () => invoke<Folder[]>('list_folders'),
  addFolder: (path: string) => invoke<Folder>('add_folder', { path }),
  removeFolder: (id: number) => invoke<void>('remove_folder', { id }),
  setDefaultFolder: (id: number | null) => invoke<void>('set_default_folder', { id }),
  databasePath: () => invoke<string>('database_path'),

  listRules: () => invoke<Rule[]>('list_rules'),
  createRule: (input: RuleInput) => invoke<Rule>('create_rule', { input }),
  updateRule: (id: number, input: RuleInput) => invoke<void>('update_rule', { id, input }),
  deleteRule: (id: number) => invoke<void>('delete_rule', { id }),
  applyRulesToExisting: () => invoke<number>('apply_rules_to_existing'),
  countPatternMatches: (pattern: string, caseInsensitive: boolean) =>
    invoke<number>('count_pattern_matches', { pattern, caseInsensitive }),
  rescanAll: () => invoke<[ScanReport, string[]]>('rescan_all'),

  listVideos: (filter: VideoFilter) => invoke<Video[]>('list_videos', { filter }),
  setWatched: (id: number, watched: boolean) => invoke<void>('set_watched', { id, watched }),

  listTags: () => invoke<Tag[]>('list_tags'),
  createTag: (name: string, color: string) => invoke<Tag>('create_tag', { name, color }),
  updateTag: (id: number, name: string, color: string) => invoke<void>('update_tag', { id, name, color }),
  deleteTag: (id: number) => invoke<void>('delete_tag', { id }),
  setVideoTag: (videoId: number, tagId: number, on: boolean) =>
    invoke<void>('set_video_tag', { videoId, tagId, on }),

  listPlaylists: () => invoke<Playlist[]>('list_playlists'),
  createPlaylist: (name: string) => invoke<Playlist>('create_playlist', { name }),
  renamePlaylist: (id: number, name: string) => invoke<void>('rename_playlist', { id, name }),
  deletePlaylist: (id: number) => invoke<void>('delete_playlist', { id }),
  addToPlaylist: (playlistId: number, videoId: number) =>
    invoke<void>('add_to_playlist', { playlistId, videoId }),
  removeFromPlaylist: (playlistId: number, videoId: number) =>
    invoke<void>('remove_from_playlist', { playlistId, videoId }),
  reorderPlaylist: (playlistId: number, orderedIds: number[]) =>
    invoke<void>('reorder_playlist', { playlistId, orderedIds }),

  getSettings: () => invoke<Settings>('get_settings'),
  setSetting: (key: keyof Settings, value: string) => invoke<void>('set_setting', { key, value }),

  playVideo: (id: number) => invoke<void>('play_video', { id }),
  playerState: () => invoke<PlayerState>('player_state'),
  setPause: (paused: boolean) => invoke<void>('player_set_pause', { paused }),
  seek: (seconds: number, relative: boolean) => invoke<void>('player_seek', { seconds, relative }),
  setVolume: (volume: number) => invoke<void>('player_set_volume', { volume }),
  setMute: (muted: boolean) => invoke<void>('player_set_mute', { muted }),
  stop: () => invoke<void>('player_stop'),

  setFullscreen: (on: boolean) => invoke<void>('set_fullscreen', { on }),
  isFullscreen: () => invoke<boolean>('is_fullscreen'),
};

export function errMsg(e: unknown): string {
  return typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e);
}

export function fmtTime(s: number): string {
  if (!isFinite(s) || s < 0) s = 0;
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), sec = Math.floor(s % 60);
  const mm = String(m).padStart(h ? 2 : 1, '0'), ss = String(sec).padStart(2, '0');
  return h ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}

export function fmtSize(bytes: number): string {
  const u = ['B', 'KB', 'MB', 'GB', 'TB'];
  let i = 0, v = bytes;
  while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
  return `${v.toFixed(i ? 1 : 0)} ${u[i]}`;
}
