<script lang="ts">
  import type {MetricSumsDto} from '../lib/api';
  import {tokenDisplay,formatTokenDisplay,type TokenMetric} from '../lib/derive';
  import {t,fmtNumber} from '../lib/i18n.svelte';
  let {sums,metric='total'}:{sums:MetricSumsDto;metric?:TokenMetric}=$props();
  const value=$derived(tokenDisplay(sums,metric));
  const count=$derived(metric==='input'?sums.input_unknown_count:metric==='output'?sums.output_unknown_count:sums.total_unknown_count);
  const hint=$derived([value.lower?t('tokens.knownBoundHint'):'',count>0?t('tokens.missingRecords',{count}):''].filter(Boolean).join(' '));
</script>
<span title={hint}>{formatTokenDisplay(value,fmtNumber)}</span>
