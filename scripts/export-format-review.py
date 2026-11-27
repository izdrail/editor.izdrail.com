#!/usr/bin/env python3
"""Export formatter differences without changing checked-out source files."""
import difflib
import json
import subprocess
from pathlib import Path

out = Path('target/format-review')
out.mkdir(parents=True, exist_ok=True)
# Include both separately reviewed completion changes and the formatting job
# itself. Restrict to existing tracked Rust files, never user-provided paths.
changed = subprocess.check_output(
    ['git', 'diff', '--name-only', '--diff-filter=ACM', 'HEAD~3', 'HEAD', '--', '*.rs'],
    text=True,
).splitlines()
summary = []
for name in changed:
    path = Path(name)
    if not path.is_file():
        continue
    original = path.read_text()
    result = subprocess.run(
        ['rustfmt', '--edition', '2024', '--emit', 'stdout', '--config', 'skip_children=true', name],
        text=True, capture_output=True,
    )
    entry = {'file': name, 'returncode': result.returncode}
    if result.returncode:
        (out / (name.replace('/', '__') + '.error.txt')).write_text(result.stderr)
    else:
        formatted = result.stdout
        diff = ''.join(difflib.unified_diff(
            original.splitlines(True), formatted.splitlines(True),
            fromfile='a/' + name, tofile='b/' + name,
        ))
        entry['has_format_diff'] = bool(diff)
        (out / (name.replace('/', '__') + '.diff')).write_text(diff)
        # A review artifact only. Never copied over the checked-out source.
        destination = out / 'formatted' / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(formatted)
    summary.append(entry)
(out / 'summary.json').write_text(json.dumps(summary, indent=2))
print(json.dumps(summary, indent=2))
if any(item['returncode'] for item in summary):
    raise SystemExit('Formatter failed; inspect format-review artifact')
