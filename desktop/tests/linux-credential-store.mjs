// Root cwd. Uses a private session bus and disposable encrypted keyring only.
import { spawn, spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';
import { randomBytes } from 'node:crypto';

if (process.argv.includes('--help')) {
  console.log('npm run test:credentials:linux — Linux; requires cargo, dbus-run-session and gnome-keyring-daemon. Creates disposable data under build/, performs native tests, removes its own keyring.');
  process.exit(0);
}
if (process.platform !== 'linux') throw new Error('Linux native credential validation requires Linux');
const manifest = 'desktop/src-tauri/Cargo.toml';
const cargo = (name, env) => {
  const result = spawnSync('cargo', ['test', '--manifest-path', manifest, '-p', 'llm-usage-desktop', '--locked', `receiver_auth::tests::${name}`, '--', '--exact', '--ignored'], { env, encoding: 'utf8', timeout: 120_000 });
  if (result.error) throw result.error;
  process.stdout.write(result.stdout);
  process.stderr.write(result.stderr);
  if (result.status !== 0) throw new Error(`${name} failed (${result.status})`);
};

if (process.argv.includes('--inside-bus')) {
  const keyringRoot = process.env.LLM_USAGE_VAULT_ROOT;
  if (!keyringRoot?.startsWith(resolve('build/native-secret-service') + '/')) throw new Error('missing isolated vault root');
  const env = { ...process.env, XDG_DATA_HOME: keyringRoot, GNOME_KEYRING_CONTROL: `${keyringRoot}/control`, LLM_USAGE_ISOLATED_VAULT: '1' };
  // A random password is fed through a pipe, never printed or passed in argv.
  const password = randomBytes(32).toString('hex');
  const daemon = spawn('gnome-keyring-daemon', ['--foreground', '--components=secrets', '--unlock', `--control-directory=${env.GNOME_KEYRING_CONTROL}`], { env, stdio: ['pipe', 'ignore', 'pipe'] });
  daemon.stderr.resume();
  daemon.stdin.end(password);
  try {
    let ready = false;
    for (let attempt = 0; attempt < 60; attempt++) {
      // Query the bus itself first: probing secrets before our daemon owns
      // the name can activate a second service with the bus's environment.
      const owner = spawnSync('dbus-send', ['--session', '--print-reply', '--dest=org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus.NameHasOwner', 'string:org.freedesktop.secrets'], { env, encoding: 'utf8', timeout: 2000 });
      if (owner.status === 0 && owner.stdout.includes('boolean true')) {
        const probe = spawnSync('dbus-send', ['--session', '--print-reply', '--dest=org.freedesktop.secrets', '/org/freedesktop/secrets', 'org.freedesktop.Secret.Service.ReadAlias', 'string:default'], { env, encoding: 'utf8', timeout: 2000 });
        if (probe.status === 0 && probe.stdout.includes('/collection/')) { ready = true; break; }
      }
      if (daemon.exitCode !== null) throw new Error('isolated keyring daemon exited');
      await new Promise(r => setTimeout(r, 100));
    }
    if (!ready) throw new Error('isolated keyring did not become ready');
    cargo('native_credential_store_roundtrip', env);
    cargo('native_credential_http_revocation', env);
    cargo('native_linux_ambiguous_vault', env);
    const alias = name => {
      const result = spawnSync('dbus-send', ['--session', '--print-reply', '--dest=org.freedesktop.secrets', '/org/freedesktop/secrets', 'org.freedesktop.Secret.Service.ReadAlias', `string:${name}`], { env, encoding: 'utf8', timeout: 2000 });
      const path = result.stdout?.match(/object path "([^"\n]+)"/)?.[1];
      if (result.status !== 0 || !path || path === '/') throw new Error('isolated collection alias unavailable');
      return path;
    };
    const setDefault = path => {
      const result = spawnSync('dbus-send', ['--session', '--print-reply', '--dest=org.freedesktop.secrets', '/org/freedesktop/secrets', 'org.freedesktop.Secret.Service.SetAlias', 'string:default', `objpath:${path}`], { env, encoding: 'utf8', timeout: 2000 });
      if (result.status !== 0) throw new Error('isolated default alias update failed');
    };
    const persisted = alias('default');
    try {
      setDefault(alias('session'));
      cargo('native_linux_vault_unavailable', env);
    } finally {
      setDefault(persisted);
    }
    cargo('native_linux_locked_vault', env);
  } finally {
    daemon.kill('SIGTERM');
    if (daemon.exitCode === null) await new Promise(r => daemon.once('exit', r));
  }
} else {
  mkdirSync('build/native-secret-service', { recursive: true });
  const keyringRoot = mkdtempSync(resolve('build/native-secret-service/vault-'));
  try {
    cargo('native_linux_vault_unavailable', { ...process.env, DBUS_SESSION_BUS_ADDRESS: `unix:path=${keyringRoot}/missing-bus` });
    const result = spawnSync('dbus-run-session', ['--', process.execPath, resolve('desktop/tests/linux-credential-store.mjs'), '--inside-bus'], {
      env: { ...process.env, LLM_USAGE_VAULT_ROOT: keyringRoot, XDG_DATA_HOME: keyringRoot, XDG_CONFIG_HOME: `${keyringRoot}/config`, GNOME_KEYRING_CONTROL: `${keyringRoot}/control` }, stdio: 'inherit', timeout: 180_000,
    });
    if (result.error) throw result.error;
    if (result.status !== 0) throw new Error(`isolated keyring validation failed (${result.status})`);
  } finally {
    rmSync(keyringRoot, { recursive: true, force: true });
  }
  console.log('Linux native credentials: unavailable, persisted cross-process read/revoke, real HTTP revocation/source isolation, duplicate/session/default-session rejection and locked vault passed; disposable keyring removed.');
}
