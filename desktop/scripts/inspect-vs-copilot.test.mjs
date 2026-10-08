import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../', import.meta.url));
const probe = spawnSync('pwsh', ['-NoProfile', '-Command', '$PSVersionTable.PSVersion.Major'], { encoding: 'utf8', windowsHide: true });
const available = probe.status === 0;

test('VS inventory queries every edition, major version and preview through vswhere', { skip: !available }, () => {
  const base = path.join(root, 'build', 'vs-copilot-discovery');
  mkdirSync(base, { recursive: true });
  const dir = mkdtempSync(path.join(base, 'inventory-test-'));
  try {
    const instances = [
      ['Community', '17.14.1'], ['Professional', '18.10.1'],
      ['Enterprise', '17.14.41'], ['Enterprise', '18.11.0-preview'],
    ].map(([edition, version], i) => ({
      instanceId: `instance-${i}`, productId: `Microsoft.VisualStudio.Product.${edition}`,
      installationVersion: version, installationPath: path.join(dir, `custom-install-${i}`),
    }));
    const inventory = path.join(dir, 'instances.json');
    const mock = path.join(dir, 'vswhere.ps1');
    writeFileSync(inventory, JSON.stringify(instances));
    writeFileSync(mock, `
if (-not ($args -contains '-all') -or -not ($args -contains '-prerelease') -or
    -not ($args -contains '-products') -or -not ($args -contains '*') -or
    ($args -contains '-latest') -or ($args -contains '-version')) { throw 'restricted query' }
if ($args -contains '-find') { return }
Get-Content -LiteralPath $env:LLM_USAGE_VS_TEST_INVENTORY -Raw
`);
    const result = spawnSync('pwsh', ['-NoProfile', '-File', path.join(root, 'desktop/scripts/inspect-vs-copilot.ps1'), '-VsWherePath', mock], {
      encoding: 'utf8', windowsHide: true, env: {
        ...process.env, TMP: dir, TEMP: dir, TMPDIR: dir, XDG_DATA_HOME: dir,
        LLM_USAGE_VS_TEST_INVENTORY: inventory,
      },
    });
    assert.equal(result.status, 0, result.stderr);
    const report = JSON.parse(result.stdout);
    assert.equal(report.installation_discovery, 'queried');
    assert.deepEqual(report.instances.map(i => [i.product_id, i.installation_version, i.installation_path]),
      instances.map(i => [i.productId, i.installationVersion, i.installationPath]));
    assert.deepEqual(report.warnings, []);
    const emptyCarrier = report.local_carriers.find(carrier =>
      carrier.traces_directory === path.join(dir, 'VSGitHubCopilotLogs', 'traces'));
    assert.equal(emptyCarrier.inspection_status, 'inspected');
    assert.equal(emptyCarrier.jsonl_files_up_to_1024, 0);
    assert.equal(emptyCarrier.bytes, 0);
    assert.ok(report.instances.every(i => i.copilot_components.length === 0));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
