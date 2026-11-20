<script lang="ts">
  import { api, fmtSize, fmtTime, type GroupStat, type Stats, type VideoBrief } from '../lib/api';
  import { app, guarded } from '../lib/store.svelte';
  import { t, type Key } from '../lib/i18n.svelte';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';

  let stats = $state<Stats | null>(null);
  let loading = $state(false);

  async function load() {
    loading = true;
    const s = await guarded(() => api.libraryStats());
    loading = false;
    if (s) stats = s;
  }
  // Load on open, and again when the library changes under us (profile switch, indexing progress).
  $effect(() => {
    app.profiles.current;
    app.indexing.running;
    load();
  });

  const num = (n: number) => n.toLocaleString();
  const pct = (part: number, whole: number) => (whole > 0 ? Math.round((part / whole) * 100) : 0);

  /** "3d 4h", "12h 30m", "45m". */
  function fmtLong(seconds: number): string {
    const m = Math.round(seconds / 60);
    const d = Math.floor(m / 1440), h = Math.floor((m % 1440) / 60);
    if (d > 0) return t('stats.dur_dh', { d, h });
    if (h > 0) return t('stats.dur_hm', { h, m: m % 60 });
    return t('stats.dur_m', { m });
  }
  const stars = (halves: number) => String((halves + 1) / 2);

  const lengthKeys: Record<string, Key> = {
    lt5: 'stats.len_lt5', '5to20': 'stats.len_5to20', '20to60': 'stats.len_20to60',
    '1to2h': 'stats.len_1to2h', gt2h: 'stats.len_gt2h', unknown: 'stats.len_unknown',
  };
  // Month label from "YYYY-MM", in the app language.
  const monthName = (ym: string) => new Date(Number(ym.slice(0, 4)), Number(ym.slice(5)) - 1, 1).toLocaleDateString(undefined, { month: 'short' });

  const max = (xs: number[]) => Math.max(1, ...xs);
  const groupDetail = (g: GroupStat) =>
    t('stats.group_detail', { watched: g.watched, videos: g.videos, size: fmtSize(g.size), duration: fmtLong(g.duration) });
  const briefValue = {
    largest: (v: VideoBrief) => fmtSize(v.size),
    longest: (v: VideoBrief) => fmtTime(v.duration ?? 0),
    top_rated: (v: VideoBrief) => `${v.rating} ★`,
  };
  const tops = $derived(stats ? ([
    ['stats.largest', stats.largest, briefValue.largest],
    ['stats.top_rated', stats.top_rated, briefValue.top_rated],
    ['stats.longest', stats.longest, briefValue.longest],
  ] as [Key, VideoBrief[], (v: VideoBrief) => string][]) : []);
</script>

<div class="head">
  <div class="grow">
    <h1>{t('stats.title')}</h1>
    <div class="muted">{t('stats.subtitle')}</div>
  </div>
  <button onclick={load} disabled={loading}><RefreshCw size={15} /> {t('stats.refresh')}</button>
</div>

