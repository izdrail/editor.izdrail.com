#!/usr/bin/env python3
"""Run one explicit JSON-lines worker request. Use with run.sh for measurements."""
import argparse
import json
import os
from pathlib import Path
import selectors
import signal
import subprocess
import sys
import time


MAX_LINE = 8 * 1024 * 1024


def exchange(process, request, timeout):
    """One bounded line, with a single deadline covering writes and partial reads."""
    payload = (json.dumps(request) + '\n').encode()
    if len(payload) > MAX_LINE:
        raise ValueError('worker request exceeds 8 MiB limit')
    deadline = time.monotonic() + timeout
    response = bytearray()
    sent = 0
    os.set_blocking(process.stdin.fileno(), False)
    os.set_blocking(process.stdout.fileno(), False)
    with selectors.DefaultSelector() as selector:
        selector.register(process.stdin, selectors.EVENT_WRITE)
        selector.register(process.stdout, selectors.EVENT_READ)
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError('worker request/response deadline exceeded')
            for key, _ in selector.select(remaining):
                if key.fileobj is process.stdin:
                    try:
                        count = os.write(process.stdin.fileno(), payload[sent:sent + 65536])
                    except BlockingIOError:
                        continue
                    sent += count
                    if sent == len(payload):
                        selector.unregister(process.stdin)
                else:
                    try:
                        chunk = os.read(process.stdout.fileno(), 65536)
                    except BlockingIOError:
                        continue
                    if not chunk:
                        raise RuntimeError('worker exited before complete response')
                    response.extend(chunk)
                    if len(response) > MAX_LINE:
                        raise ValueError('worker response exceeds 8 MiB limit')
                    if b'\n' in response:
                        line, remainder = response.split(b'\n', 1)
                        if sent != len(payload) or remainder:
                            raise RuntimeError('unexpected extra worker output')
                        return json.loads(line)


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
    expected_model = os.environ.get('MODEL')
    if not expected_model or not Path(request['model']).samefile(expected_model):
        parser.error('request model must be the exact MODEL file verified by run.sh')
    command = [args.worker, '--ai-worker']
    records = []
    process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=sys.stderr,
                               bufsize=0, start_new_session=True)
    try:
        for i in range(args.repeat):
            request['request_id'] = i + 1
            started = time.monotonic()
            result = exchange(process, request, args.timeout)
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
