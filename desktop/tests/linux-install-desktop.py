import base64
import json
import os
from pathlib import Path
import time
import urllib.request
import sqlite3
import subprocess

root = Path('/workspace/llm-usage/build/install-lifecycle')
data = root / '含空格 data'
data.mkdir(parents=True, exist_ok=True)
checks = []
session = None
fuse_mounts = []
result = None

def appimage_mounts():
    return [line for line in Path('/proc/self/mountinfo').read_text().splitlines()
            if ' - fuse.LLMUsage.AppImage ' in line]

def process_security():
    fields = ('Uid:', 'CapEff:', 'Seccomp:')
    return dict(line.split(':', 1) for line in Path('/proc/self/status').read_text().splitlines()
                if line.startswith(fields))

assert os.getuid() == 1000, 'GUI acceptance must run as the ordinary acceptance user'
assert int(process_security()['CapEff'].strip(), 16) == 0
assert process_security()['Seccomp'].strip() == '2', 'seccomp must remain active'

# A DOM containing Chinese text can still render unreadable boxes without a CJK font.
charset = subprocess.check_output(['fc-match', '-f', '%{charset}', ':lang=zh-cn'], text=True)
ranges = [tuple(int(part, 16) for part in token.split('-')) for token in charset.split()]
for character in '中文设置':
    assert any(interval[0] <= ord(character) <= interval[-1] for interval in ranges), 'CJK font coverage missing'

def request(method, route, payload=None):
    body = None if payload is None else json.dumps(payload).encode()
    req = urllib.request.Request('http://127.0.0.1:4444'+route, data=body, method=method, headers={'Content-Type':'application/json'})
    try:
        with urllib.request.urlopen(req, timeout=40) as response:
            result = json.load(response)
    except urllib.error.HTTPError as error:
        raise RuntimeError(error.read().decode()) from error
    value = result.get('value')
    if isinstance(value, dict) and 'error' in value:
        raise RuntimeError(json.dumps(value))
    return value

def script(text, args=None, asynchronous=False):
    return request('POST', f'/session/{session}/execute/'+('async' if asynchronous else 'sync'), {'script':text,'args':args or []})

def ipc(command, arguments=None):
    return script('const done=arguments[arguments.length-1];window.__TAURI_INTERNALS__.invoke(arguments[0],arguments[1]).then(value=>done({ok:true,value}),error=>done({ok:false,error:String(error)}));', [command, arguments or {}], True)

