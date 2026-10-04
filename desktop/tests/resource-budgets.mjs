// Windows all-process private bytes; authority: architecture.md#budgets.
export const idleMeanBudgetBytes=350*1048576;
export const idlePeakBudgetBytes=400*1048576;
export const importPeakBudgetBytes=512*1048576;
export function idleMemoryBudget(resources,{diagnostic=false}={}){
  const qualified=Boolean(resources&&resources.duration_seconds>=600&&resources.sample_count>1&&!diagnostic);
  return {
    metric:'all-process private bytes',
    mean_limit_bytes:idleMeanBudgetBytes,
    peak_limit_bytes:idlePeakBudgetBytes,
    qualified,
    within_budget:qualified?resources.mean_private_bytes<=idleMeanBudgetBytes&&resources.peak_private_bytes<=idlePeakBudgetBytes:null,
  };
}
