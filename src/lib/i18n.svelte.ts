import type { Settings } from './api';
import { en } from './i18n/en';
import { it } from './i18n/it';

export type Key = {
  [K in keyof typeof en]: K extends `${infer B}_${'one' | 'other'}` ? B : K;
}[keyof typeof en];
type Params = Record<string, string | number>;

export type LanguageSetting = 'system' | 'en' | 'it';
export type Lang = 'en' | 'it';
export type DateOrder = 'dmy' | 'mdy' | 'ymd';

/** Languages offered in Settings (names are shown in their own language). */
export const LANGUAGES: { id: Lang; name: string }[] = [
  { id: 'en', name: 'English' },
  { id: 'it', name: 'Italiano' },
];
export const DATE_SEPARATORS = ['/', '-', '.'] as const;

const dicts: Record<Lang, Record<string, string>> = { en, it };

/** Reactive locale state, synced from the settings by `applyI18nSettings`. */
export const i18n = $state({
  lang: 'en' as Lang,
  dateOrder: 'dmy' as DateOrder,
  dateSeparator: '/' as string,
  time12h: false,
  seconds: true,
  timeFirst: true,
});

function systemLanguage(): Lang {
  const l = (typeof navigator !== 'undefined' ? navigator.language : 'en').toLowerCase();
  return LANGUAGES.find((x) => l === x.id || l.startsWith(x.id + '-'))?.id ?? 'en';
}

/** Copy the language and date/time format settings into the reactive state (call after load and on change). */
export function applyI18nSettings(s: Partial<Settings>) {
  const lang = s.language === 'en' || s.language === 'it' ? s.language : systemLanguage();
  i18n.lang = lang;
  if (typeof document !== 'undefined') document.documentElement.lang = lang;
  i18n.dateOrder = s.date_order === 'mdy' || s.date_order === 'ymd' ? s.date_order : 'dmy';
  i18n.dateSeparator = (DATE_SEPARATORS as readonly string[]).includes(s.date_separator ?? '') ? s.date_separator! : '/';
  i18n.time12h = s.time_format === '12h';
  i18n.seconds = s.time_seconds !== 'false';
  i18n.timeFirst = s.date_layout !== 'date_first';
}

function format(raw: string, params?: Params): string {
  return params ? raw.replace(/\{(\w+)\}/g, (m, k: string) => (k in params ? String(params[k]) : m)) : raw;
}

/** Translate `key`. With `params.n` the plural variant (`key_one` / `key_other`) is used. Falls back to English. */
export function t(key: Key, params?: Params): string {
  const dict = dicts[i18n.lang];
  let raw: string | undefined;
  if (params && typeof params.n === 'number') {
    const cat = new Intl.PluralRules(i18n.lang).select(params.n);
    raw = dict[`${key}_${cat}`] ?? dict[`${key}_other`] ?? (en as Record<string, string>)[`${key}_other`];
  }
  raw ??= dict[key] ?? (en as Record<string, string>)[key] ?? key;
  return format(raw, params);
}

const escapeHtml = (s: string) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);

/** Like `t`, for strings rendered with `{@html}`: the (user-provided) params are HTML-escaped. */
export function th(key: Key, params?: Params): string {
  const safe = params && Object.fromEntries(Object.entries(params).map(([k, v]) => [k, typeof v === 'string' ? escapeHtml(v) : v]));
  return t(key, safe);
}

const p2 = (n: number) => String(n).padStart(2, '0');

/** Date + time from unix seconds (local time), laid out as chosen in Settings → Language & formats. Empty for unknown (0). */
export function fmtDateTime(unixSeconds: number): string {
  if (!unixSeconds) return '';
  const d = new Date(unixSeconds * 1000);
  const [dd, mm, yyyy] = [p2(d.getDate()), p2(d.getMonth() + 1), String(d.getFullYear())];
  const parts = i18n.dateOrder === 'mdy' ? [mm, dd, yyyy] : i18n.dateOrder === 'ymd' ? [yyyy, mm, dd] : [dd, mm, yyyy];
  const date = parts.join(i18n.dateSeparator);
  let h = d.getHours();
  let suffix = '';
  if (i18n.time12h) {
    suffix = h < 12 ? ' AM' : ' PM';
    h = h % 12 || 12;
  }
  const time = [p2(h), p2(d.getMinutes()), ...(i18n.seconds ? [p2(d.getSeconds())] : [])].join(':') + suffix;
  return i18n.timeFirst ? `${time} ${date}` : `${date} ${time}`;
}
