<script lang="ts">
  import type {MetricSumsDto} from '../lib/api';
  import {observedTokenTotal} from '../lib/derive';
  import {t,fmtNumber,i18n} from '../lib/i18n.svelte';
  let {sums,allowObserved=false}:{sums:MetricSumsDto;allowObserved?:boolean}=$props();
  const observed=$derived(allowObserved && sums.total_tokens_known===null
    ? observedTokenTotal(sums.input_total_known,sums.output_total_known) : null);
</script>
{#if observed!==null}
  <span title={t('tokens.observedTotalHint')}>≥ {BigInt(observed).toLocaleString(i18n.locale)}</span>
{:else}
  {fmtNumber(sums.total_tokens_known)}
{/if}
