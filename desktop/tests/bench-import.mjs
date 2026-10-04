// Current release ingestion pipeline, with an independently sampled process tree.
import {spawn} from 'node:child_process';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import {once} from 'node:events';
import assert from 'node:assert/strict';
if(process.argv.includes('--help')){console.log('node desktop/tests/bench-import.mjs (Windows, new build/ directory, 1,000,000 synthetic events)');process.exit(0);}
assert.equal(process.platform,'win32');
const root=resolve('build/plan-execution/import',String(Date.now()));await mkdir(root,{recursive:true});
const child=spawn(resolve('desktop/src-tauri/target/release/examples/bench_v20.exe'),[root,'1000000'],{stdio:['ignore','pipe','pipe'],windowsHide:true});
let output='';child.stdout.on('data',v=>output+=v);child.stderr.on('data',v=>output+=v);
const completed=once(child,'exit');
const script=await readFile(resolve('desktop/tests/process-resources.ps1'),'utf8');
const sampler=spawn(join(process.env.SystemRoot,'System32/WindowsPowerShell/v1.0/powershell.exe'),['-NoProfile','-NonInteractive','-Command',`& {\n${script}\n} -RootPid ${child.pid} -Seconds 600 -IntervalMs 500`],{stdio:['ignore','pipe','pipe'],windowsHide:true});
let sampled='',errors='';sampler.stdout.on('data',v=>sampled+=v);sampler.stderr.on('data',v=>errors+=v);
const sampledDone=once(sampler,'exit');
const timeout=setTimeout(()=>{child.kill();sampler.kill();},600000);
try{
 const [code]=await completed;const [sampleCode]=await sampledDone;
 await writeFile(join(root,'bench.log'),output);
 assert.equal(code,0,output);assert.equal(sampleCode,0,errors);
 assert.match(output,/inserted: 1000000 in/);assert.match(output,/V20 BENCH DONE/);
 const resources=JSON.parse(sampled);assert.ok(resources.sample_count>2);
 const result={root,events:1000000,sampling_interval_ms:500,resources,ingestion:output.match(/inserted:.*$/m)?.[0],query:output.match(/uncached dashboard.*$/m)?.[0]};
 await writeFile(join(root,'result.json'),JSON.stringify(result,null,2));
 console.log(JSON.stringify({root,peak_private_mib:resources.peak_private_bytes/1048576,peak_working_set_mib:resources.peak_working_set_bytes/1048576,ingestion:result.ingestion}));
}finally{clearTimeout(timeout);if(child.exitCode===null)child.kill();if(sampler.exitCode===null)sampler.kill();}
