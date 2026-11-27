export function availableAgents(
  catalogAgents: { agent: string }[] | undefined,
  sources: { agent: string; user_id: string }[],
  currentUser: string,
): string[] {
  return Array.from(new Set([
    ...(catalogAgents ?? []).map((a) => a.agent),
    ...sources.filter((s) => s.user_id === currentUser && s.agent === 'zed').map((s) => s.agent),
  ])).sort();
}
