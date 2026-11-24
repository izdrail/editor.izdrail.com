#!/usr/bin/env python3
"""Standard-library fixture tests only, no model or GPU use."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent


def module(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / (name + '.py'))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


class BenchTests(unittest.TestCase):
    def test_header_rejects_truncation_and_wrong_magic(self):
        inspector = module('inspect_gguf')
        with tempfile.TemporaryDirectory() as d:
            p = Path(d, 'fixture')
            p.write_bytes(b'GGUF' + struct.pack('<IQQ', 3, 0, 0))
            self.assertEqual(inspector.inspect(p)['tensor_count'], 0)
            for bad in (b'', b'GGUF', b'FAIL' + struct.pack('<IQQ', 3, 0, 0)):
                p.write_bytes(bad)
                with self.assertRaises(ValueError):
                    inspector.inspect(p)

    def test_run_preserves_quotes_and_refuses_overwrite_and_wrong_hash(self):
        with tempfile.TemporaryDirectory() as d:
            model = Path(d, 'fixture-not-a-model.gguf')
            model.write_bytes(b'collector fixture')
            env = dict(os.environ, MODEL=str(model), EXPECT_SHA256=hashlib.sha256(model.read_bytes()).hexdigest(), OUT=d, CONFIG='fixture', INTERVAL='.1')
            command = ['sh', str(ROOT / 'run.sh'), 'candle', 'short', '1', '--', sys.executable, '-c', 'import json;print(json.dumps({"tokens":2,"text":"quote\\\""}))']
            result = subprocess.run(command, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            value = json.loads(Path(d, 'candle_short_1.json').read_text())
            self.assertEqual(value['reported']['tokens'], 2)
            self.assertEqual(value['state'], 'complete')
            self.assertNotEqual(subprocess.run(command, env=env, capture_output=True).returncode, 0)
            env['EXPECT_SHA256'] = '0' * 64
            self.assertNotEqual(subprocess.run(command, env=env, capture_output=True).returncode, 0)

    def test_worker_driver_repeats_and_refuses_different_model(self):
        with tempfile.TemporaryDirectory() as d:
            model = Path(d, 'model'); model.write_bytes(b'fixture')
            other = Path(d, 'other'); other.write_bytes(b'fixture')
            worker = Path(d, 'worker')
            worker.write_text('#!/usr/bin/env python3\nimport sys,json\nfor line in sys.stdin:\n print(json.dumps(dict(text="ok",tokens=2,decode_ms=10,load_ms=1)),flush=True)\n')
            worker.chmod(0o700)
            request = Path(d, 'request.json');request.write_text(json.dumps(dict(model=str(model),remote=None)))
            env = dict(os.environ, MODEL=str(model))
            command = [sys.executable, str(ROOT/'worker.py'), '--worker', str(worker), '--request', str(request), '--repeat', '3']
            result = subprocess.run(command, env=env, capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stderr)
            output = json.loads(result.stdout)
            self.assertEqual(output['worker_requests'], 3)
            self.assertEqual(output['gen_tps'], 200)
            self.assertIsNone(output['ttft_ms'])
            env['MODEL'] = str(other)
            self.assertNotEqual(subprocess.run(command, env=env, capture_output=True, timeout=10).returncode, 0)

    def test_worker_deadline_covers_partial_line_and_blocked_write(self):
        import time
        driver = module('worker')
        for program, request in [
            ('import sys,time;sys.stdout.write("{\\"text\\":");sys.stdout.flush();time.sleep(10)', {}),
            ('import time;time.sleep(10)', {'prefix': 'x' * 1_000_000}),
        ]:
            process = subprocess.Popen([sys.executable, '-c', program], stdin=subprocess.PIPE, stdout=subprocess.PIPE, bufsize=0)
            started = time.monotonic()
            try:
                with self.assertRaises(TimeoutError):
                    driver.exchange(process, request, .15)
                self.assertLess(time.monotonic() - started, 2)
            finally:
                process.kill(); process.wait()
                process.stdin.close(); process.stdout.close()

    def test_build_helper_isolates_backend_and_device_artifacts(self):
        with tempfile.TemporaryDirectory() as d:
            bin_dir = Path(d, 'bin'); bin_dir.mkdir()
            cargo = bin_dir / 'cargo'
            cargo.write_text('''#!/bin/sh
set -eu
if [ "$1" = --version ]; then echo fixture-cargo; exit; fi
name=lapce-ai
case "$*" in *mistral-worker*) name=lapce-mistral-worker ;; esac
mkdir -p "$CARGO_TARGET_DIR/release"
printf '%s\\n' '#!/bin/sh' > "$CARGO_TARGET_DIR/release/$name"
chmod +x "$CARGO_TARGET_DIR/release/$name"
''')
            cargo.chmod(0o700)
            for tool in ['nvcc', 'nvidia-smi']:
                path = bin_dir / tool; path.write_text('#!/bin/sh\nexit 0\n'); path.chmod(0o700)
            env = dict(os.environ, PATH=str(bin_dir) + os.pathsep + os.environ['PATH'], BENCH_BUILD_ROOT=str(Path(d, 'builds')), CARGO_TARGET_DIR=str(Path(d, 'ignored')))
            for backend in ['candle', 'llamacpp', 'mistralrs']:
                for device in ['cpu', 'cuda']:
                    result = subprocess.run(['sh', str(ROOT/'build.sh'), backend, device], env=env, capture_output=True, text=True)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertIn(str(Path(d, 'builds', backend + '-' + device, 'release')), result.stdout)
            self.assertEqual(len(list(Path(d, 'builds').glob('*/release/*'))), 6)

    def test_aggregate_excludes_failed_metrics_and_rejects_mixed_config(self):
        collector = module('collect')
        with tempfile.TemporaryDirectory() as d:
            value = dict(state='complete', model_sha256='a' * 64, host={'node':'fixture'}, gpu=None, config='same', provider='candle', scenario='short', exit_code=0, reported={'gen_tps':10})
            # A CPU run has no GPU; explicit null is a valid hardware identity.
            p = Path(d, 'one.json');p.write_text(json.dumps(value))
            bad = dict(value, exit_code=1, reported={'gen_tps':999})
            q = Path(d, 'two.json');q.write_text(json.dumps(bad))
            r = collector.aggregate(d)['providers']['candle']['short']
            self.assertEqual(r['gen_tps']['mean'], 10)
            self.assertEqual(r['failed_runs'], 1)
            self.assertFalse(r['cuda_verified'])
            q.write_text(json.dumps(dict(value, config='different')))
            with self.assertRaises(ValueError):
                collector.aggregate(d)


if __name__ == '__main__':
    unittest.main()
