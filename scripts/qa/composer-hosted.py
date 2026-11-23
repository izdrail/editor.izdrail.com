#!/usr/bin/env python3
"""Capture synthetic composer states. PNG review is a separate acceptance gate."""
import os
from pathlib import Path
import subprocess
import sys
import time

binary = str(Path(sys.argv[1]).resolve())
out = Path(sys.argv[2]).resolve()
out.mkdir(parents=True, exist_ok=True)
home = out / 'home'
project = out / 'project'
project.mkdir(exist_ok=True)
source = '<?php\nnamespace App;\nfinal class UserService { public function name(): string { return "Synthetic"; } }\n'
(project / 'example.php').write_text(source)
config = home / '.config/lapce-nightly'
config.mkdir(parents=True, exist_ok=True)
(config / 'settings.toml').write_text('[core]\nmodal = false\n[ai-chat]\nenabled = true\nbackend = "local"\nsave-history = false\n[ai-autocomplete]\nenabled = false\nauto-trigger = false\n')
(config / 'language-servers.toml').write_text('enabled = false\nauto-install = false\n')
env = os.environ | {'HOME': str(home), 'DISPLAY': ':97', 'XDG_CONFIG_HOME': str(home / '.config'), 'XDG_DATA_HOME': str(home / '.local/share')}
for name in ['LAPCE_LLM_API_KEY', 'OPENAI_API_KEY', 'ANTHROPIC_API_KEY']:
    env.pop(name, None)

def run(*args):
    subprocess.run(['xdotool', *args], env=env, check=True, timeout=10)

def shot(name):
    time.sleep(1)
    subprocess.run(['scrot', '-o', str(out / (name + '.png'))], env=env, check=True, timeout=10)

with (out / 'xvfb.log').open('w') as xlog, (out / 'editor.log').open('w') as log:
    xvfb = subprocess.Popen(['Xvfb', ':97', '-screen', '0', '1500x1000x24'], stdout=xlog, stderr=subprocess.STDOUT)
    app = None
    try:
        time.sleep(2)
        app = subprocess.Popen([binary, str(project), '--wait'], env=env, stdout=log, stderr=subprocess.STDOUT)
        window = None
        for _ in range(30):
            time.sleep(1)
            if app.poll() is not None:
                raise RuntimeError('Editor exited before GUI capture')
            found = subprocess.run(['xdotool', 'search', '--class', 'lapce'], env=env, text=True, capture_output=True)
            if found.returncode == 0 and found.stdout.strip():
                window = found.stdout.splitlines()[0]
                break
        if window is None:
            raise RuntimeError('No editor window within 30 seconds')
        run('windowsize', window, '1443', '796')
        run('windowfocus', window)
        time.sleep(5)
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', '--delay', '40', 'example.php')
        run('key', 'Return'); time.sleep(2)
        run('key', 'Escape', 'F1'); time.sleep(1)
        run('type', '--clearmodifiers', '--delay', '40', 'toggle ai chat')
        run('key', 'Return'); time.sleep(2)
        shot('00-initial')
        run('mousemove', '1250', '640', 'click', '1')
        run('type', '--clearmodifiers', 'Hosted synthetic draft, never send')
        shot('01-draft')
        run('key', 'Tab'); shot('02-mode')
        run('key', 'Down'); shot('03-mode-list')
        run('key', 'Escape'); run('key', 'Tab', 'Right'); shot('04-model')
        run('key', 'Down'); shot('05-model-list')
        run('key', 'Escape'); run('key', 'Tab', 'Right', 'Right'); shot('06-send-focus')
        run('key', 'Right'); run('type', '--clearmodifiers', ' after focus return'); shot('07-draft-return')
        run('mousemove', '600', '220', 'click', '1'); shot('08-outside')
        if (project / 'example.php').read_text() != source:
            raise RuntimeError('Synthetic source changed')
        if app.poll() is not None:
            raise RuntimeError('Editor exited during capture')
        (out / 'result.txt').write_text('Capture completed; source unchanged; no Send activation. Inspect PNGs before visual acceptance.\n')
    finally:
        if app is not None:
            app.terminate()
            try:
                app.wait(timeout=5)
            except subprocess.TimeoutExpired:
                app.kill()
        xvfb.terminate()
        xvfb.wait(timeout=5)
