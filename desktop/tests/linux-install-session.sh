set -euo pipefail
test "$(id -u)" -ne 0
task_root=/workspace/llm-usage/build/install-lifecycle
export HOME=/home/acceptance
export CODEX_HOME="$HOME/.codex"
export TEMP="$task_root/temp" TMP="$task_root/temp"
mkdir -p "$TEMP" "$CODEX_HOME/sessions"
python3 - <<'PY'
import datetime,json,os
from pathlib import Path
at=datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00','Z')
rows=[{'timestamp':at,'type':'session_meta','payload':{'id':'linux-lifecycle-synthetic','session_id':'linux-lifecycle-synthetic','timestamp':at,'originator':'codex_vscode','cli_version':'0.999.0-synthetic','model_provider':'openai','source':'vscode'}},{'timestamp':at,'type':'token_usage_record','payload':{'thread_id':'linux-lifecycle-synthetic','turn_id':'linux-lifecycle-synthetic','session_id':'linux-lifecycle-synthetic','response_id':'linux-lifecycle-synthetic','usage':{'input_tokens':10,'cached_input_tokens':0,'cache_write_input_tokens':0,'output_tokens':5,'reasoning_output_tokens':0,'total_tokens':15}}}]
path=Path(os.environ['CODEX_HOME'])/'sessions/rollout-lifecycle.jsonl'
if not path.exists():
    path.write_text('\n'.join(map(json.dumps,rows))+'\n')
PY
export DISPLAY=:99 TAURI_WEBVIEW_AUTOMATION=true
case "${LLM_USAGE_GUI_SCALE:-1}" in
  1) display_size=1440x1000x24 ;;
  2) display_size=2880x2000x24 ;;
  *) printf 'Unsupported GUI scale\n' >&2; exit 1 ;;
esac
export GDK_SCALE="${LLM_USAGE_GUI_SCALE:-1}" GDK_DPI_SCALE=1
export XDG_RUNTIME_DIR="$task_root/runtime"
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
Xvfb :99 -screen 0 "$display_size" -dpi 96 -nolisten tcp >"$task_root/xvfb.log" 2>&1 &
display_pid=$!
trap 'kill "$display_pid" 2>/dev/null || true' EXIT
for n in {1..50}; do xdpyinfo >/dev/null 2>&1 && break; sleep .1; done
dbus-run-session -- bash -c '
set -euo pipefail
task_root=/workspace/llm-usage/build/install-lifecycle
openbox >"$task_root/openbox.log" 2>&1 &
manager_pid=$!
WebKitWebDriver --host=127.0.0.1 --port=4444 >"$task_root/webkit-driver.log" 2>&1 &
driver_pid=$!
trap '\''kill "$driver_pid" "$manager_pid" 2>/dev/null || true'\'' EXIT
python3 "$task_root/linux-install-desktop.py"
'
