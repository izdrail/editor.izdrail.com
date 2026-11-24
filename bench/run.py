#!/usr/bin/env python3
"""Owner-run Linux collector. Never downloads models or installs software."""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import subprocess
import time


def capture(argv):
    try:
        p = subprocess.run(argv, capture_output=True, text=True, timeout=10)
        return p.stdout.strip() if p.returncode == 0 else None
    except (OSError, subprocess.TimeoutExpired):
        return None


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('provider', choices=['candle', 'llamacpp', 'mistralrs', 'ollama'])
    ap.add_argument('scenario', choices=['cold', 'warm', 'short', 'long', 'repeat', 'cancel', 'stability'])
    ap.add_argument('run', type=int)
    ap.add_argument('command', nargs=argparse.REMAINDER)
    args = ap.parse_args()
    command = args.command[1:] if args.command[:1] == ['--'] else args.command
    if not command or args.run < 1:
        ap.error('a command and positive run index are required')
    model = Path(os.environ.get('MODEL', ''))
    expected = os.environ.get('EXPECT_SHA256', '')
    if not model.is_file() or len(expected) != 64:
        ap.error('MODEL and EXPECT_SHA256 must identify the exact approved GGUF')
    digest = hashlib.sha256()
    with model.open('rb') as f:
        for block in iter(lambda: f.read(4 * 1024 * 1024), b''):
            digest.update(block)
    if digest.hexdigest() != expected.lower():
        ap.error('model SHA256 mismatch')
    interval = float(os.environ.get('INTERVAL', '.5'))
    if interval < .1:
        ap.error('INTERVAL must be at least 0.1 seconds')
    out = Path(os.environ.get('OUT', 'bench/results'))
    out.mkdir(parents=True, exist_ok=True)
    base = out / f'{args.provider}_{args.scenario}_{args.run}'
    # Exclusive creation prevents silently replacing previous measurements.
    reservation = base.with_suffix('.json')
    with reservation.open('x') as f:
        json.dump({'state': 'incomplete'}, f)
    record = {
        'provider': args.provider, 'scenario': args.scenario, 'run': args.run,
        'model_sha256': digest.hexdigest(), 'model': str(model.resolve()),
        'host': {'kernel': platform.release(), 'machine': platform.machine(), 'node': platform.node(), 'cpus': os.cpu_count()},
        'meminfo': Path('/proc/meminfo').read_text(),
        'gpu': capture(['nvidia-smi', '--query-gpu=name,driver_version,memory.total', '--format=csv,noheader']),
        'cuda': capture(['nvcc', '--version']),
        'git_commit': capture(['git', 'rev-parse', 'HEAD']),
        'command': command,
        'config': os.environ.get('CONFIG'), 'cargo_features': os.environ.get('CARGO_FEATURES'),
        'build_cmd': os.environ.get('BUILD_CMD'), 'provider_version': os.environ.get('PROVIDER_VERSION'),
        'cold_cache_verified': False, 'reported': None,
        'measurement_scope': 'RSS/CPU are command process only; GPU samples are device-wide, not proof of this process using CUDA',
    }
    start = time.monotonic()
    with base.with_suffix('.out').open('x') as stdout, base.with_suffix('.err').open('x') as stderr, base.with_suffix('.samples.csv').open('x', newline='') as samples:
        writer = csv.writer(samples)
        writer.writerow(['t', 'rss_kb', 'cpu_pct', 'gpu_util', 'vram_mib'])
        proc = subprocess.Popen(command, stdout=stdout, stderr=stderr, start_new_session=True)
        try:
            while proc.poll() is None:
                rss = None
                try:
                    for line in Path(f'/proc/{proc.pid}/status').read_text().splitlines():
                        if line.startswith('VmRSS:'):
                            rss = int(line.split()[1])
                except (FileNotFoundError, ProcessLookupError):
                    pass
                cpu = capture(['ps', '-o', '%cpu=', '-p', str(proc.pid)])
                gpu = capture(['nvidia-smi', '--query-gpu=utilization.gpu,memory.used', '--format=csv,noheader,nounits'])
                # Never attribute a random GPU when multiple devices exist.
                pair = gpu.split(',') if gpu and '\n' not in gpu else ['', '']
                writer.writerow([time.monotonic() - start, rss, cpu, *[p.strip() for p in pair]])
                samples.flush()
                time.sleep(interval)
            record['exit_code'] = proc.wait()
        except BaseException:
            os.killpg(proc.pid, signal.SIGTERM)
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(proc.pid, signal.SIGKILL)
                proc.wait()
            raise
    record['wall_s'] = time.monotonic() - start
    lines = base.with_suffix('.out').read_text(errors='replace').splitlines()
    try:
        value = json.loads(lines[-1])
        if isinstance(value, dict):
            record['reported'] = value
    except (IndexError, json.JSONDecodeError):
        pass
    record['state'] = 'complete'
    reservation.write_text(json.dumps(record, indent=2))
    print(reservation)
    return record['exit_code']


if __name__ == '__main__':
    raise SystemExit(main())
