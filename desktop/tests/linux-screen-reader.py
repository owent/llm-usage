import json
import os
from pathlib import Path
import subprocess
import time


def verify(root, script, ipc):
    # Use the real settings form; backend-only IPC does not update frontend locale.
    script('const el=document.querySelector(\'[data-testid="language-select"]\');'
           'el.value="en";el.dispatchEvent(new Event("input",{bubbles:true}));'
           'el.dispatchEvent(new Event("change",{bubbles:true}));'
           'document.querySelector(\'button[type="submit"]\').click();')
    for _ in range(100):
        if script('return document.documentElement.lang;') == 'en':
            break
        time.sleep(.1)
    assert script('return document.documentElement.lang;') == 'en'
    assert ipc('get_settings')['value']['language'] == 'en'
    processes = []
    for entry in Path('/proc').iterdir():
        if entry.name.isdigit():
            try:
                if Path(os.readlink(entry / 'exe')).name == 'LLMUsage':
                    processes.append(entry.name)
            except (FileNotFoundError, PermissionError, ProcessLookupError):
                pass
    assert len(processes) == 1, processes
    windows = subprocess.check_output(
        ['xdotool', 'search', '--onlyvisible', '--pid', processes[0]], text=True).splitlines()
    assert windows
    subprocess.run(['xdotool', 'windowactivate', '--sync', windows[0]], check=True)
    time.sleep(5)
    names = script('return Array.from(document.querySelector("nav").querySelectorAll("button"))'
                   '.map(el=>el.textContent.trim());')
    assert len(names) == 5, names
    assert script('return Array.from(document.querySelector("nav").querySelectorAll("button"))'
                  '.every(el=>el.getAttribute("aria-label")===el.textContent.trim());')
    observed = []
    for _ in range(35):
        subprocess.run(['xdotool', 'key', '--clearmodifiers', 'Tab'], check=True)
        time.sleep(.4)
        focused = script('const el=document.activeElement;return {tag:el.tagName,'
                         'name:(el.getAttribute("aria-label")||el.textContent||"").trim(),'
                         'navigation:Boolean(el.closest("nav")===document.querySelector("nav"))};')
        observed.append(focused)
        if focused['navigation']:
            for _ in range(200):
                lines = (root / 'orca-debug.log').read_text(errors='replace').splitlines()
                if any('SPEECH OUTPUT' in line and focused['name'] in line for line in lines):
                    break
                time.sleep(.1)
            assert any('SPEECH OUTPUT' in line and focused['name'] in line for line in lines), focused
            subprocess.run(['xdotool', 'key', '--clearmodifiers', 'Return'], check=True)
            time.sleep(.3)
            assert script('return document.querySelector("main h1").textContent.trim();') == focused['name']
        if all(any(value['navigation'] and value['name'] == name for value in observed) for name in names):
            break
    assert all(any(value['navigation'] and value['name'] == name for value in observed) for name in names)
    # The installed Orca only uses line buffering for this test's diagnostic file.
    orca_pid = int(os.environ['LLM_USAGE_ORCA_PID'])
    owned_status = dict(line.split(':', 1) for line in
                        (Path('/proc') / str(orca_pid) / 'status').read_text().splitlines()
                        if line.startswith(('PPid:', 'Uid:', 'Name:')))
    assert int(owned_status['PPid'].strip()) == os.getppid(), owned_status
    assert all(int(uid) == os.getuid() for uid in owned_status['Uid'].split()), owned_status
    debug = (root / 'orca-debug.log').read_text(errors='replace')
    speech = [line for line in debug.splitlines() if 'SPEECH OUTPUT' in line]
    assert all(name in '\n'.join(speech) for name in names), (names, speech[-30:])
    assert 'speechdispatcherfactory' in debug
    assert all(f"name='{name}' role='button'" in debug for name in names)
    assert 'SPEECH DISPATCHER: Speaking' in debug
    locale_navigation = []
    # Prepare each locale through the product form, then require fresh native
    # focus events and Orca speech. Earlier English speech cannot certify a
    # subsequent locale, even when a translated control has the same spelling.
    locales = ['zh-CN', 'zh-TW', 'en', 'ja', 'ko', 'es', 'fr', 'de', 'pt-BR', 'ru']
    script('document.querySelector("nav").querySelectorAll("button")[4].click();')
    for _ in range(100):
        if script('return Boolean(document.querySelector('
                  '\'[data-testid="language-select"]\'));'):
            break
        time.sleep(.1)
    assert sorted(script('return Array.from(document.querySelector('
                         '\'[data-testid="language-select"]\')?.options||[])'
                         '.map(el=>el.value);')) == sorted(locales)
    for locale in locales:
        script('document.querySelector("nav").querySelectorAll("button")[4].click();')
        for _ in range(100):
            if script('return Boolean(document.querySelector('
                      '\'[data-testid="language-select"]\'));'):
                break
            time.sleep(.1)
        script('const el=document.querySelector(\'[data-testid="language-select"]\');'
               f'el.value={json.dumps(locale)};'
               'el.dispatchEvent(new Event("input",{bubbles:true}));'
               'el.dispatchEvent(new Event("change",{bubbles:true}));'
               'document.querySelector(\'button[type="submit"]\').click();')
        for _ in range(100):
            if (script('return document.documentElement.lang;') == locale
                    and ipc('get_settings')['value']['language'] == locale):
                break
            time.sleep(.1)
        assert script('return document.documentElement.lang;') == locale
        assert ipc('get_settings')['value']['language'] == locale
        locale_names = script('return Array.from(document.querySelector("nav").querySelectorAll("button"))'
                              '.map(el=>el.getAttribute("aria-label"));')
        assert len(locale_names) == 5 and all(locale_names), (locale, locale_names)
        reached = []
        locale_speech = []
        for _ in range(60):
            before = len((root / 'orca-debug.log').read_text(errors='replace').splitlines())
            subprocess.run(['xdotool', 'key', '--clearmodifiers', 'Tab'], check=True)
            time.sleep(.4)
            focused = script('const el=document.activeElement;return {tag:el.tagName,'
                             'name:(el.getAttribute("aria-label")||el.textContent||"").trim(),'
                             'navigation:Boolean(el.closest("nav")===document.querySelector("nav"))};')
            if not focused['navigation']:
                continue
            for _ in range(200):
                fresh = (root / 'orca-debug.log').read_text(errors='replace').splitlines()[before:]
                spoken = [line for line in fresh if 'SPEECH OUTPUT' in line]
                if any(focused['name'] in line for line in spoken):
                    break
                time.sleep(.1)
            assert any(focused['name'] in line for line in spoken), (locale, focused, fresh[-30:])
            focus_events = [line for line in fresh if 'FOCUS MANAGER: Locus of focus is' in line
                            and f"[button: '{focused['name']}']" in line]
            assert focus_events, (locale, focused, fresh[-30:])
            focused['atspi_focus_events'] = focus_events
            locale_speech.extend(spoken)
            reached.append(focused)
            subprocess.run(['xdotool', 'key', '--clearmodifiers', 'Return'], check=True)
            for _ in range(100):
                if script('return document.querySelector("main h1").textContent.trim();') == focused['name']:
                    break
                time.sleep(.1)
            assert script('return document.querySelector("main h1").textContent.trim();') == focused['name']
            if all(any(value['name'] == name for value in reached) for name in locale_names):
                break
        assert all(any(value['name'] == name for value in reached) for name in locale_names), (locale, reached)
        locale_navigation.append({'locale': locale, 'navigation': locale_names,
                                  'focused': reached, 'speech_lines': locale_speech})
    # Preserve the lifecycle driver's original English end state.
    script('document.querySelector("nav").querySelectorAll("button")[4].click();')
    script('const el=document.querySelector(\'[data-testid="language-select"]\');'
           'el.value="en";el.dispatchEvent(new Event("input",{bubbles:true}));'
           'el.dispatchEvent(new Event("change",{bubbles:true}));'
           'document.querySelector(\'button[type="submit"]\').click();')
    for _ in range(100):
        if script('return document.documentElement.lang;') == 'en':
            break
        time.sleep(.1)
    assert script('return document.documentElement.lang;') == 'en'
    assert ipc('get_settings')['value']['language'] == 'en'
    result = {'navigation': names, 'focused': observed, 'speech_lines': speech,
              'locale_navigation': locale_navigation,
              'speech_backend': 'speech-dispatcher/espeak-ng', 'audio_device': 'ALSA null',
              'actual_keyboard': True, 'physical_audio_checked': False, 'owned_reader': owned_status,
              'diagnostic_line_buffering': True}
    (root / 'orca-result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2))
    return 'actual Orca fresh speech and AT-SPI names cover five native Tab/Enter navigation controls in ten locales'
