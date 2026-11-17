#!/usr/bin/env python3
"""Retain mtimes only for byte-identical tracked sources copied by git archive.

Cargo otherwise sees every file as changed because archive mtimes equal the
new commit time. State lives beside the persistent target cache, not in source.
"""
import hashlib
import json
import os
from pathlib import Path

root = Path('/src')
state = root / 'target/.gravity-source-mtimes.json'
try:
    old = json.loads(state.read_text())
except (OSError, ValueError):
    old = {}
new = {}
restored = 0
for directory, dirs, files in os.walk(root):
    if Path(directory) == root:
        dirs[:] = [d for d in dirs if d != 'target']
    for name in files:
        path = Path(directory) / name
        if path.is_symlink():
            continue
        relative = str(path.relative_to(root))
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        before = old.get(relative)
        if isinstance(before, list) and len(before) == 2 and before[0] == digest and isinstance(before[1], int):
            stat = path.stat()
            os.utime(path, ns=(stat.st_atime_ns, before[1]))
            restored += 1
        new[relative] = [digest, path.stat().st_mtime_ns]
state.parent.mkdir(parents=True, exist_ok=True)
temporary = state.with_suffix('.tmp')
temporary.write_text(json.dumps(new))
temporary.replace(state)
print(f'Source timestamp cache: {restored}/{len(new)} unchanged files restored')
