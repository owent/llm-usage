<script lang="ts">
  import type { SourceDto, UserDto } from '../lib/api';
  import { api, parseError } from '../lib/api';
  import { t, fmtRelative, fmtDurationShort, i18n } from '../lib/i18n.svelte';
  import Icon from './Icon.svelte';
  let { sources, users, onchanged }: {
    sources: SourceDto[]; users: UserDto[]; onchanged: () => void | Promise<void>;
  } = $props();
  let error = $state('');
  let search = $state('');
  let pending = $state<Record<string, boolean>>({});
  const visible = $derived(sources.filter((s) =>
    `${s.agent} ${s.instance_id}`.toLowerCase().includes(search.toLowerCase())));
  async function change(s: SourceDto, action: () => Promise<unknown>) {
    if (pending[s.instance_id]) return;
    pending[s.instance_id] = true;
    error = '';
    try { await action(); await onchanged(); }
    catch (e) { error = parseError(e); }
    finally { pending[s.instance_id] = false; }
  }

  // ---- 逐源提取计划（M6：interval 预设 / 每日 / 每周；空=继承全局） ----
  const INTERVAL_PRESETS = [15, 60, 300, 900, 1800, 3600, 10800, 21600, 43200, 86400];
  /** 下拉值：'inherit' | 'sec:<n>' | 'daily' | 'weekly'。 */
  function scheduleMode(s: SourceDto): string {
    const rule = s.schedule;
    if (!rule) return 'inherit';
    if (rule.kind === 'interval') return 'sec:' + (rule.intervalSeconds ?? 3600);
    return rule.kind;
  }
  function scheduleTime(s: SourceDto): string {
    return s.schedule?.timeOfDay ?? '09:00';
  }
  function scheduleWeekday(s: SourceDto): number {
    return s.schedule?.weekday ?? 1;
  }
  async function applySchedule(
    s: SourceDto,
    mode: string,
    time: string,
    weekday: number,
  ) {
    const rule =
      mode === 'inherit'
        ? null
        : mode === 'daily'
          ? { kind: 'daily' as const, timeOfDay: time }
          : mode === 'weekly'
            ? { kind: 'weekly' as const, timeOfDay: time, weekday }
            : { kind: 'interval' as const, intervalSeconds: Number(mode.slice(4)) };
    await change(s, () => api.setSourceSchedule(s.instance_id, rule));
  }
  // 周名按当前语言 Intl 生成（周一=1…周日=7，与后端 ISO 对齐；基准 2024-01-01）。
  // 渲染必须固定 UTC：本地时区渲染 UTC 零点会在负偏移时区错一天。
  const weekdayNames = $derived(
    Array.from({ length: 7 }, (_, i) =>
      new Intl.DateTimeFormat(i18n.locale, { weekday: 'short', timeZone: 'UTC' }).format(
        new Date(Date.UTC(2024, 0, 1 + i)),
      ),
    ),
  );
</script>

<div class="source-toolbar">
  <div><h2>{t('sources.title')}</h2><p>{t('sources.localHint')}</p></div>
  <input type="search" bind:value={search} aria-label={t('sources.search')} placeholder={t('sources.search')} />
