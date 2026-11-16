/** Comparison keys and display labels stay separate from core reference identities. */
import displayAliases from './model-display-aliases.json' with { type: 'json' };

export function modelKey(model:string|null):string {
  const key=(model??'').trim().toLowerCase().split(/[\s_-]+/).filter(Boolean).join('-');
  return key.startsWith('claude-') ? key.replaceAll('.','-') : key;
}

/** Current display overrides leave stored identities, filters and prices unchanged. */
export function modelDisplayName(model: string | null, agent?: string): string | null {
  if (model === null) return null;
  const rules = displayAliases.aliases.filter(rule => modelKey(rule.key) === modelKey(model)
    && (agent === undefined || agent.toLowerCase() === rule.agent));
  return rules.length === 1 ? rules[0].display_name : model;
}

/** Format model/agent-model legends and metric suffixes without changing series keys. */
export function modelSeriesDisplayName(series: string): string {
  const [head, ...suffix] = series.split(' · ');
  const slash = head.indexOf('/');
  const name = slash < 0 ? modelDisplayName(head)!
    : `${head.slice(0, slash)}/${modelDisplayName(head.slice(slash + 1), head.slice(0, slash))}`;
  return [name, ...suffix].join(' · ');
}

type ModelLabelRow = { model: string | null; provider?: string | null; display_model?: string | null };

/** Prefer scoped server labels, including an intentional raw label on ambiguity. */
export function modelRowDisplayName(row: ModelLabelRow): string | null {
  return row.display_model !== undefined ? row.display_model : modelDisplayName(row.model);
}

/** Merged names need agreement across every contributing provider/Agent scope. */
export function modelLabelFromRows(rows: ModelLabelRow[] | undefined, raw: string, provider?: string | null): string {
  if (rows === undefined) return modelDisplayName(raw)!;
  const matches = rows.filter(row => row.model !== null && modelKey(row.model) === modelKey(raw)
    && (provider === undefined || (row.provider ?? '') === (provider ?? '')));
  const names = new Set(matches.map(row => modelRowDisplayName(row) ?? raw));
  return names.size === 1 ? [...names][0] : raw;
}
