#!/usr/bin/env python3
"""Flatten pinned Gutenberg JSON schemas into a completion pack.
Usage: generate-wp-json-schemas.py GUTENBERG_DIR
Only facts are extracted: key paths, value types, enum values and the
public doc URL per file. No schema prose is copied (GPL-licensed text);
the manifest records provenance.
"""
import hashlib, json, re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
COMMIT = '9180415f97d923a80ee576200a3212b0f64c5446'
SOURCE_URL = 'https://github.com/WordPress/gutenberg'
DOC_URLS = {
    'theme': 'https://developer.wordpress.org/block-editor/reference-guides/theme-json-reference/',
    'block': 'https://developer.wordpress.org/block-editor/reference-guides/block-api/block-metadata/',
}

def resolve(node, defs, depth=0):
    """Resolve local $ref and merge allOf/anyOf/oneOf/if-then-else."""
    if depth > 12 or not isinstance(node, dict):
        return {}
    out = {}
    if '$ref' in node:
        m = re.match(r'^#/definitions/(.+)$', node['$ref'])
        if m and m.group(1) in defs:
            out = resolve(defs[m.group(1)], defs, depth + 1)
    for k, v in node.items():
        if k == '$ref':
            continue
        if k in ('allOf', 'anyOf', 'oneOf'):
            for sub in v or []:
                out = merge(out, resolve(sub, defs, depth + 1))
        elif k in ('then', 'else'):
            # union the branches so version-gated keys still complete
            if isinstance(v, dict):
                out = merge(out, resolve(v, defs, depth + 1))
        elif k == 'properties' and 'properties' in out:
            props = dict(out['properties'])
            props.update(v)
            out['properties'] = props
        else:
            out[k] = v
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
        elif k == 'enum' and k in out:
            out[k] = sorted(set(out[k]) | set(v), key=str)
        else:
            out.setdefault(k, v)
    return out

def types_of(node):
    t = node.get('type')
    if isinstance(t, str):
        return [t]
    if isinstance(t, list):
        return [x for x in t if isinstance(x, str)]
    if 'properties' in node:
        return ['object']
    if 'items' in node:
        return ['array']
    if 'enum' in node:
        vals = node['enum']
        return sorted({type(v).__name__.replace('str', 'string').replace('int', 'integer').replace('float', 'number').replace('bool', 'boolean') for v in vals})
    return []

def merge_entry(out, child, sub):
    entry = out.setdefault(child, {})
    types = types_of(sub)
    if types and 'types' not in entry:
        entry['types'] = types
    enum = sub.get('enum')
    if isinstance(enum, list) and enum and all(isinstance(v, (str, int, float, bool)) or v is None for v in enum) and 'enum' not in entry:
        entry['enum'] = enum[:40]
    if 'default' in sub and isinstance(sub['default'], (str, int, float, bool)) and 'default' not in entry:
        entry['default'] = sub['default']

def flatten(schema, path, out, defs, depth=0, ancestry=()):
    if depth > 8:
        return
    refname = None
    if isinstance(schema, dict) and isinstance(schema.get('$ref'), str):
        m = re.match(r'^#/definitions/(.+)$', schema['$ref'])
        if m:
            refname = m.group(1)
            if refname in ancestry:
                # recursive definition (elements inside pseudo selectors):
                # record the entry but do not expand the subtree again
                return
    sub = resolve(schema, defs)
    props = sub.get('properties')
    if isinstance(props, dict):
        in_pseudo = any(seg.startswith(':') for seg in path.split('.'))
        for name, subschema in props.items():
            # breakpoint variants (@mobile/@tablet) mirror the full styles
            # shape; skip them rather than tripling the pack
            if name.startswith('@'):
                continue
            # path segments deeper than this are leaf internals (fontFace
            # fields, selector breakdowns) not completed by hand
            if path.count('.') + 1 >= 7:
                continue
            # pseudo-class subtrees contain the full styles shape again
            # (elements/blocks/pseudos): expanding them recursively explodes
            # combinatorially and matches no real theme.json structure
            if in_pseudo and (name.startswith(':') or name in ('elements', 'blocks')):
                continue
            # per-block settings/styles enumerate every core block with a
            # mostly shared shape: collapse concrete block names to `*`
            child_name = '*' if path.split('.')[-1] == 'blocks' else name
            child = f'{path}.{child_name}' if path else child_name
            resolved = resolve(subschema, defs)
            merge_entry(out, child, resolved)
            flatten(subschema, child, out, defs, depth + 1,
                    ancestry + ((refname,) if refname else ()))
    items = sub.get('items')
    if isinstance(items, dict):
        child = f'{path}.*' if path else '*'
        flatten(items, child, out, defs, depth + 1,
                ancestry + ((refname,) if refname else ()))

def build(gb, name):
    schema = json.loads((gb / 'schemas/json' / f'{name}.json').read_text())
    defs = schema.get('definitions', {})
    root = resolve(schema, defs)
    out = {}
    flatten(root, '', out, defs)
    return out

def main():
    gb = Path(sys.argv[1])
    import subprocess
    actual = subprocess.check_output(['git', '-C', str(gb), 'rev-parse', 'HEAD'], text=True).strip()
    if actual != COMMIT:
        raise SystemExit(f'gutenberg: expected {COMMIT}, got {actual}')
    pack = {}
    inputs = []
    for name in ('theme', 'block'):
        entries = build(gb, name)
        src = (gb / 'schemas/json' / f'{name}.json').read_bytes()
        inputs.append({'path': f'schemas/json/{name}.json', 'sha256': hashlib.sha256(src).hexdigest()})
        pack[name] = {
            'doc': DOC_URLS[name],
            'source_sha256': inputs[-1]['sha256'],
            'entries': dict(sorted(entries.items())),
        }
    out_path = ROOT / 'metadata/web/wp-json-schemas.json'
    out_path.write_text(json.dumps(pack, indent=1, sort_keys=True) + '\n')
    # provenance: key paths, types and enum values are extracted facts;
    # no GPL schema prose is copied into the pack
    manifest_path = ROOT / 'metadata/web/manifest.json'
    manifest = json.loads(manifest_path.read_text())
    manifest = [m for m in manifest if m.get('url') != SOURCE_URL]
    manifest.append(dict(
        framework='wordpress', version='gutenberg-schemas@' + COMMIT[:12],
        commit=COMMIT, url=SOURCE_URL, licence='GPL-2.0-or-later',
        note='fact extraction (paths/types/enum values); schema prose not vendored',
        inputs=sorted(inputs, key=lambda i: i['path'])))
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
    total = sum(len(v['entries']) for v in pack.values())
    print(f'wrote {out_path} with {total} entries '
          f'(theme={len(pack["theme"]["entries"])} block={len(pack["block"]["entries"])})')

if __name__ == '__main__':
    main()
