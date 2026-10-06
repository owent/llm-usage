// Real deb packages and native GTK/WebKit IPC in a task-owned rootless Podman environment.
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdir, readFile, writeFile, copyFile } from 'node:fs/promises';
import { resolve, join, sep } from 'node:path';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';

const args=process.argv.slice(2);
const option=(name,fallback)=>args.includes(name)?args[args.indexOf(name)+1]:fallback;
if(args.includes('--help')) {
  console.log('Usage: npm run test:install:linux -- --previous-deb PATH --deb PATH [--appimage PATH] [--appimage-mode extract|fuse] [--gui-scale 1|2] [--screen-reader] [--image IMAGE] [--storage-dir build/PATH] [--skip-build-image]\nRequires Linux, rootless Podman and real packages. Builds a pinned Debian GUI image; runs native WebKit/IPC and actual package upgrade/rollback/uninstall/reinstall. All artifacts stay under build/install-lifecycle. Runtime containers have no network or host directory mounts. FUSE mode explicitly adds /dev/fuse and SYS_ADMIN to this container; the GUI runs as an ordinary user with default seccomp. GUI scale changes only this container display and GTK window scaling. --screen-reader adds an actual Orca Tab/Enter navigation check on the reinstalled current package, using Speech Dispatcher/espeak-ng with ALSA null; physical audio is not checked. A reused image must contain those tools.');
  process.exit(0);
}
assert.equal(process.platform,'linux');
assert.ok(process.getuid()>0,'Podman must run rootless');
assert.ok(option('--previous-deb')&&option('--deb'),'Both real previous/current deb packages are required');
const root=resolve('build/install-lifecycle/linux',String(Date.now()));
const storage=resolve(option('--storage-dir','build/install-lifecycle/podman'));
assert.ok(storage.startsWith(resolve('build')+sep),'Dedicated Podman storage must be under repository build/');
const image=option('--image','localhost/llm-usage-lifecycle:debian13');
const previous=resolve(option('--previous-deb')),current=resolve(option('--deb'));
const appimage=option('--appimage')?resolve(option('--appimage')):undefined;
const appimageMode=option('--appimage-mode','extract');
assert.ok(['extract','fuse'].includes(appimageMode));
const guiScale=option('--gui-scale','1');
const screenReader=args.includes('--screen-reader');
assert.ok(['1','2'].includes(guiScale),'GUI scale must be 1 or 2');
await Promise.all([mkdir(root,{recursive:true}),mkdir(storage,{recursive:true})]);
const auth=join(storage,'auth.json');
try { await writeFile(auth,'{"auths":{}}\n',{flag:'wx',mode:0o600}); }
catch(error) {
  if(error.code!=='EEXIST')throw error;
  assert.equal(Object.keys(JSON.parse(await readFile(auth,'utf8')).auths||{}).length,0,'Test registry auth must be empty');
}
const env={...process.env,REGISTRY_AUTH_FILE:auth};
const prefix=['--root',join(storage,'root'),'--runroot',join(storage,'run'),'--cgroup-manager=cgroupfs'];
const operations=[],checks=[];
let container,failure;
async function command(binary,arguments_,timeout=300000) {
  const child=spawn(binary,arguments_,{env,stdio:['ignore','pipe','pipe']});
  const quiet=arguments_.includes('info')||arguments_.includes('inspect');
  let output='',stdout='',stderr='';child.stdout.on('data',v=>{output+=v;stdout+=v;if(!quiet)process.stdout.write(v);});child.stderr.on('data',v=>{output+=v;stderr+=v;if(!quiet)process.stderr.write(v);});
  const timer=setTimeout(()=>child.kill(),timeout);
  let code;
  try { [code]=await once(child,'exit'); } finally { clearTimeout(timer); }
  operations.push({binary,arguments:arguments_,code,output,stdout,stderr});
  assert.equal(code,0,output);return stdout.trim();
}
const podman=(...arguments_)=>command('podman',[...prefix,...arguments_]);
const exec=(...arguments_)=>podman('exec',container,...arguments_);
const destination='/workspace/llm-usage/build/install-lifecycle';
const metadata={};
metadata.gui_scale=Number(guiScale);
metadata.screen_reader=screenReader;
async function fileMetadata(path) {
  const bytes=await readFile(path);return {path,bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex')};
}
try {
  metadata.previous=await fileMetadata(previous);metadata.current=await fileMetadata(current);
  if(appimage)metadata.appimage=await fileMetadata(appimage);
  const info=JSON.parse(await podman('info','--format','json'));
  assert.equal(info.host.security.rootless,true);
  metadata.podman_version=info.version.Version;
  if(!args.includes('--skip-build-image')) {
    const context=join(root,'image');await mkdir(context);
    await copyFile(resolve('desktop/tests/linux-install.Containerfile'),join(context,'Containerfile'));
    await command('podman',[...prefix,'build','--build-arg',`WITH_SCREEN_READER=${screenReader?1:0}`,'-t',image,context],1800000);
  }
  metadata.image=JSON.parse(await podman('image','inspect',image))[0].Id;
  const name=`llmusage-lifecycle-${process.pid}-${Date.now()}`;
  const devices=appimage&&appimageMode==='fuse'?['--device=/dev/fuse','--cap-add=SYS_ADMIN']:[];
  container=await podman('create','--name',name,'--network=none','--shm-size=512m',...devices,image,'bash','-c','trap "exit" TERM; while true; do sleep 1 & wait $!; done');
  await podman('start',container);
  const runtime=JSON.parse(await podman('inspect',container))[0];
  metadata.runtime={rootless:info.host.security.rootless,privileged:runtime.HostConfig.Privileged,capabilities:runtime.HostConfig.CapAdd,security_options:runtime.HostConfig.SecurityOpt,network:runtime.HostConfig.NetworkMode};
  assert.equal(metadata.runtime.privileged,false);
  assert.equal(metadata.runtime.network,'none');
  assert.deepEqual(metadata.runtime.capabilities,devices.length?['CAP_SYS_ADMIN']:[]);
  for(const file of ['linux-install-desktop.py','linux-install-session.sh','linux-screen-reader.py','linux-screen-reader-orca.py','linux-screen-reader-session.sh'])await podman('cp',resolve('desktop/tests',file),`${container}:${destination}/${file}`);
  if(screenReader)metadata.screen_reader_versions=await exec('dpkg-query','-W','-f=${Package} ${Version}\n','orca','speech-dispatcher','speech-dispatcher-espeak-ng','python3-pyatspi');
  await podman('cp',previous,`${container}:${destination}/previous.deb`);
  await podman('cp',current,`${container}:${destination}/current.deb`);
  await exec('chown','-R','acceptance:acceptance','/workspace/llm-usage');
  const packageName=await exec('dpkg-deb','--field',`${destination}/current.deb`,'Package');
  assert.equal(packageName,'llm-usage');
  metadata.previous_version=await exec('dpkg-deb','--field',`${destination}/previous.deb`,'Version');
  metadata.current_version=await exec('dpkg-deb','--field',`${destination}/current.deb`,'Version');
  async function gui(stage,extra=[]) {
    await podman('exec','--user','acceptance','--env',`LLM_USAGE_GUI_SCALE=${guiScale}`,...extra,container,'bash',`${destination}/linux-install-session.sh`);
    await exec('cp',`${destination}/linux-desktop-result.json`,`${destination}/${stage}-result.json`);
    await exec('cp',`${destination}/linux-desktop.png`,`${destination}/${stage}.png`);
    checks.push(`${stage}: native GUI, isolated discovery, IPC refresh, five pages and platform status`);
  }
  async function stage(name,file,version) {
    await exec('dpkg','-i',`${destination}/${file}`);
    assert.equal(await exec('dpkg-query','-W','-f=${Version}',packageName),version);
    await gui(name);
  }
  const retained=['python3','-c','import sqlite3; c=sqlite3.connect("/workspace/llm-usage/build/install-lifecycle/含空格 data/llm-usage.sqlite"); assert c.execute("SELECT COUNT(*),SUM(CAST(total_tokens AS INTEGER)) FROM usage_events").fetchone()==(1,15); print("data retained: one observation, 15 synthetic tokens")'];
  await stage('old','previous.deb',metadata.previous_version);
  await stage('upgrade','current.deb',metadata.current_version);
  await stage('rollback','previous.deb',metadata.previous_version);
  await stage('upgrade-again','current.deb',metadata.current_version);
  await exec('dpkg','--remove',packageName);await exec('test','!','-e','/usr/bin/LLMUsage');await exec(...retained);checks.push('uninstall removes executable and retains data');
  await stage('reinstall','current.deb',metadata.current_version);
  if(screenReader)await gui('screen-reader',['--env','LLM_USAGE_SCREEN_READER=1']);
  await exec('dpkg','--purge',packageName);await exec(...retained);checks.push('purge retains user data');
  if(appimage) {
    await podman('cp',appimage,`${container}:${destination}/LLMUsage.AppImage`);
    await exec('chmod','+x',`${destination}/LLMUsage.AppImage`);
    const extra=['--env',`LLM_USAGE_EXE=${destination}/LLMUsage.AppImage`,'--env',`LLM_USAGE_APPIMAGE_MODE=${appimageMode}`];
    if(appimageMode==='extract')extra.push('--env','APPIMAGE_EXTRACT_AND_RUN=1');
    await gui(`appimage-${appimageMode}`,extra);
    metadata.appimage_mode=appimageMode;
  }
}catch(error){failure=String(error.stack||error);console.error(failure);process.exitCode=1;}
finally {
  if(container) {
    const collected=join(root,'container');await mkdir(collected,{recursive:true});
    try {await podman('cp',`${container}:${destination}/.`,collected);}catch(error){console.error(String(error));process.exitCode=1;}
    try {await podman('stop','-t','5',container);await podman('rm',container);}catch(error){console.error(String(error));process.exitCode=1;}
  }
  await writeFile(join(root,'result.json'),JSON.stringify({root,metadata,checks,operations,failure},null,2));
  console.log(JSON.stringify({root,checks:checks.length,failure}));
}
