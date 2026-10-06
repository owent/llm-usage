"""Run installed Orca with line-buffered test diagnostics; retain its behavior."""
from pathlib import Path
import sys

entry = Path('/usr/bin/orca')
source = entry.read_text()
original = "debug.debugFile = open(args.debug_file, 'w')"
assert source.count(original) == 1, 'Recheck the installed Orca diagnostic file API after upgrades'
source = source.replace(original, "debug.debugFile = open(args.debug_file, 'w', buffering=1)")
sys.argv[0] = str(entry)
exec(compile(source, str(entry), 'exec'), {'__name__': '__main__', '__file__': str(entry)})
