/** Spelling only; serving-profile and version evidence belongs to the core. */
export function modelKey(model:string|null):string {
  const key=(model??'').trim().toLowerCase().split(/[\s_-]+/).filter(Boolean).join('-');
  return key.startsWith('claude-') ? key.replaceAll('.','-') : key;
}
