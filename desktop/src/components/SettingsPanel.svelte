<script lang="ts">
  import { api, parseError } from '../lib/api';
  import type { AppSettings, SummaryQuery } from '../lib/api';
  import { t } from '../lib/i18n.svelte';

  let {
    settings,
    onsaved,
  }: {
    settings: AppSettings;
    onsaved: (next: AppSettings) => void;
  } = $props();

  // svelte-ignore state_referenced_locally
  // 草稿编辑器刻意只捕获挂载时的设置初值；外部更新由父组件重新挂载本面板。
  let draft = $state<AppSettings>({ ...settings, manual_roots: [...settings.manual_roots] });
  // svelte-ignore state_referenced_locally
  let retentionInput = $state(settings.retention_days === null ? '' : String(settings.retention_days));
  // svelte-ignore state_referenced_locally
  let rootsText = $state(settings.manual_roots.join('\n'));
  let saving = $state(false);
  let message = $state('');
  let errorMessage = $state('');
  let info = $state<{ db_path: string; host_id: string; schema_version: number } | null>(null);
  let exportMessage = $state('');
  let exportError = $state('');

  // 当前查询（导出用）：近 30 天日粒度。
  const exportQuery: SummaryQuery = {
    first_day: new Date(Date.now() - 29 * 86_400_000).toISOString().slice(0, 10),
    last_day: new Date().toISOString().slice(0, 10),
    granularity: 'day',
    agents: [],
    providers: [],
    models: [],
  };

  async function loadInfo() {
    try {
      info = await api.appInfo();
    } catch {
      info = null;
    }
  }
  loadInfo();

  async function save() {
    saving = true;
    message = '';
    errorMessage = '';
    const retention = retentionInput.trim() === '' ? null : Number(retentionInput);
    if (retention !== null && (!Number.isInteger(retention) || retention < 1)) {
      errorMessage = t('settings.saveFailed', { message: 'retention' });
      saving = false;
      return;
    }
    const next: AppSettings = {
      ...draft,
      retention_days: retention,
      manual_roots: rootsText.split('\n').map((l) => l.trim()).filter(Boolean),
    };
    try {
      await api.setSettings(next);
      message = t('settings.saved');
      onsaved(next);
    } catch (e) {
      errorMessage = t('settings.saveFailed', { message: parseError(e) });
    } finally {
      saving = false;
    }
  }

  async function exportData(kind: 'summary-csv' | 'exchange') {
    exportMessage = '';
    exportError = '';
    try {
      const r = await api.exportData(kind, null, exportQuery);
      exportMessage = t('export.done', { path: r.path });
    } catch (e) {
      exportError = t('export.failed', { message: parseError(e) });
    }
  }
</script>

<h3>{t('settings.title')}</h3>
<form onsubmit={(e) => { e.preventDefault(); void save(); }}>
  <div class="grid">
    <label>{t('settings.timezone')}
      <input bind:value={draft.timezone} placeholder="Asia/Shanghai" />
    </label>
    <label>{t('settings.weekStart')}
      <select bind:value={draft.week_start}>
        <option value={0}>{t('settings.weekStart.monday')}</option>
        <option value={6}>{t('settings.weekStart.sunday')}</option>
      </select>
    </label>
    <label>{t('settings.retention')}
      <input bind:value={retentionInput} inputmode="numeric" placeholder="—" />
    </label>
    <label>{t('settings.interval')}
      <input bind:value={draft.refresh_interval_secs} type="number" min="0" max="86400" />
    </label>
    <label>{t('settings.language')}
      <select bind:value={draft.language}>
        <option value="zh-CN">简体中文</option>
        <option value="en">English</option>
      </select>
    </label>
  </div>
  <label class="roots">{t('settings.manualRoots')}
    <textarea bind:value={rootsText} rows="3" spellcheck="false"></textarea>
  </label>
  <p class="note">{t('settings.note')}</p>
  <button type="submit" disabled={saving}>{t('settings.title')}</button>
  {#if message}<span class="ok">{message}</span>{/if}
  {#if errorMessage}<span class="bad">{errorMessage}</span>{/if}
</form>

{#if info}
  <dl class="meta">
    <dt>{t('settings.host')}</dt>
    <dd class="mono">{info.host_id}</dd>
    <dt>{t('settings.dbPath')}</dt>
    <dd class="mono">{info.db_path}</dd>
    <dt>schema</dt>
    <dd>v{info.schema_version}</dd>
  </dl>
{/if}

<h4>{t('action.export')}</h4>
<div class="export">
  <button onclick={() => void exportData('summary-csv')}>{t('action.exportCsv')}</button>
  <button onclick={() => void exportData('exchange')}>{t('action.exportExchange')}</button>
</div>
{#if exportMessage}<p class="ok">{exportMessage}</p>{/if}
{#if exportError}<p class="bad">{exportError}</p>{/if}

<style>
  h3, h4 {
    font-size: 14px;
    margin: 14px 0 8px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 12px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 13px;
    color: #444;
  }
  input, select, textarea {
    padding: 5px 8px;
    font-size: 13px;
  }
  textarea {
    font-family: ui-monospace, monospace;
  }
  .roots {
    margin-top: 12px;
  }
  .note {
    font-size: 12px;
    color: #888;
  }
  button {
    margin-top: 8px;
    padding: 6px 16px;
    cursor: pointer;
  }
  .ok {
    color: #1e7a3c;
    font-size: 13px;
    margin-left: 10px;
  }
  .bad {
    color: #b3261e;
    font-size: 13px;
    margin-left: 10px;
  }
  .meta {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 4px 12px;
    font-size: 13px;
    background: #f7f8fa;
    border-radius: 8px;
    padding: 10px 14px;
  }
  .meta dt {
    color: #666;
  }
  .mono {
    font-family: ui-monospace, monospace;
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  .export {
    display: flex;
    gap: 10px;
  }
</style>
