#!/usr/bin/env python3
"""Flatten pinned Symfony YAML config JSON schemas into a completion pack.
Usage: generate-symfony-yaml-schemas.py SYMFONY_DIR
Only facts are extracted: key paths, value types and enum/boolean values.
The schemas are MIT-licensed (Symfony); the manifest records provenance.
"""
import hashlib, json, re, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
COMMIT = '039017e8635dbc56d1f4bc930c60e5aa45d218e6'
SOURCE_URL = 'https://github.com/symfony/symfony'
FILES = {
    'services': 'src/Symfony/Component/DependencyInjection/Loader/schema/services.schema.json',
    'routing': 'src/Symfony/Component/Routing/Loader/schema/routing.schema.json',
}
DOC_URLS = {
    'services': 'https://symfony.com/doc/current/service_container.html',
    'routing': 'https://symfony.com/doc/current/routing.html',
}
MAX_DEPTH = 10

def resolve(node, defs, root, depth=0):
    """Resolve local $ref (incl. the `when@` self-ref `#`) and union
    allOf/anyOf/oneOf branches so every allowed key completes."""
    if depth > 12 or not isinstance(node, dict):
        return {}
    out = {}
    ref = node.get('$ref')
    if isinstance(ref, str):
        if ref == '#':
            out = resolve(root, defs, root, depth + 1)
        else:
            m = re.match(r'^#/\$defs/(.+)$', ref)
            if m and m.group(1) in defs:
                out = resolve(defs[m.group(1)], defs, root, depth + 1)
    for k, v in node.items():
        if k == '$ref':
            continue
        if k in ('allOf', 'anyOf', 'oneOf'):
            for sub in v or []:
                out = merge(out, resolve(sub, defs, root, depth + 1))
        elif k == 'properties' and 'properties' in out:
            props = dict(out['properties'])
            props.update(v)
            out['properties'] = props
        elif k == 'patternProperties' and 'patternProperties' in out:
            pats = dict(out['patternProperties'])
            pats.update(v)
            out['patternProperties'] = pats
        elif k in ('type', 'enum') and k in out:
            cur = out[k] if isinstance(out[k], list) else [out[k]]
            new = v if isinstance(v, list) else [v]
            out[k] = sorted({str(x) for x in cur} | {str(x) for x in new}) \
                if k == 'type' else sorted(set(cur) | set(new), key=str)
        else:
            out.setdefault(k, v)
    return out

def merge(a, b):
    out = dict(a)
    for k, v in b.items():
        if k == 'properties' and k in out:
            props = dict(out[k])
            for pk, pv in v.items():
                if pk in props and isinstance(props[pk], dict) and isinstance(pv, dict):
                    props[pk] = merge(props[pk], pv)
                else:
                    props[pk] = pv
            out[k] = props
        elif k == 'patternProperties' and k in out:
            pats = dict(out[k])
            pats.update(v)
            out[k] = pats
        elif k in ('type', 'enum') and k in out:
            cur = out[k] if isinstance(out[k], list) else [out[k]]
            new = v if isinstance(v, list) else [v]
            merged = sorted({str(x) for x in cur} | {str(x) for x in new}) \
                if k == 'type' else sorted(set(cur) | set(new), key=str)
            out[k] = merged
        else:
            out.setdefault(k, v)
    return out

def types_of(node):
    t = node.get('type')
    if isinstance(t, str):
        return [t]
    if isinstance(t, list):
        return sorted({str(x) for x in t})
    if 'properties' in node or 'patternProperties' in node:
        return ['object']
    if 'items' in node:
        return ['array']
    if 'enum' in node:
        return sorted({type(v).__name__.replace('str', 'string')
                       .replace('int', 'integer').replace('float', 'number')
                       .replace('bool', 'boolean') for v in node['enum']})
    return []

def pattern_segment(pat):
    """Schema pattern -> pack path segment. `when@<env>` keys keep their
    prefix so completion can match them; anything else is a wildcard."""
    if pat.startswith('^when@'):
        return 'when@*'
    return '*'

def merge_entry(out, path, node):
    entry = out.setdefault(path, {})
    types = sorted(set(entry.get('types', [])) | set(types_of(node)))
    if types:
        entry['types'] = types
    values = set(entry.get('enum', []))
    for v in node.get('enum') or []:
        if isinstance(v, str):
            values.add(v)
    if 'boolean' in types and set(types) <= {'boolean', 'null'}:
        values |= {'true', 'false'}
    if values:
        entry['enum'] = sorted(values)

def flatten(sub, path, out, defs, root, depth=0):
    if depth > MAX_DEPTH or not isinstance(sub, dict):
        return
    node = resolve(sub, defs, root)
    props = node.get('properties')
    if isinstance(props, dict):
        for name, subschema in props.items():
            child = f'{path}.{name}' if path else name
            merge_entry(out, child, resolve(subschema, defs, root))
            flatten(subschema, child, out, defs, root, depth + 1)
    pats = node.get('patternProperties')
    if isinstance(pats, dict):
        for pat, subschema in pats.items():
            seg = pattern_segment(pat)
            child = f'{path}.{seg}' if path else seg
            merge_entry(out, child, resolve(subschema, defs, root))
            flatten(subschema, child, out, defs, root, depth + 1)
    items = node.get('items')
    if isinstance(items, dict):
        child = f'{path}.*' if path else '*'
        flatten(items, child, out, defs, root, depth + 1)

def build(sf, rel):
    schema = json.loads((sf / rel).read_text())
    defs = schema.get('$defs', {})
    out = {}
    flatten(schema, '', out, defs, schema)
    return out

def main():
    sf = Path(sys.argv[1])
    actual = subprocess.check_output(
        ['git', '-C', str(sf), 'rev-parse', 'HEAD'], text=True).strip()
    if actual != COMMIT:
        raise SystemExit(f'symfony: expected {COMMIT}, got {actual}')
    pack = {}
    inputs = []
    for name, rel in FILES.items():
        entries = build(sf, rel)
        src = (sf / rel).read_bytes()
        sha = hashlib.sha256(src).hexdigest()
        inputs.append({'path': rel, 'sha256': sha})
        pack[name] = {
            'doc': DOC_URLS[name],
            'source_sha256': sha,
            'entries': dict(sorted(entries.items())),
        }
    out_path = ROOT / 'metadata/web/symfony-yaml-schemas.json'
    out_path.write_text(json.dumps(pack, indent=1, sort_keys=True) + '\n')
    # provenance: key paths, types and enum values are extracted facts
    manifest_path = ROOT / 'metadata/web/manifest.json'
    manifest = json.loads(manifest_path.read_text())
    manifest = [m for m in manifest
                if not (m.get('url') == SOURCE_URL and 'yaml-schemas' in m.get('version', ''))]
    manifest.append(dict(
        framework='symfony', version='symfony-yaml-schemas@' + COMMIT[:12],
        commit=COMMIT, url=SOURCE_URL, licence='MIT',
        note='fact extraction (paths/types/enum values) from the merged '
             'YAML config JSON schemas (symfony/symfony#61282)',
        inputs=sorted(inputs, key=lambda i: i['path'])))
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
    total = sum(len(v['entries']) for v in pack.values())
    print(f'wrote {out_path} with {total} entries '
          f'(services={len(pack["services"]["entries"])} '
          f'routing={len(pack["routing"]["entries"])})')

if __name__ == '__main__':
    main()