try:
    for _ in range(100):
        try:
            request('GET','/status')
            break
        except Exception:
            time.sleep(.1)
    response=request('POST','/session',{'capabilities':{'alwaysMatch':{'webkitgtk:browserOptions':{'binary':os.environ.get('LLM_USAGE_EXE','/usr/bin/LLMUsage'),'args':['--data-dir',str(data)]}}}})
    session=response['sessionId']
    request('POST',f'/session/{session}/timeouts',{'script':30000,'implicit':0,'pageLoad':30000})
    for _ in range(150):
        if script('return Boolean(window.__TAURI_INTERNALS__?.invoke && document.querySelector(".today-cards"));'):
            break
        time.sleep(.1)
    assert script('return Boolean(window.__TAURI_INTERNALS__?.invoke && document.querySelector(".today-cards"));')
    checks.append('native GTK/WebKit window loads packaged frontend')
    display = script('return {scale:devicePixelRatio,width:innerWidth,height:innerHeight,zoom:getComputedStyle(document.documentElement).zoom,screen:{width:screen.width,height:screen.height}};')
    assert display['scale'] == int(os.environ.get('LLM_USAGE_GUI_SCALE', '1')), display
    assert display['zoom'] in ('1', 'normal'), display
    checks.append('native WebKit device scale matches GTK window scaling without CSS zoom')
    if os.environ.get('LLM_USAGE_APPIMAGE_MODE') == 'fuse':
        assert not os.environ.get('APPIMAGE_EXTRACT_AND_RUN'), 'FUSE acceptance cannot use extraction fallback'
        fuse_mounts = appimage_mounts()
        assert len(fuse_mounts) == 1, fuse_mounts
        fields = fuse_mounts[0].split()
        assert 'ro' in fields[5].split(',') and 'user_id=1000' in fields[-1].split(','), fields
        mountpoint = fields[4]
        applications = []
        for entry in Path('/proc').iterdir():
            if not entry.name.isdigit():
                continue
            try:
                executable = os.readlink(entry / 'exe')
                if executable.startswith(mountpoint + '/') and Path(executable).name == 'LLMUsage':
                    status = dict(line.split(':', 1) for line in (entry / 'status').read_text().splitlines() if line.startswith(('Uid:', 'CapEff:', 'Seccomp:')))
                    assert [int(value) for value in status['Uid'].split()] == [1000]*4, status
                    assert int(status['CapEff'].strip(), 16) == 0, status
                    assert status['Seccomp'].strip() == '2', status
                    applications.append({'pid': int(entry.name), 'executable': executable, 'security': status})
            except (FileNotFoundError, PermissionError, ProcessLookupError):
                continue
        assert applications, 'The actual GUI executable must run from the FUSE mount'
        checks.append('AppImage GUI runs from a read-only FUSE mount as UID 1000 with no effective capabilities')
    for _ in range(100):
        sources=ipc('list_sources')
        if sources['ok'] and len(sources['value']['sources'])==1:
            break
        time.sleep(.1)
    sources=ipc('list_sources')
    assert sources['ok'],sources
    assert len(sources['value']['sources']) == 1, sources
    assert sources['value']['sources'][0]['agent']=='codex',sources
    checks.append('real IPC discovers only isolated synthetic source')
    previous=ipc('refresh_status')['value'].get('last_finished_ms')
    assert ipc('refresh_sources')['ok']
    for _ in range(200):
        status=ipc('refresh_status')['value']
        if not status['running'] and status.get('last_finished_ms') != previous:
            break
        time.sleep(.1)
    with sqlite3.connect(data/'llm-usage.sqlite') as connection:
        count,tokens=connection.execute('SELECT COUNT(*),SUM(CAST(total_tokens AS INTEGER)) FROM usage_events').fetchone()
    assert (count,tokens)==(1,15),(count,tokens)
    checks.append('real native refresh commits one observation and 15 synthetic tokens')
    controls=script('return Array.from(document.querySelectorAll("nav button")).map(el=>el.textContent);')
    assert len(controls)==5,controls
    layouts = []
    for index in range(5):
        script('document.querySelectorAll("nav button")[arguments[0]].click();',[index])
        time.sleep(.4)
        assert script('return document.body.innerText.length;')>100
        layout = script('return {width:innerWidth,documentWidth:document.documentElement.scrollWidth,bodyWidth:document.body.scrollWidth};')
        assert max(layout['documentWidth'], layout['bodyWidth']) <= layout['width'] + 1, layout
        layouts.append(layout)
    checks.append('all five pages render through real WebKit')
    assert ipc('system_task_status')['value']['unsupported']
    checks.append('Linux reports unsupported Windows-only scheduling integration')
    screenshot=request('GET',f'/session/{session}/screenshot')
    (root/'linux-desktop.png').write_bytes(base64.b64decode(screenshot))
    print(json.dumps({'checks':checks,'events':count,'synthetic_tokens':tokens,'pages':controls},ensure_ascii=False))
    result = {'checks':checks,'events':count,'synthetic_tokens':tokens,'pages':controls,'security':process_security(),'display':display,'layouts':layouts}
    if fuse_mounts:
        result['fuse'] = {'mountinfo': fuse_mounts, 'applications': applications}
    (root/'linux-desktop-result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2))
finally:
    if session:
        request('DELETE',f'/session/{session}')
        if fuse_mounts:
            for _ in range(100):
                if not appimage_mounts():
                    break
                time.sleep(.1)
            assert not appimage_mounts(), 'AppImage FUSE mount must be released after closing the GUI'
            if result is not None:
                result['fuse']['released_after_exit'] = True
                result['checks'].append('AppImage FUSE mount is released after the GUI exits')
                (root/'linux-desktop-result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2))
