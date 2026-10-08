import { execFileSync } from 'node:child_process';

const endpoint = 'repos/owent/llm-usage/pages';
function request(method, body, target = endpoint) {
  const args = ['api', '--method', method, '-H', 'Accept: application/vnd.github+json',
    '-H', 'X-GitHub-Api-Version: 2026-03-10', target];
  if (body) args.push('--input', '-');
  const result = execFileSync('gh', args, { encoding: 'utf8', input: body ? JSON.stringify(body) : undefined,
    timeout: 60000, env: { ...process.env, GH_PROMPT_DISABLED: '1' }, stdio: ['pipe', 'pipe', 'pipe'] }).trim();
  return result ? JSON.parse(result) : null;
}

export function pagesProblems(site) {
  const problems = [];
  if (site.source?.branch !== 'gh-pages' || site.source?.path !== '/') problems.push('Pages source must be gh-pages at /');
  if (site.build_type && site.build_type !== 'legacy') problems.push('Pages must use the branch publishing source');
  if (site.cname !== 'llm-usage.atframe.work') problems.push('Pages custom domain differs from llm-usage.atframe.work');
  return problems;
}

export function configuredPages() {
  const site = request('GET');
  const problems = pagesProblems(site);
  if (problems.length) throw new Error(problems.join('; '));
  return site;
}

export function configurePages() {
  let site;
  try { site = request('GET'); }
  catch (error) {
    if (!String(error.stderr).includes('HTTP 404')) throw error;
  }
  if (site && (site.source?.branch !== 'gh-pages' || site.source?.path !== '/')) {
    throw new Error('Existing Pages source belongs to a different publication; review its settings before changing it');
  }
  if (!site) request('POST', { build_type: 'legacy', source: { branch: 'gh-pages', path: '/' } });
  if (site?.cname !== 'llm-usage.atframe.work' || (site.build_type && site.build_type !== 'legacy')) {
    request('PUT', { cname: 'llm-usage.atframe.work', build_type: 'legacy' });
  }
  return configuredPages();
}

export function configurePublicationEnvironment(api = request) {
  const environment = 'repos/owent/llm-usage/environments/github-pages';
  const settings = api('GET', undefined, environment);
  if (!settings.deployment_branch_policy?.custom_branch_policies) return;
  const policies = `${environment}/deployment-branch-policies`;
  const read = () => {
    const result = api('GET', undefined, `${policies}?per_page=100`);
    if (!Array.isArray(result.branch_policies) || result.total_count !== result.branch_policies.length) {
      throw new Error('Incomplete deployment branch rules; inspect the environment before modifying it');
    }
    return result.branch_policies;
  };
  const previous = read();
  const allowsMain = rules => rules.some(rule => rule.name === 'main' && rule.type !== 'tag');
  if (allowsMain(previous)) return;
  api('POST', { name: 'main', type: 'branch' }, policies);
  const actual = read();
  if (!allowsMain(actual) || previous.some(rule => !actual.some(item =>
    item.id === rule.id && item.name === rule.name && item.type === rule.type))) {
    throw new Error('Deployment branch rule read-back differs; inspect existing rules before retrying');
  }
}
