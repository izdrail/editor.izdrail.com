#!/usr/bin/env python3
"""Run one explicit JSON-lines worker request. Use with run.sh for measurements."""
import argparse
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import sys
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--worker', required=True)
    parser.add_argument('--request', required=True, help='JSON file with one complete worker request')
    parser.add_argument('--timeout', type=float, default=300)
    parser.add_argument('--repeat', type=int, default=1)
    args = parser.parse_args()
    if not 1 <= args.repeat <= 1000 or not 1 <= args.timeout <= 3600:
        parser.error('repeat must be 1..1000 and timeout 1..3600')
    request = json.loads(Path(args.request).read_text())
    if not isinstance(request, dict) or request.get('remote') is not None:
        parser.error('this driver accepts local requests only; remote benchmarking requires separate server telemetry')
    if not Path(request.get('model', '')).is_file():
        parser.error('request must name an existing local model')
    command = [args.worker, '--ai-worker']
    records = []
    process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=sys.stderr,
                               text=True, bufsize=1, start_new_session=True)
    try:
        for i in range(args.repeat):
            request['request_id'] = i + 1
            started = time.monotonic()
            process.stdin.write(json.dumps(request) + '\n')
            process.stdin.flush()
            if not select.select([process.stdout], [], [], args.timeout)[0]:
                raise TimeoutError('worker response deadline exceeded')
            line = process.stdout.readline()
            if not line:
                raise RuntimeError('worker exited before completion')
            result = json.loads(line)
            if not isinstance(result, dict) or not isinstance(result.get('text'), str) or 'tokens' not in result:
                raise RuntimeError('worker returned an error or malformed completion')
            if result.get('cancelled'):
                raise RuntimeError('worker cancelled benchmark request')
            elapsed = time.monotonic() - started
            records.append({'index': i, 'wall_s': elapsed, 'completion': result})
            print(json.dumps(records[-1]), file=sys.stderr, flush=True)
    finally:
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
    last = records[-1]['completion']
    decode = last.get('decode_ms', 0)
    # Worker protocol returns one completion. It cannot measure TTFT or
    # token streaming latency; do not relabel wall time as those metrics.
    print(json.dumps({'load_ms': last.get('load_ms'), 'tokens': last['tokens'],
                      'gen_tps': last['tokens'] * 1000 / decode if decode > 0 else None,
                      'ttft_ms': None, 'prompt_tps': None, 'cancel_ms': None,
                      'worker_requests': len(records), 'results': records}))


if __name__ == '__main__':
    main()
