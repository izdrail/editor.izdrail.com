#!/usr/bin/env python3
"""Loopback-only OpenAI fixture. Never logs headers, keys or request bodies."""
import argparse
import json
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def reply(self, status, value):
        data = json.dumps(value).encode()
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        self.reply(200, {'data': [{'id': 'fixture-model'}]})

    def do_POST(self):
        size = min(int(self.headers.get('Content-Length', '0')), 65536)
        self.rfile.read(size)
        time.sleep(self.server.delay)
        if self.server.status != 200:
            self.reply(self.server.status, {'error': {'message': 'Fixture rejection'}})
        elif self.path == '/api/generate' and self.server.inline:
            # Native Ollama FIM lane consumes one JSON object per line.
            data = (json.dumps({'response': 'fixtureCompletion()', 'done': True}) + '\n').encode()
            self.send_response(200)
            self.send_header('Content-Type', 'application/x-ndjson')
            self.send_header('Content-Length', str(len(data)))
            self.end_headers()
            self.wfile.write(data)
        elif self.path == '/api/generate':
            self.reply(404, {'error': 'Inline fixture is disabled; start with --inline'})
        else:
            self.reply(200, {'choices': [{'message': {'role': 'assistant', 'content': 'Fixture reply'}, 'finish_reason': 'stop'}]})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port', type=int, default=18761)
    parser.add_argument('--status', type=int, choices=[200, 401, 403, 429, 500], default=200)
    parser.add_argument('--delay', type=float, default=0)
    parser.add_argument('--inline', action='store_true', help='Enable canned Ollama /api/generate fixture')
    args = parser.parse_args()
    if not 0 <= args.delay <= 25:
        parser.error('delay must be 0..25 seconds')
    server = ThreadingHTTPServer(('127.0.0.1', args.port), Handler)
    server.status, server.delay, server.inline = args.status, args.delay, args.inline
    print(f'Fixture listening on http://127.0.0.1:{args.port} status={args.status}', flush=True)
    server.serve_forever()


if __name__ == '__main__':
    main()
