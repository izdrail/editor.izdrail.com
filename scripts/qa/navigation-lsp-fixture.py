#!/usr/bin/env python3
"""Deterministic stdio LSP fixture, no external service or language packages."""
import json
from pathlib import Path
import sys
import time
from urllib.parse import urlparse, unquote

log = Path(sys.argv[1])

def send(request_id, result):
    body = json.dumps({'jsonrpc': '2.0', 'id': request_id, 'result': result}).encode()
    sys.stdout.buffer.write(b'Content-Length: ' + str(len(body)).encode() + b'\r\n\r\n' + body)
    sys.stdout.buffer.flush()

while True:
    headers = {}
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            sys.exit(0)
        if line == b'\r\n':
            break
        key, value = line.decode().split(':', 1)
        headers[key.lower()] = value.strip()
    message = json.loads(sys.stdin.buffer.read(int(headers['content-length'])))
    method = message.get('method')
    params = message.get('params', {})
    with log.open('a') as stream:
        stream.write(json.dumps({'method': method, 'params': params}) + '\n')
    if method == 'initialize':
        send(message['id'], {'capabilities': {'textDocumentSync': 1, 'definitionProvider': True, 'referencesProvider': True}})
    elif method in ('textDocument/didOpen', 'textDocument/didChange'):
        document = params['textDocument']
        if Path(unquote(urlparse(document['uri']).path)).name == 'diagnostics.py':
            text = document.get('text', '') if method.endswith('didOpen') else params['contentChanges'][-1]['text']
            diagnostics = []
            for token, severity in [('ERROR', 1), ('WARN', 2)]:
                for line, content in enumerate(text.splitlines()):
                    if token in content:
                        column = content.index(token)
                        diagnostics.append({'range': {'start': {'line': line, 'character': column}, 'end': {'line': line, 'character': column + len(token)}}, 'severity': severity, 'source': 'local-ui-fixture', 'message': 'Synthetic ' + token.lower() + ' for UI acceptance'})
            body = json.dumps({'jsonrpc': '2.0', 'method': 'textDocument/publishDiagnostics', 'params': {'uri': document['uri'], 'version': document.get('version', 0), 'diagnostics': diagnostics}}).encode()
            sys.stdout.buffer.write(b'Content-Length: ' + str(len(body)).encode() + b'\r\n\r\n' + body)
            sys.stdout.buffer.flush()
    elif method == 'textDocument/definition':
        source = Path(unquote(urlparse(params['textDocument']['uri']).path))
        if source.name == 'lsp_delayed.py':
            time.sleep(2)
        send(message['id'], {'uri': source.with_name('lsp_target.py').as_uri(), 'range': {'start': {'line': 0, 'character': 0}, 'end': {'line': 0, 'character': 5}}})
    elif method == 'textDocument/references':
        send(message['id'], [])
    elif method == 'exit':
        sys.exit(0)
    elif 'id' in message:
        send(message['id'], None)
