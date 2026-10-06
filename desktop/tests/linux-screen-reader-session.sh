set -euo pipefail
task_root=/workspace/llm-usage/build/install-lifecycle
python3 - <<'PY'
from gi.repository import Gio
from pathlib import Path
import shutil
assert Gio.Settings.new('org.gnome.desktop.interface').set_boolean('toolkit-accessibility', True)
assert Gio.Settings.new('org.gnome.desktop.a11y.applications').set_boolean('screen-reader-enabled', True)
Gio.Settings.sync()
root=Path('/workspace/llm-usage/build/install-lifecycle')
shutil.copytree('/etc/speech-dispatcher', root/'speechd-config', dirs_exist_ok=True)
(root/'speechd-logs').mkdir(exist_ok=True)
with (root/'speechd-config/speechd.conf').open('a') as file:
    file.write('\nAudioOutputMethod "alsa"\nAudioALSADevice "null"\nAddModule "espeak-ng" "sd_espeak-ng" "espeak-ng.conf"\nDefaultModule espeak-ng\n')
PY
speech-dispatcher -s -C "$task_root/speechd-config" -L "$task_root/speechd-logs" -l 4 -t 0 >"$task_root/speechd-stdout.log" 2>&1 &
speech_pid=$!
python3 "$task_root/linux-screen-reader-orca.py" --debug --debug-file "$task_root/orca-debug.log" -e speech -d braille >"$task_root/orca-stdout.log" 2>&1 &
orca_pid=$!
export LLM_USAGE_ORCA_PID="$orca_pid"
cleanup() {
    kill "$orca_pid" "$speech_pid" 2>/dev/null || true
    for n in {1..30}; do
        if ! kill -0 "$orca_pid" 2>/dev/null && ! kill -0 "$speech_pid" 2>/dev/null; then break; fi
        sleep .1
    done
    kill -KILL "$orca_pid" "$speech_pid" 2>/dev/null || true
    wait "$orca_pid" "$speech_pid" 2>/dev/null || true
}
trap cleanup EXIT
for n in {1..50}; do
    kill -0 "$speech_pid" "$orca_pid"
    test -s "$task_root/orca-debug.log" && break
    sleep .1
done
sleep 2
python3 "$task_root/linux-install-desktop.py"
