#!/usr/bin/env python3
"""Summarize compatible measured runs, rejecting mixed comparison identities."""
import argparse
from collections import defaultdict
import csv
import json
import math
from pathlib import Path
import statistics

METRICS = ('load_ms', 'ttft_ms', 'prompt_tps', 'gen_tps', 'tokens', 'cancel_ms')
IDENTITY = ('model_sha256', 'host', 'gpu', 'config', 'measurement_scope')


def stats(values):
    values = [v for v in values if isinstance(v, (int, float)) and not isinstance(v, bool) and math.isfinite(v)]
    if not values:
        return None
    return {'n': len(values), 'mean': statistics.mean(values),
            'stdev': statistics.stdev(values) if len(values) > 1 else None,
            'min': min(values), 'max': max(values)}


def aggregate(directory):
    groups = defaultdict(list)
    identity = None
    for path in sorted(Path(directory).glob('*.json')):
        if path.name == 'summary.json':
            continue
        run = json.loads(path.read_text())
        if run.get('state') != 'complete':
            raise ValueError(f'Incomplete measurement: {path.name}')
        if any(k not in run for k in IDENTITY) or any(run[k] is None for k in ('model_sha256', 'host', 'config')):
            raise ValueError(f'Missing comparison identity: {path.name}')
        current = json.dumps([run[k] for k in IDENTITY], sort_keys=True)
        if identity is not None and identity != current:
            raise ValueError('Different model/hardware/config: split results before comparison')
        identity = current
        groups[(run['provider'], run['scenario'])].append((path, run))
    output = {}
    for (provider, scenario), runs in groups.items():
        successful = [(p, r) for p, r in runs if r['exit_code'] == 0]
        result = {'runs': len(runs), 'successful_runs': len(successful),
                  'under_three_successful_runs': len(successful) < 3,
                  'failed_runs': len(runs) - len(successful),
                  'cuda_verified': False,
                  'cuda_note': 'Device-wide activity is not process-specific inference evidence'}
        reported = [r.get('reported') or {} for _, r in successful]
        for metric in METRICS:
            result[metric] = stats([r.get(metric) for r in reported])
        for field in ('rss_kb', 'vram_mib', 'gpu_util', 'cpu_pct'):
            peaks = []
            for path, _ in successful:
                sample = path.with_suffix('.samples.csv')
                values = []
                if sample.is_file():
                    with sample.open() as file:
                        for row in csv.DictReader(file):
                            try:
                                values.append(float(row[field]))
                            except (KeyError, ValueError, TypeError):
                                continue
                if values:
                    peaks.append(max(values))
            result['peak_' + field] = stats(peaks)
        output.setdefault(provider, {})[scenario] = result
    return {'comparison_identity': json.loads(identity) if identity else None,
            'providers': output}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', nargs='?', default='bench/results')
    args = parser.parse_args()
    result = aggregate(args.directory)
    Path(args.directory, 'summary.json').write_text(json.dumps(result, indent=2))
    print(json.dumps(result, indent=2))
