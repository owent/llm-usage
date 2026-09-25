<script lang="ts">
  import type { SourceDto } from '../lib/api';
  import { api, parseError } from '../lib/api';
  import { t, fmtRelative } from '../lib/i18n.svelte';

  let {
    sources,
    onchanged,
  }: {
    sources: SourceDto[];
    onchanged: () => void;
  } = $props();

  let error = $state('');

  async function toggle(s: SourceDto) {
    error = '';
    try {
      await api.setSourceEnabled(s.instance_id, !s.enabled);
      onchanged();
    } catch (e) {
      error = parseError(e);
    }
  }
</script>

<h3>{t('sources.title')}</h3>
{#if error}
  <p class="error">{t('common.error', { message: error })}</p>
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
    font-size: 13px;
  }
  th,
  td {
    text-align: left;
    padding: 6px 8px;
    border-bottom: 1px solid #eee;
  }
  th {
    color: #666;
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
    color: #999;
  }
  .tag {
    display: inline-block;
    border-radius: 10px;
    padding: 1px 8px;
    font-size: 12px;
    margin-right: 6px;
  }
  .compat {
    background: #fff4d6;
    color: #8a6d1a;
  }
  .bad {
    background: #fdecea;
    color: #b3261e;
  }
  .error {
    color: #b3261e;
    background: #fdecea;
    border-radius: 6px;
    padding: 6px 10px;
    font-size: 13px;
  }
  .empty {
    color: #666;
    background: #f5f6f7;
    border-radius: 8px;
    padding: 24px;
    text-align: center;
  }
  button {
    padding: 3px 10px;
    cursor: pointer;
  }
</style>
