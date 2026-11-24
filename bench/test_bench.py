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
