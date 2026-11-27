#!/usr/bin/env python3
"""Loopback-only OpenAI fixture. Never logs headers, keys or request bodies."""
import argparse
import json
import time
from pathlib import Path
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
        raw = self.rfile.read(size)
        try:
            request = json.loads(raw)
        except (ValueError, UnicodeDecodeError):
            request = {}
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
        elif self.server.chat_stream and request.get('stream') is True:
            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream')
            self.send_header('Connection', 'close')
            self.end_headers()
            parts = ["Synthetic streamed reply.\n\n```php\n", "<?php\nclass Example {\n", "    public function name(): string { return 'café'; }\n", "}\n```\n\nDone.", " More streamed tail."]
            if self.server.long_code:
                code = "\n".join(f"// Synthetic line {index:02}: " + "long_column_" * 20 + " ENDH" for index in range(45))
                parts = ["Long synthetic code fixture.\n\n```php\n<?php\n" + code + "\n```\n\n" + "\n\n".join(f"Tail paragraph {index}." for index in range(12))]
            # Normal chat fixture closes its PHP card in part3, then part4
            # changes only trailing prose. Long-code mode is a separate case.
            if self.server.advance_file and not self.server.long_code:
                assert len(parts) >= 5 and parts[3].startswith("}\n```")
            try:
                for index, part in enumerate(parts):
                    if index == 4 and self.server.advance_file:
                        deadline = time.monotonic() + 90
                        while not Path(self.server.advance_file).exists():
                            if time.monotonic() >= deadline:
                                print('FIXTURE_ADVANCE_TIMEOUT', flush=True)
                                return
                            time.sleep(.05)
                    data = {'choices': [{'delta': {'content': part}}]}
                    self.wfile.write(('data: ' + json.dumps(data) + '\n\n').encode())
                    self.wfile.flush()
                    time.sleep(self.server.token_delay)
                self.wfile.write(b'data: [DONE]\n\n')
                self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError):
                pass  # Stop closes the fixture connection; this is expected.
            self.close_connection = True
        else:
            self.reply(200, {'choices': [{'message': {'role': 'assistant', 'content': 'Fixture reply'}, 'finish_reason': 'stop'}]})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port', type=int, default=18761)
    parser.add_argument('--status', type=int, choices=[200, 401, 403, 429, 500], default=200)
    parser.add_argument('--delay', type=float, default=0)
    parser.add_argument('--inline', action='store_true', help='Enable canned Ollama /api/generate fixture')
    parser.add_argument('--chat-stream', action='store_true', help='Enable synthetic OpenAI chat SSE')
    parser.add_argument('--token-delay', type=float, default=2, help='Seconds between synthetic chat deltas')
    parser.add_argument('--long-code', action='store_true', help='Large synthetic scroll fixture, requires --chat-stream')
    parser.add_argument('--advance-file', help='Synthetic chat: hold final tail until this file exists, bounded90s')
    args = parser.parse_args()
    if not 0 <= args.token_delay <= 25:
        parser.error('token-delay must be 0..25 seconds')
    if not 0 <= args.delay <= 25:
        parser.error('delay must be 0..25 seconds')
    server = ThreadingHTTPServer(('127.0.0.1', args.port), Handler)
    server.status, server.delay, server.inline = args.status, args.delay, args.inline
    server.chat_stream, server.token_delay = args.chat_stream, args.token_delay
    server.long_code = args.long_code
    server.advance_file = args.advance_file
    print(f'Fixture listening on http://127.0.0.1:{args.port} status={args.status}', flush=True)
    server.serve_forever()


if __name__ == '__main__':
    main()
