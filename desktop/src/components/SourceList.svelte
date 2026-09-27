<script lang="ts">
  import type { SourceDto, UserDto } from '../lib/api';
  import { api, parseError } from '../lib/api';
  import { t, fmtRelative } from '../lib/i18n.svelte';

  let {
    sources,
    users,
    onchanged,
  }: {
    sources: SourceDto[];
    users: UserDto[];
    onchanged: () => void;
  } = $props();

  let error = $state('');
  // list_sources 未返回每实例的当前归属（后端合同），下拉初始显示占位；
  // 本地记录最近一次选择，避免刷新后立即回落。
  let assigned = $state<Record<string, string>>({});

  async function toggle(s: SourceDto) {
    error = '';
    try {
      await api.setSourceEnabled(s.instance_id, !s.enabled);
      onchanged();
    } catch (e) {
      error = parseError(e);
    }
  }

  async function assign(s: SourceDto, userId: string) {
    if (!userId) return;
    error = '';
    try {
      await api.assignSourceUser(s.instance_id, userId);
      assigned[s.instance_id] = userId;
      onchanged();
    } catch (e) {
      error = t('sources.assignFailed', { message: parseError(e) });
    }
  }
</script>

<h3>{t('sources.title')}</h3>
{#if error}
  <p class="error">{error}</p>
{/if}
{#if sources.length === 0}
  <p class="empty">{t('sources.none')}</p>
{:else}
  <table>
    <thead>
      <tr>
        <th>{t('sources.agent')}</th>
        <th>{t('sources.instance')}</th>
        <th>{t('sources.health')}</th>
        <th>{t('sources.lastSuccess')}</th>
        <th>{t('sources.status')}</th>
        <th>{t('sources.user')}</th>
        <th></th>
      </tr>
    </thead>
    <tbody>
      {#each sources as s (s.instance_id)}
        <tr class:muted={!s.enabled}>
          <td>{s.agent}</td>
          <td class="mono">{s.instance_id}</td>
          <td>{s.health}</td>
          <td>{fmtRelative(s.last_success_ms)}</td>
          <td>
            {#if s.compat_files > 0}
              <span class="tag compat">{t('sources.compatFiles', { count: s.compat_files })}</span>
            {/if}
            {#if s.incompatible_files > 0}
              <span class="tag bad">{t('sources.incompatibleFiles', { count: s.incompatible_files })}</span>
            {/if}
            {#if s.missing_files > 0}
              <span class="tag bad" title={t('sources.missingFiles.hint')}>{t('sources.missingFiles', { count: s.missing_files })}</span>
            {/if}
          </td>
          <td>
            <select
              value={assigned[s.instance_id] ?? s.user_id}
              onchange={(e) => void assign(s, e.currentTarget.value)}
            >
              <option value="">{t('sources.userPlaceholder')}</option>
              {#each users as u (u.user_id)}
                <option value={u.user_id}>{u.name}</option>
              {/each}
            </select>
          </td>
          <td>
            <button onclick={() => toggle(s)}>
              {s.enabled ? t('sources.enabled') : t('sources.disabled')}
            </button>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<style>
  h3 {
    font-size: 14px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12.5px;
  }
  th,
  td {
    text-align: left;
    padding: 4px 6px;
    border-bottom: 1px solid var(--border-light);
    vertical-align: middle;
  }
  th {
    color: var(--text-secondary);
    font-weight: 500;
  }
  .mono {
    font-family: ui-monospace, monospace;
    font-size: 12px;
    max-width: 380px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .muted {
    color: var(--text-muted);
  }
  .tag {
    display: inline-block;
    border-radius: 10px;
    padding: 1px 8px;
    font-size: 12px;
    margin-right: 6px;
  }
  .compat {
    background: var(--warning-bg);
    color: var(--warning);
  }
  .bad {
    background: var(--danger-bg);
    color: var(--danger);
  }
  .error {
    color: var(--danger);
    background: var(--danger-bg);
    border-radius: 6px;
    padding: 6px 10px;
    font-size: 13px;
  }
  .empty {
    color: var(--text-muted);
    background: var(--bg-hover);
    border-radius: 8px;
    padding: 24px;
    text-align: center;
  }
  button {
    padding: 3px 10px;
    cursor: pointer;
  }
  td select {
    max-width: 150px;
    font-size: 12.5px;
    padding: 2px 4px;
  }
</style>
