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
