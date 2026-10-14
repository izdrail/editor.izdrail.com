#!/usr/bin/env python3
"""Static extraction of pinned permissive declaration sources. Never runs PHP.
Usage: generate-web-metadata.py WP_STUBS TWIG_SOURCE SYMFONY_SOURCE PHPSTORM_STUBS
Not an AST parser: deliberately limited to the audited pinned declaration forms.
"""
import hashlib, json, re, subprocess, sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
SOURCES = [
    ('wordpress', '6.8.3', 'abeb5a8b58fda7ac21f15ee596f302f2959a7114', 'https://github.com/php-stubs/wordpress-stubs', 'MIT'),
    ('twig', '3.21.1', '285123877d4dd97dd7c11842ac5fb7e86e60d81d', 'https://github.com/twigphp/Twig', 'BSD-3-Clause'),
    ('symfony', '7.3.0', '5babe8da13d379288ec141f9b5e325a1a6c39611', 'https://github.com/symfony/symfony', 'MIT'),
    ('php', 'phpstorm-stubs@2049ca3', '2049ca3cfa1feabf52db074108fe776a5713cf02', 'https://github.com/JetBrains/phpstorm-stubs', 'Apache-2.0'),
]
PHP_EXTENSIONS = ['Core', 'standard', 'date', 'json', 'mbstring', 'ctype', 'pcre', 'SPL', 'filter', 'hash', 'random', 'session', 'iconv', 'fileinfo', 'PDO', 'curl', 'openssl']

def stub_declarations(text):
    # phpstorm-stubs are un-namespaced: top-level (depth 0) functions and classes.
    clean = re.sub(r'/\*.*?\*/|//[^\n]*|\#[^\[\n][^\n]*|\'(?:\\.|[^\'\\])*\'|"(?:\\.|[^"\\])*"', lambda m: ' ' * len(m[0]), text, flags=re.S)
    depth = 0
    for m in re.finditer(r'[{}]|^\s*(?:(?:final|abstract|readonly)\s+)*(class|interface|trait|enum)\s+([A-Za-z_]\w*)|\bfunction\s+([A-Za-z_]\w*)\s*\(', clean, re.M):
        if m[0] == '{': depth += 1
        elif m[0] == '}': depth -= 1
        elif depth == 0:
            start = m.start(); end = text.find('\n', start)
            sig = text[start:end if end >= 0 else len(text)].strip().rstrip('{;').strip()
            if m[3]: yield 'function', m[3], sig
            else: yield 'class', m[2], sig


def declarations(text):
    # Strip strings/comments while preserving offsets; then track class braces.
    clean = re.sub(r'/\*.*?\*/|//[^\n]*|\#[^\[\n][^\n]*|\'(?:\\.|[^\'\\])*\'|"(?:\\.|[^"\\])*"', lambda m: ' ' * len(m[0]), text, flags=re.S)
    depth = 0
    namespace = ''
    for m in re.finditer(r'[{}]|\bnamespace\s*([^{};]*)\{|\bfunction\s+([A-Za-z_]\w*)\s*\(', clean):
        if m[0].startswith('namespace'): namespace = m[1].strip(); depth += 1
        elif m[0] == '{': depth += 1
        elif m[0] == '}': depth -= 1
        elif depth == 1 and namespace == '':
            start = m.start(); end = text.find('\n', start)
            signature = text[start:end if end >= 0 else len(text)].strip().rstrip('{;').strip()
            yield m[2], signature

records=[]; manifest=[]
for source, directory in zip(SOURCES, sys.argv[1:]):
    framework, version, commit, url, licence = source
    path=Path(directory)
    actual=subprocess.check_output(['git','-C',str(path),'rev-parse','HEAD'],text=True).strip()
    if actual != commit: raise SystemExit(f'{framework}: expected {commit}, got {actual}')
    inputs=[]
    def read(file):
        data=file.read_bytes(); inputs.append({'path':str(file.relative_to(path)), 'sha256':hashlib.sha256(data).hexdigest()}); return data.decode('utf-8', errors='replace')
    def add(name, kind, package, **extra):
        records.append(dict(name=name, kind=kind, framework=framework, package=package, source_version=version, source_commit=commit, source_url=url, licence=licence, **extra))
    if framework=='wordpress':
        for name,signature in declarations(read(path/'wordpress-stubs.php')):
            add(name,'function','wordpress/core',signature=signature,doc_url='https://developer.wordpress.org/reference/')
    elif framework=='php':
        seen=set()
        for ext in PHP_EXTENSIONS:
            for file in sorted((path/ext).glob('*.php')):
                if file.name.startswith('_'): continue
                for kind,name,signature in stub_declarations(read(file)):
                    if (kind,name) in seen: continue
                    seen.add((kind,name))
                    if kind=='function':
                        add(name,'function','php/'+ext,signature=signature,doc_url='https://www.php.net/manual/en/function.'+name.replace('_','-')+'.php')
                    else:
                        add(name,'class','php/'+ext,qualified_name=name,doc_url='https://www.php.net/manual/en/class.'+name.lower()+'.php')
    elif framework=='twig':
        text=read(path/'src/Extension/CoreExtension.php')
        for kind in ['Filter','Function','Test']:
            for name in sorted(set(re.findall(r"new Twig"+kind+r"\('([^']+)'",text))):
                add(name,kind.lower(),'twig/twig',doc_url='https://twig.symfony.com/doc/3.x/')
        for file in sorted((path/'src/TokenParser').glob('*.php')):
            text=read(file)
            tag=re.search(r"function getTag\(\).*?return '([^']+)'",text,re.S)
            if tag: add(tag[1],'tag','twig/twig',doc_url='https://twig.symfony.com/doc/3.x/')
    else:
        # Public declaration names from component and bundle source, not fixtures.
        for file in sorted((path/'src/Symfony').rglob('*.php')):
            if any(part in ['Tests','Resources','Fixtures'] for part in file.parts): continue
            if file.relative_to(path/'src/Symfony').parts[0] != 'Component': continue
            text=read(file); ns=re.search(r'namespace\s+([^;]+);',text)
            decl=re.search(r'^\s*(?:(?:final|abstract|readonly)\s+)*(class|interface|enum)\s+(\w+)',text,re.M)
            if ns and decl:
                parts=file.relative_to(path/'src/Symfony').parts
                package='symfony/'+re.sub(r'(?<!^)(?=[A-Z])','-',parts[1]).lower()
                add(decl[2],'class',package,qualified_name=ns[1]+'\\'+decl[2],doc_url='https://symfony.com/doc/current/reference/index.html')
    manifest.append(dict(framework=framework,version=version,commit=commit,url=url,licence=licence,inputs=sorted(inputs,key=lambda i:i['path'])))
records.sort(key=lambda r:(r['framework'],r['kind'],r['name'],r.get('qualified_name','')))
out=ROOT/'metadata/web';out.mkdir(parents=True,exist_ok=True)
(out/'symbols.json').write_text(json.dumps(records,indent=2,sort_keys=True)+'\n')
(out/'manifest.json').write_text(json.dumps(manifest,indent=2,sort_keys=True)+'\n')
print({f:sum(r['framework']==f for r in records) for f,*_ in SOURCES})