</div>
{#if error}<p class="error" role="alert">{error}</p>{/if}
<div class="source-grid">
  {#each visible as s (s.instance_id)}
    <section class="source-card" class:paused={!s.enabled}>
      <div class="source-head">
        <span class="source-icon"><Icon name="sources" size={22} /></span>
        <div><h3>{s.agent}</h3><span class="health" class:attention={s.health !== 'ok'}>{t('sources.health.' + s.health)}</span></div>
        <button class="toggle" class:on={s.enabled} role="switch" aria-checked={s.enabled}
          aria-label={`${s.agent}: ${t('sources.enabled')}`}
          disabled={pending[s.instance_id]}
          onclick={() => change(s, () => api.setSourceEnabled(s.instance_id, !s.enabled))}><span></span></button>
      </div>
      <p class="instance" title={s.instance_id}>{s.instance_id}</p>
      <div class="tags">
        {#if s.compat_files > 0}<span class="tag compat">{t('sources.compatFiles', { count: s.compat_files })}</span>{/if}
        {#if s.degraded_files > 0}<span class="tag bad">{t('sources.degradedFiles', { count: s.degraded_files })}</span>{/if}
        {#if s.unsupported_files > 0}<span class="tag bad">{t('sources.unsupportedFiles', { count: s.unsupported_files })}</span>{/if}
        {#if s.incompatible_files > 0}<span class="tag bad">{t('sources.incompatibleFiles', { count: s.incompatible_files })}</span>{/if}
        {#if s.missing_files > 0}<span class="tag compat" title={t('sources.missingFiles.hint')}>{t('sources.missingFiles', { count: s.missing_files })}</span>{/if}
      </div>
      <div class="source-meta"><span>{t('sources.lastSuccess')}</span><strong>{fmtRelative(s.last_success_ms)}</strong></div>
      <div class="schedule-row">
        <label>{t('sources.schedule')}
          <select data-testid="schedule-mode" disabled={pending[s.instance_id]}
            aria-label={`${s.agent}: ${t('sources.schedule')}`}
            onchange={(e) => {
              const mode = e.currentTarget.value;
              void applySchedule(s, mode, scheduleTime(s), scheduleWeekday(s));
            }}>
            <option value="inherit" selected={!s.schedule}>{t('sources.schedule.inherit')}</option>
            {#each INTERVAL_PRESETS as secs (secs)}
              <option value={'sec:' + secs} selected={scheduleMode(s) === 'sec:' + secs}>
                {fmtDurationShort(secs * 1000)}
              </option>
            {/each}
            <option value="daily" selected={scheduleMode(s) === 'daily'}>{t('sources.schedule.daily')}</option>
            <option value="weekly" selected={scheduleMode(s) === 'weekly'}>{t('sources.schedule.weekly')}</option>
          </select>
        </label>
        {#if scheduleMode(s) === 'daily' || scheduleMode(s) === 'weekly'}
          <label>{t('sources.schedule.time')}
            <input type="time" value={scheduleTime(s)} disabled={pending[s.instance_id]}
              onchange={(e) => {
                const value = e.currentTarget.value || '09:00';
                void applySchedule(s, scheduleMode(s), value, scheduleWeekday(s));
              }} />
          </label>
          {#if scheduleMode(s) === 'weekly'}
            <select data-testid="schedule-weekday" disabled={pending[s.instance_id]}
              onchange={(e) => {
                const value = Number(e.currentTarget.value);
                void applySchedule(s, scheduleMode(s), scheduleTime(s), value);
              }}>
              {#each weekdayNames as name, i (i)}
                <option value={i + 1} selected={scheduleWeekday(s) === i + 1}>{name}</option>
              {/each}
            </select>
          {/if}
        {/if}
        {#if s.schedule?.nextDueMs}
          <span class="next-due">{t('sources.schedule.nextDue')} {fmtRelative(s.schedule.nextDueMs)}</span>
        {/if}
      </div>
      <div class="source-footer">
        <label>{t('sources.user')}
          <select value={s.user_id} disabled={pending[s.instance_id]} aria-label={`${s.agent}: ${t('sources.user')}`}
            onchange={(e) => { const value = e.currentTarget.value; void change(s, () => api.assignSourceUser(s.instance_id, value)); }}>
            {#each users as u (u.user_id)}<option value={u.user_id}>{u.name}</option>{/each}
          </select>
        </label>
        <span>{s.enabled ? t('sources.enabled') : t('sources.disabled')}</span>
      </div>
    </section>
  {:else}
    <p class="empty">{sources.length ? t('sources.noMatch') : t('sources.none')}</p>
  {/each}
</div>

<style>
  .source-toolbar { display: flex; gap: 20px; align-items: center; justify-content: space-between; margin: 24px 0; flex-wrap: wrap; }
  h2 { margin: 0; color: var(--text-heading); font-size: 18px; }
  .source-toolbar p { margin: 6px 0 0; color: var(--text-secondary); font-size: 13px; }
  .source-toolbar input { width: 260px; }
  .source-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 340px), 1fr)); gap: 20px; }
  .source-card { padding: 22px; border: 1px solid var(--border); border-radius: 16px; background: var(--bg-card); box-shadow: var(--shadow); min-width: 0; }
  .source-card.paused { background: var(--bg-card-hover); }
  .source-head { display: flex; align-items: center; gap: 12px; }
  .source-icon { display: grid; place-items: center; width: 44px; height: 44px; border-radius: 12px; background: var(--accent-bg); color: var(--accent); }
  h3 { font-size: 16px; margin: 0 0 3px; color: var(--text-heading); }
  .health { color: var(--success); font-size: 12px; }
  .health.attention { color: var(--warning); }
  .toggle { margin-left: auto; width: 40px; height: 24px; border: 0; border-radius: 20px; padding: 3px; background: var(--scrollbar); }
  .toggle span { display: block; width: 18px; height: 18px; border-radius: 50%; background: white; transition: transform 120ms; }
  .toggle.on { background: var(--accent); }
  .toggle.on span { transform: translateX(16px); }
  .instance { font-family: ui-monospace, monospace; font-size: 11px; color: var(--text-muted); padding: 11px 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin: 10px 0 0; }
  .tags { display: flex; gap: 6px; flex-wrap: wrap; min-height: 24px; }
  .tag { border-radius: 6px; padding: 3px 7px; font-size: 11px; }
  .compat { color: var(--accent); background: var(--accent-bg); }
  .bad, .error { color: var(--danger); background: var(--danger-bg); }
  .source-meta { display: flex; justify-content: space-between; font-size: 12px; color: var(--text-secondary); padding: 18px 0; }
  .source-meta strong { font-weight: 500; }
  .schedule-row { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; border-top: 1px solid var(--border-light); padding-top: 12px; margin-top: 4px; font-size: 12px; color: var(--text-muted); }
  .schedule-row input[type='time'] { font-size: 12px; }
  .next-due { color: var(--text-secondary); }
  .source-footer { display: flex; justify-content: space-between; align-items: center; border-top: 1px solid var(--border-light); padding-top: 16px; font-size: 12px; color: var(--text-muted); gap: 12px; }
  label { display: flex; align-items: center; gap: 10px; }
  select { max-width: 150px; font-size: 12px; }
  .empty, .error { padding: 24px; border-radius: 12px; grid-column: 1 / -1; }
  .empty { text-align: center; color: var(--text-secondary); background: var(--bg-card); border: 1px dashed var(--border); }
</style>