{#if stats && stats.videos === 0}
  <div class="empty muted">{t('stats.empty')}</div>
{:else if stats}
  {@const s = stats}
  {@const ratingPeak = max(s.rating_distribution)}
  {@const lengthPeak = max(s.duration_buckets.map((b) => b.count))}
  {@const monthPeak = max(s.added_by_month.map((m) => m.count))}
  <div class="tiles">
    <div class="card tile">
      <div class="muted label">{t('stats.videos')}</div>
      <div class="big">{num(s.videos)}</div>
      {#if s.hidden > 0}<div class="muted small">{t('stats.hidden_n', { n: num(s.hidden) })}</div>{/if}
    </div>
    <div class="card tile">
      <div class="muted label">{t('stats.watched')}</div>
      <div class="big">{pct(s.watched, s.videos)}%</div>
      <div class="track" title={t('stats.watched_of', { watched: s.watched, total: s.videos, pct: pct(s.watched, s.videos) })}>
        <div class="fill" style:width="{pct(s.watched, s.videos)}%"></div>
      </div>
      <div class="muted small">{t('stats.watched_of', { watched: num(s.watched), total: num(s.videos), pct: pct(s.watched, s.videos) })}</div>
    </div>
    <div class="card tile">
      <div class="muted label">{t('stats.total_duration')}</div>
      <div class="big">{fmtLong(s.total_duration)}</div>
      <div class="muted small">
        {t('stats.watched_duration')}: {fmtLong(s.watched_duration)}{#if s.unindexed > 0} · {t('stats.unindexed', { n: num(s.unindexed) })}{/if}
      </div>
    </div>
    <div class="card tile">
      <div class="muted label">{t('stats.total_size')}</div>
      <div class="big">{fmtSize(s.total_size)}</div>
    </div>
    <div class="card tile">
      <div class="muted label">{t('stats.avg_rating')}</div>
      <div class="big">{s.rated > 0 ? `${s.avg_rating.toFixed(1)} ★` : '—'}</div>
      <div class="muted small">{s.rated > 0 ? t('stats.rated_n', { n: num(s.rated) }) : t('stats.none_rated')}</div>
    </div>
  </div>

  <div class="charts">
    <section class="card half">
      <h2>{t('stats.ratings')}</h2>
      <div class="cols" role="list">
        {#each s.rating_distribution as n, i}
          <div class="col" role="listitem" title="{stars(i)} ★: {num(n)}">
            <span class="val small">{n > 0 ? num(n) : ''}</span>
            <div class="bar" style:height="{(n / ratingPeak) * 100}%"></div>
            <span class="muted small">{stars(i)}</span>
          </div>
        {/each}
      </div>
    </section>

    <section class="card half">
      <h2>{t('stats.lengths')}</h2>
      <div class="rows">
        {#each s.duration_buckets as b}
          <div class="hrow" title="{t(lengthKeys[b.key])}: {num(b.count)}">
            <span class="name">{t(lengthKeys[b.key])}</span>
            <div class="track"><div class="fill" style:width="{(b.count / lengthPeak) * 100}%"></div></div>
            <span class="val">{num(b.count)}</span>
          </div>
        {/each}
      </div>
    </section>

    <section class="card wide">
      <h2>{t('stats.added')}</h2>
      <div class="cols months" role="list">
        {#each s.added_by_month as m}
          <div class="col" role="listitem" title="{m.month}: {num(m.count)}">
            <span class="val small">{m.count > 0 ? num(m.count) : ''}</span>
            <div class="bar" style:height="{(m.count / monthPeak) * 100}%"></div>
            <span class="muted small">{monthName(m.month)}</span>
          </div>
        {/each}
      </div>
    </section>

    {#each [['stats.folders', s.folders], ['stats.tags', s.tags], ['stats.playlists', s.playlists]] as [title, groups] (title)}
      {@const list = groups as GroupStat[]}
      {#if list.length > 0}
        {@const peak = max(list.map((g) => g.videos))}
        <section class="card wide">
          <h2>{t(title as Key)}</h2>
          <div class="rows">
            {#each list as g (g.id)}
              <div class="hrow groupline" title={groupDetail(g)}>
                <span class="name" title={g.name}>
                  {#if g.color}<span class="dot" style:background={g.color}></span>{/if}{g.name}
                </span>
                <div class="track"><div class="fill" style:width="{(g.videos / peak) * 100}%"></div></div>
                <span class="val">{num(g.videos)}</span>
                <span class="muted small detail">{groupDetail(g)}</span>
              </div>
            {/each}
          </div>
        </section>
      {/if}
    {/each}

    {#each tops as [title, list, value] (title)}
      <section class="card" class:half={title === 'stats.top_rated'}>
        <h2>{t(title)}</h2>
        {#each list as v, i (v.id)}
          <div class="toprow"><span class="muted n">{i + 1}</span><span class="name" title={v.title}>{v.title}</span><span class="val">{value(v)}</span></div>
        {:else}<div class="muted">{t('stats.nothing')}</div>{/each}
      </section>
    {/each}
  </div>
{/if}

<style>
  .head { display: flex; align-items: center; gap: 10px; margin-bottom: 16px; }
  .empty { text-align: center; padding: 40px; }
  .tiles { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 14px; margin-bottom: 16px; }
  .tile { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .label { font-size: 12px; text-transform: uppercase; letter-spacing: .04em; }
  .big { font-size: 28px; font-weight: 700; font-variant-numeric: tabular-nums; }
  .small { font-size: 12px; }
  /* Four equal columns: charts take 1/4 (default), 2/4 (.half) or the full row (.wide). */
  .charts { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 14px; }
  .half { grid-column: span 2; }
  .wide { grid-column: 1 / -1; }
  @media (max-width: 900px) { .charts { grid-template-columns: minmax(0, 1fr); } .half, .wide { grid-column: auto; } }
  section h2 { margin-bottom: 14px; }

  /* Bars use the theme color only; values are always printed as text. */
  .track { height: 8px; background: var(--surface-2); border-radius: 4px; overflow: hidden; }
  .fill { height: 100%; background: var(--primary); border-radius: 4px; min-width: 0; }
  .cols { display: flex; align-items: stretch; gap: 6px; height: 170px; }
  .col { flex: 1; min-width: 0; display: flex; flex-direction: column; align-items: center; justify-content: flex-end; gap: 4px; }
  .col .bar { width: 100%; max-width: 40px; min-height: 2px; background: var(--primary); border-radius: 4px 4px 0 0; }
  .col .bar[style*='height: 0%'] { background: var(--surface-2); }
  .val { font-variant-numeric: tabular-nums; }
  .rows { display: flex; flex-direction: column; gap: 8px; }
  .hrow { display: grid; grid-template-columns: minmax(80px, 200px) 1fr 48px; align-items: center; gap: 12px; }
  .hrow .val { text-align: right; }
  .groupline { grid-template-columns: minmax(120px, 260px) 1fr 48px minmax(0, auto); }
  .detail { white-space: nowrap; }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; display: flex; align-items: center; gap: 8px; }
  .dot { width: 10px; height: 10px; border-radius: 50%; flex: none; }
  .toprow { display: grid; grid-template-columns: 20px 1fr auto; gap: 10px; align-items: center; padding: 5px 0; border-bottom: 1px solid var(--border); }
  .toprow:last-child { border-bottom: 0; }
  @media (max-width: 900px) { .detail { display: none; } .groupline { grid-template-columns: minmax(120px, 260px) 1fr 48px; } }
</style>
