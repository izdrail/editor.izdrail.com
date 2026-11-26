#!/usr/bin/env python3
"""Forward a real disposable bash PTY and record every UI input byte."""
import os
from pathlib import Path
import pty
import select
import sys
import termios
import tty

audit = Path(sys.argv[1])
audit.mkdir(parents=True, exist_ok=True)
record = audit / (str(os.getpid()) + '.input')
record.write_bytes(b'')
child, master = pty.fork()
if child == 0:
    os.environ['PS1'] = 'SYNTHETIC_PTY> '
    os.execvp('bash', ['bash', '--noprofile', '--norc', '-i'])
original = termios.tcgetattr(sys.stdin.fileno())
try:
    tty.setraw(sys.stdin.fileno())
    with record.open('ab', buffering=0) as log:
        while True:
            ready, _, _ = select.select([sys.stdin.fileno(), master], [], [])
            if sys.stdin.fileno() in ready:
                data = os.read(sys.stdin.fileno(), 4096)
                if not data:
                    break
                log.write(data)
                os.write(master, data)
            if master in ready:
                try:
                    data = os.read(master, 4096)
                except OSError:
                    break
                if not data:
                    break
                os.write(sys.stdout.fileno(), data)
finally:
    termios.tcsetattr(sys.stdin.fileno(), termios.TCSADRAIN, original)
