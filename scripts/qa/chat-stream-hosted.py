#!/usr/bin/env python3
"""Synthetic delayed-SSE chat UI proof. PNG inspection remains mandatory."""
import csv
import io
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time

binary = str(Path(sys.argv[1]).resolve())
out = Path(sys.argv[2]).resolve()
out.mkdir(parents=True, exist_ok=True)
home = Path(tempfile.mkdtemp(prefix='gravity-chat-home-', dir='/tmp'))
project = Path(tempfile.mkdtemp(prefix='gravity-chat-project-', dir='/tmp'))
advance_file = home / 'advance-stream'
source = '<?php\nclass Example { public function name(): string { return "fixture"; } }\n'
(project / 'example.php').write_text(source)
config = home / '.config/lapce-nightly'
config.mkdir(parents=True)
settings = '[core]\nmodal = false\ncolor-theme = "Lapce Dark"\n[ai-chat]\nenabled = true\nbackend = "ollama"\nendpoint = "http://127.0.0.1:18762"\nmodel = "fixture-model"\nsave-history = false\n[ai-autocomplete]\nenabled = false\nauto-trigger = false\n'
(config / 'settings.toml').write_text(settings)
(config / 'language-servers.toml').write_text('enabled = false\nauto-install = false\n')
env = os.environ | {'LAPCE_CHAT_COPY_TRACE': '1', 'HOME': str(home), 'DISPLAY': ':96', 'XDG_CONFIG_HOME': str(home / '.config'), 'XDG_DATA_HOME': str(home / '.local/share')}
for key in ['LAPCE_LLM_API_KEY', 'OPENAI_API_KEY', 'ANTHROPIC_API_KEY']:
    env.pop(key, None)

def wait_fixture():
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        try:
            with socket.create_connection(('127.0.0.1', 18762), timeout=.2):
                return
        except OSError:
            time.sleep(.1)
    raise RuntimeError('Loopback fixture did not accept connections')

def run(*args):
    subprocess.run(['xdotool', *args], env=env, check=True, timeout=10)

def shot(name):
    path = out / (name + '.png')
    subprocess.run(['scrot', '-o', str(path)], env=env, check=True, timeout=10)
    return path

def words(path, psm=6):
    # Whole-screen OCR misdetects small dim text on a mostly empty canvas.
    # Scale only the right-side chat region and map OCR boxes back to pixels.
    from PIL import Image, ImageOps, ImageStat
    image = Image.open(path)
    geometry = subprocess.check_output(['xdotool', 'getwindowgeometry', '--shell', window], env=env, text=True)
    values = dict(line.split('=', 1) for line in geometry.splitlines() if '=' in line)
    left = max(0, int(values['X']) + int(values['WIDTH']) - 360)
    right = min(image.width, int(values['X']) + int(values['WIDTH']))
    scaled = out / (path.stem + '-ocr.png')
    # Invert the known dark-theme captures to dark glyphs on a light canvas.
    # Preserve normal pixels for light theme; artifacts remain unmodified.
    crop = image.crop((left, 50, right, image.height)).convert('RGB')
    invert = ImageStat.Stat(crop.convert('L')).median[0] < 128
    print(f'OCR shot={path.stem} polarity={"inverted" if invert else "normal"} psm={psm}', flush=True)
    if invert:
        crop = ImageOps.invert(crop)
    crop.resize(((right - left) * 3, (image.height - 50) * 3)).save(scaled)
    data = subprocess.run(['tesseract', str(scaled), 'stdout', '--psm', str(psm), '--dpi', '150', 'tsv'], text=True, capture_output=True, check=True, timeout=15).stdout
    rows = list(csv.DictReader(io.StringIO(data), delimiter='\t'))
    for row in rows:
        row['left'] = str(left + int(row['left']) // 3)
        row['top'] = str(50 + int(row['top']) // 3)
        row['width'] = str(max(1, int(row['width']) // 3))
        row['height'] = str(max(1, int(row['height']) // 3))
        row['psm'] = str(psm)
    return rows

def all_words(path):
    # Sparse composer hints and dense transcript rows need different OCR
    # segmentation. Both must read the same pixels; neither invents a target.
    return words(path, psm=6) + words(path, psm=11)

def text(path):
    return ' '.join(w['text'] for w in all_words(path)).lower()

def wait_text(needle, name, timeout=35):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        path = shot(name)
        if needle.lower() in text(path):
            return path
        time.sleep(.4)
    raise RuntimeError('Missing UI text: ' + needle)

def click_word(path, value, near_y=None):
    candidates = [w for w in words(path) if w['text'].lower() == value.lower()]
    if near_y is not None:
        candidates = [w for w in candidates if abs(int(w['top']) - near_y) < 24]
    if not candidates:
        candidates = [w for w in words(path, psm=11) if w['text'].lower() == value.lower()
                      and (near_y is None or abs(int(w['top']) - near_y) < 24)]
        if candidates:
            print(f'OCR click-target={value} psm=11', flush=True)
    else:
        print(f'OCR click-target={value} psm=6', flush=True)
    if not candidates and value == 'Copy' and near_y is not None:
        # A 1px border confuses page segmentation. Verify a tight single-line
        # crop of the header's right-aligned button, never click unverified UI.
        # This hosted profile keeps the chat panel flush with the right edge.
        from PIL import Image
        geometry = subprocess.check_output(['xdotool', 'getwindowgeometry', '--shell', window], env=env, text=True)
        values = dict(line.split('=', 1) for line in geometry.splitlines() if '=' in line)
        right = int(values['X']) + int(values['WIDTH'])
        crop = out / (path.stem + '-copy-target.png')
        Image.open(path).crop((right - 70, near_y - 4, right - 18, near_y + 17)).resize((312, 126)).save(crop)
        found = subprocess.run(['tesseract', str(crop), 'stdout', '--psm', '7', '--dpi', '150'], text=True, capture_output=True, check=True, timeout=10).stdout.strip().lower()
        if found == 'copy':
            run('mousemove', str(right - 44), str(near_y + 7), 'click', '1')
            return
    if not candidates:
        raise RuntimeError('Missing click target: ' + value)
    w = candidates[0]
    print(f'OCR click={value} psm={w.get("psm", "crop7")}', flush=True)
    run('mousemove', str(int(w['left']) + int(w['width']) // 2), str(int(w['top']) + int(w['height']) // 2), 'click', '1')

def clipboard():
    return subprocess.run(['xclip', '-selection', 'clipboard', '-o'], env=env, text=True, capture_output=True, check=True, timeout=5).stdout

with (out / 'xvfb.log').open('w') as xlog, (out / 'editor.log').open('w') as log, (out / 'fixture.log').open('w') as flog:
    xvfb = subprocess.Popen(['Xvfb', ':96', '-screen', '0', '1500x1000x24'], stdout=xlog, stderr=subprocess.STDOUT)
    fixture = subprocess.Popen([sys.executable, str(Path(__file__).with_name('ai_fixture_server.py')), '--port', '18762', '--chat-stream', '--delay', '8', '--token-delay', '3', '--advance-file', str(advance_file)], stdout=flog, stderr=subprocess.STDOUT)
    app = None
    try:
        time.sleep(2)
        app = subprocess.Popen([binary, str(project), '--wait', '--new'], env=env, stdout=log, stderr=subprocess.STDOUT)
        for _ in range(35):
            time.sleep(1)
            if app.poll() is not None:
                raise RuntimeError('Editor exited during startup')
            found = subprocess.run(['xdotool', 'search', '--onlyvisible', '--class', 'lapce'], env=env, text=True, capture_output=True)
            if found.returncode == 0 and found.stdout.strip():
                window = found.stdout.splitlines()[0]
                break
        else:
            raise RuntimeError('No editor window')
        run('windowsize', window, '1443', '900'); run('windowfocus', '--sync', window)
        time.sleep(5)
        run('key', 'ctrl+p'); time.sleep(1); run('type', '--clearmodifiers', 'example.php'); run('key', 'Return'); time.sleep(2)
        opened = shot('00a-editor-file-open')
        # Editor tab is left of the chat OCR strip, so inspect its own crop.
        from PIL import Image
        Image.open(opened).crop((220, 25, 850, 110)).resize((1260, 170)).save(out / 'editor-tab-ocr.png')
        tab_text = subprocess.run(['tesseract', str(out / 'editor-tab-ocr.png'), 'stdout', '--psm', '11', '--dpi', '150'], text=True, capture_output=True, check=True, timeout=10).stdout.lower()
        if 'example.php' not in tab_text:
            raise RuntimeError('Palette did not open example.php')
        run('key', 'Escape'); shot('00b-picker-dismissed-source-unchanged')
        run('key', 'F1'); time.sleep(1); run('type', '--clearmodifiers', 'toggle ai chat'); run('key', 'Return'); time.sleep(2)
        # Existing composer focus route: panel at right, draft above mode row.
        initial = shot('00-initial')
        placeholders = [w for w in all_words(initial) if w['text'].lower().strip('.,:;') == 'anything' and int(w['top']) > 550]
        if not placeholders:
            raise RuntimeError('Composer placeholder not in right-panel OCR strip, inspect initial pixels/crop')
        w = max(placeholders, key=lambda row: int(row['top']))
        print(f'OCR placeholder psm={w["psm"]}', flush=True)
        run('mousemove', str(int(w['left']) + 10), str(int(w['top']) + 6), 'click', '1')
        run('type', '--clearmodifiers', 'Synthetic chat stream fixture'); run('key', 'Return')
        waiting = wait_text('first token', '01-waiting', timeout=12)
        first = wait_text('Synthetic streamed reply', '02-first-delta', timeout=12)
        if 'done.' in text(first):
            raise RuntimeError('Only final text painted, no incremental proof')
        code = wait_text('public function', '03-partial-code', timeout=15)
        final = wait_text('Done.', '04-complete-dark', timeout=15)
        rows = words(final)
        php = next((w for w in rows if w['text'].lower() == 'php'), None)
        if php is None:
            raise RuntimeError('PHP card header missing')
        click_word(final, 'Copy', near_y=int(php['top']))
        time.sleep(.4)
        shot('04b-after-copy-click')
        expected = "<?php\nclass Example {\n    public function name(): string { return 'café'; }\n}\n"
        if clipboard() != expected:
            raise RuntimeError('Per-code-block Copy changed source bytes')
        shot('05-copy-exact')
        # Explicitly release tail/DONE only after Copy. OCR latency cannot
        # move the target between screenshot and click while the fixture waits.
        advance_file.touch()
        # A later delta only changes prose after the completed code block.
        wait_text('More streamed tail', '05b-tail-focus-retained', timeout=12)
        time.sleep(4)
        if 'generating' in text(shot('05c-done')):
            raise RuntimeError('Loading indicator stayed visible after DONE')
        # CHAT_COPY is emitted with Rust eprintln! (unbuffered stderr).
        writes_before = (out / 'editor.log').read_text().count('CHAT_COPY success=')
        if writes_before != 1:
            raise RuntimeError('Mouse Copy did not write exactly once')
        # Focused Copy must consume held Enter and activate only on release.
        # Mouse Copy focuses its keyboard-navigable label. Do not Tab away:
        # the composer owns a custom traversal ring which does not include
        # transcript controls. Test the retained Copy focus through real keys.
        shot('05d-copy-keyboard-focus')
        subprocess.run(['xclip', '-selection', 'clipboard', '-i'], input='CHAT_COPY_SENTINEL', text=True, env=env, check=True, timeout=5)
        run('keydown', 'Return'); time.sleep(.3); shot('06-copy-held')
        if clipboard() != 'CHAT_COPY_SENTINEL':
            raise RuntimeError('Held Enter wrote clipboard before release')
        if (out / 'editor.log').read_text().count('CHAT_COPY success=') != writes_before:
            raise RuntimeError('Held Enter copied before release')
        run('keyup', 'Return'); time.sleep(.4)
        if (out / 'editor.log').read_text().count('CHAT_COPY success=') != writes_before + 1:
            raise RuntimeError('Focus lost across tail update, or Enter copied more than once')
        if clipboard() != expected:
            raise RuntimeError('Keyboard Copy changed source bytes')
        # Matched-release write proves the preceding mouse click retained label
        # focus. This is not a keyboard-navigation focus-ring claim.
        shot('06b-copy-release-retained-focus')
        # The focus ring itself is a manual pixel gate, not an OCR claim.
        # Leave Copy by keyboard and traverse back through Floem's real Tab
        # order. The predecessor is whole-turn Copy, so negative Space may
        # change clipboard, but must not emit a per-block trace or edit draft.
        # This is distinct from mouse-retained focus tested above.
        run('key', 'shift+Tab'); time.sleep(.3)
        negative_before = shot('06c-before-keyboard-copy')
        if 'synthetic streamed reply' not in text(negative_before):
            raise RuntimeError('Transcript missing after reverse Copy traversal')
        def empty_draft_pixels(path):
            from PIL import Image
            geometry = subprocess.check_output(['xdotool', 'getwindowgeometry', '--shell', window], env=env, text=True)
            values = dict(line.split('=', 1) for line in geometry.splitlines() if '=' in line)
            right = int(values['X']) + int(values['WIDTH'])
            crop = out / (path.stem + '-draft-ocr.png')
            Image.open(path).crop((right - 295, 690, right - 12, 755)).resize((1132, 260)).save(crop)
            draft_text = subprocess.run(['tesseract', str(crop), 'stdout', '--psm', '6', '--dpi', '150'], text=True, capture_output=True, check=True, timeout=10).stdout.lower()
            return 'ask anything' in draft_text
        if not empty_draft_pixels(negative_before):
            raise RuntimeError('Draft is not empty before navigation negative control')
        subprocess.run(['xclip', '-selection', 'clipboard', '-i'], input='CHAT_NAV_NEGATIVE', text=True, env=env, check=True, timeout=5)
        negative_writes = (out / 'editor.log').read_text().count('CHAT_COPY success=')
        run('key', 'space'); time.sleep(.3)
        negative_after = shot('06c2-keyboard-copy-negative')
        if (out / 'editor.log').read_text().count('CHAT_COPY success=') != negative_writes:
            raise RuntimeError('Per-block Copy focus leaked through reverse traversal')
        whole_turn = clipboard()
        if whole_turn == 'CHAT_NAV_NEGATIVE' or whole_turn == expected:
            raise RuntimeError('Negative target did not copy distinct whole-turn bytes')
        if not (whole_turn.startswith('Synthetic streamed reply.') and '```php' in whole_turn and 'More streamed tail.' in whole_turn):
            raise RuntimeError('Negative target clipboard is not the expected whole turn')
        if not empty_draft_pixels(negative_after):
            raise RuntimeError('Negative-control Space changed the empty draft')
        # The visible ring at06d is a manual pixel gate.
        run('key', 'Tab'); time.sleep(.3); shot('06d-keyboard-reached-copy-ring')
        subprocess.run(['xclip', '-selection', 'clipboard', '-i'], input='CHAT_KEYBOARD_NAV_SENTINEL', text=True, env=env, check=True, timeout=5)
        nav_writes = (out / 'editor.log').read_text().count('CHAT_COPY success=')
        run('keydown', 'space'); time.sleep(.3); shot('06e-keyboard-copy-space-held')
        if clipboard() != 'CHAT_KEYBOARD_NAV_SENTINEL':
            raise RuntimeError('Keyboard-reached Copy activated before Space release')
        run('keyup', 'space'); time.sleep(.3); shot('06f-keyboard-copy-space-released')
        if clipboard() != expected or (out / 'editor.log').read_text().count('CHAT_COPY success=') != nav_writes + 1:
            raise RuntimeError('Tab navigation did not reach per-block Copy exactly once')
        (config / 'settings.toml').write_text(settings.replace('Lapce Dark', 'Lapce Light'))
        time.sleep(3); shot('07-complete-light')
        run('windowsize', window, '850', '700'); time.sleep(2); shot('08-narrow-light')
        run('windowsize', window, '1443', '900'); time.sleep(2)
        # Stop during the next first-token wait, via the composer's keyboard path.
        current = shot('09-before-stop')
        replies_before_stop = text(current).count('synthetic streamed reply')
        placeholders = [w for w in all_words(current) if w['text'].lower().strip('.,:;') == 'anything' and int(w['top']) > 550]
        if not placeholders:
            raise RuntimeError('Composer placeholder missing before Stop test')
        w = max(placeholders, key=lambda row: int(row['top']))
        print(f'OCR placeholder psm={w["psm"]}', flush=True)
        run('mousemove', str(int(w['left']) + 10), str(int(w['top']) + 6), 'click', '1')
        run('type', '--clearmodifiers', 'Synthetic cancellation'); run('key', 'Return')
        deadline = time.monotonic() + 12
        while time.monotonic() < deadline:
            running = shot('10-stop-waiting')
            # Tight top Runs-row and bordered-pill crops, since whole-panel
            # light-theme OCR drops the small red Stop label. This case runs
            # in the restored1443x900 light-theme window, panel flush right.
            # Row and pill y-bands overlap intentionally: one active Runs row.
            from PIL import Image
            geometry = subprocess.check_output(['xdotool', 'getwindowgeometry', '--shell', window], env=env, text=True)
            values = dict(line.split('=', 1) for line in geometry.splitlines() if '=' in line)
            right = int(values['X']) + int(values['WIDTH'])
            image = Image.open(running)
            row_crop = out / '10-running-row-target.png'
            pill_crop = out / '10-stop-pill-target.png'
            image.crop((right - 303, 116, right - 8, 151)).resize((1180, 140)).save(row_crop)
            image.crop((right - 45, 132, right - 9, 146)).resize((288, 112)).save(pill_crop)
            row_text = subprocess.run(['tesseract', str(row_crop), 'stdout', '--psm', '6', '--dpi', '150'], text=True, capture_output=True, check=True, timeout=10).stdout.lower()
            pill_text = subprocess.run(['tesseract', str(pill_crop), 'stdout', '--psm', '7', '--dpi', '150'], text=True, capture_output=True, check=True, timeout=10).stdout.lower()
            import re
            if re.search(r'\brunning\b', row_text) and re.search(r'\bstop\b', pill_text):
                break
            time.sleep(.2)
        else:
            raise RuntimeError('Running row with adjacent Stop not visible')
        print('OCR Stop target verified row-psm=6 pill-psm=7', flush=True)
        run('mousemove', str(right - 27), '139', 'click', '1')
        time.sleep(1)
        stopped = shot('11-stopped')
        if 'first token' in text(stopped) or 'generating...' in text(stopped):
            raise RuntimeError('Stop left loading active')
        time.sleep(9)
        stale = shot('12-stale-response-rejected')
        if text(stale).count('synthetic streamed reply') != replies_before_stop:
            raise RuntimeError('Stopped request added a stale streamed reply')
        fixture.terminate(); fixture.wait(timeout=5)
        fixture = subprocess.Popen([sys.executable, str(Path(__file__).with_name('ai_fixture_server.py')), '--port', '18762', '--chat-stream', '--delay', '25', '--token-delay', '3', '--advance-file', str(advance_file)], stdout=flog, stderr=subprocess.STDOUT)
        wait_fixture()
        # Separate keyboard-only cancellation. Focus the draft to type, then
        # use its documented Shift+Tab route to the composer Stop action.
        current = shot('12b-before-keyboard-stop')
        keyboard_replies = text(current).count('synthetic streamed reply')
        placeholders = [w for w in all_words(current) if w['text'].lower().strip('.,:;') == 'anything' and int(w['top']) > 550]
        if not placeholders:
            raise RuntimeError('Composer placeholder missing before keyboard Stop')
        w = max(placeholders, key=lambda row: int(row['top']))
        run('mousemove', str(int(w['left']) + 10), str(int(w['top']) + 6), 'click', '1')
        run('type', '--clearmodifiers', 'Synthetic keyboard cancellation'); run('key', 'Return')
        wait_text('first token', '12c-keyboard-stop-waiting', timeout=12)
        run('key', 'shift+Tab'); time.sleep(.3)
        focused_stop = shot('12d-keyboard-stop-focus-ring')
        if 'to stop' not in text(focused_stop):
            raise RuntimeError('Keyboard Stop focus help missing after Shift+Tab')
        run('key', 'space'); time.sleep(.5)
        keyboard_stopped = shot('12e-keyboard-stopped')
        stopped_text = text(keyboard_stopped)
        if 'first token' in stopped_text or 'generating...' in stopped_text:
            raise RuntimeError('Keyboard Stop left loading active')
        # Beyond the25s first-delta delay, not merely a pre-response check.
        time.sleep(27)
        keyboard_stale = shot('12f-keyboard-stale-rejected')
        if text(keyboard_stale).count('synthetic streamed reply') != keyboard_replies:
            raise RuntimeError('Keyboard-stopped request added stale streamed reply')
        fixture.terminate(); fixture.wait(timeout=5)
        fixture = subprocess.Popen([sys.executable, str(Path(__file__).with_name('ai_fixture_server.py')), '--port', '18762', '--chat-stream', '--long-code', '--token-delay', '0'], stdout=flog, stderr=subprocess.STDOUT)
        wait_fixture()
        # Fresh disposable profile means exactly one code card can exist.
        old_window = window
        app.terminate(); app.wait(timeout=5)
        gone = subprocess.run(['xdotool', 'getwindowname', old_window], env=env, capture_output=True)
        if gone.returncode == 0:
            raise RuntimeError('Original editor window remained after process exit')
        fresh = Path(tempfile.mkdtemp(prefix='gravity-chat-scroll-', dir='/tmp'))
        fresh_config = fresh / '.config/lapce-nightly'
        fresh_config.mkdir(parents=True)
        fresh_settings = settings.replace('Lapce Dark', 'Lapce Light')
        assert 'color-theme = "Lapce Light"' in fresh_settings
        (fresh_config / 'settings.toml').write_text(fresh_settings)
        fresh_project = Path(tempfile.mkdtemp(prefix='gravity-chat-scroll-project-', dir='/tmp'))
        (fresh_project / 'example.php').write_text(source)
        (fresh_config / 'language-servers.toml').write_text('enabled = false\nauto-install = false\n')
        env.update({'HOME': str(fresh), 'XDG_CONFIG_HOME': str(fresh / '.config'), 'XDG_DATA_HOME': str(fresh / '.local/share')})
        app = subprocess.Popen([binary, str(fresh_project), '--wait', '--new'], env=env, stdout=log, stderr=subprocess.STDOUT)
        for _ in range(35):
            time.sleep(1)
            found = subprocess.run(['xdotool', 'search', '--onlyvisible', '--class', 'lapce'], env=env, text=True, capture_output=True)
            if found.returncode == 0 and found.stdout.strip():
                window = found.stdout.splitlines()[0]
                break
        else:
            raise RuntimeError('Fresh scroll-profile window missing')
        run('windowsize', window, '1443', '900'); run('windowfocus', '--sync', window); time.sleep(4)
        run('key', 'F1'); time.sleep(1); run('type', '--clearmodifiers', 'toggle ai chat'); run('key', 'Return'); time.sleep(2)
        current = shot('13-before-long-code')
        if 'synthetic streamed reply' in text(current):
            raise RuntimeError('Fresh scroll profile retained an old conversation')
        placeholders = [w for w in all_words(current) if w['text'].lower().strip('.,:;') == 'anything' and int(w['top']) > 550]
        if not placeholders:
            raise RuntimeError('Composer placeholder missing before scroll test')
        w = max(placeholders, key=lambda row: int(row['top']))
        print(f'OCR placeholder psm={w["psm"]}', flush=True)
        run('mousemove', str(int(w['left']) + 10), str(int(w['top']) + 6), 'click', '1')
        run('type', '--clearmodifiers', 'Long synthetic code fixture'); run('key', 'Return'); time.sleep(3)
        # Remaining crop coordinates assume this restored1443x900 window.
        # Bring the current code header into view, not the composer or editor.
        run('mousemove', '1210', '260'); run('click', '--repeat', '35', '--delay', '30', '4'); time.sleep(1)
        scrolled = shot('14-long-code-top')
        php_headers = [w for w in words(scrolled) if w['text'].lower() == 'php']
        if not php_headers:
            raise RuntimeError('Long code header not visible after transcript scroll')
        header_y = int(php_headers[-1]['top'])
        body_y = min(header_y + 110, 500)
        run('mousemove', '1240', str(body_y))
        first_lines = None
        for step in range(30):
            bottom = shot(f'15-code-wheel-{step:02d}')
            step_headers = [w for w in words(bottom) if w['text'].lower() == 'php']
            step_y = int(step_headers[-1]['top']) if step_headers else None
            print(f'CODE_WHEEL step={step} wheel_x=1240 wheel_y={body_y} initial_header={header_y} current_header={step_y}', flush=True)
            from PIL import Image
            last_crop = out / f'15-last-line-{step:02d}-ocr.png'
            Image.open(bottom).crop((1140, max(header_y + 20, 140), 1435, 675)).resize((1180, (675 - max(header_y + 20, 140)) * 4)).save(last_crop)
            last_text = subprocess.run(['tesseract', str(last_crop), 'stdout', '--psm', '6', '--dpi', '150'], text=True, capture_output=True, check=True, timeout=10).stdout
            import re
            line_numbers = re.findall(r'line\s+(\d+)\b', last_text, re.IGNORECASE)
            at_end = bool(re.search(r'line\s+44\b', last_text, re.IGNORECASE))
            if step == 0:
                first_lines = line_numbers
            if step == 1 and (not line_numbers or line_numbers == first_lines):
                raise RuntimeError('First interior wheel did not change visible code lines')
            if not at_end and (step_y is None or abs(step_y - header_y) >= 8):
                raise RuntimeError('Code header moved before last line was visible')
            if at_end:
                if step < 2:
                    raise RuntimeError('Last code line visible before enough body wheels')
                if step_y is None:
                    raise RuntimeError('Code header missing when last line first visible')
                print(f'CODE_END header_shift={step_y - header_y}', flush=True)
                break
            run('click', '5'); time.sleep(.1)
        else:
            raise RuntimeError('Last code line not reachable by vertical body scrolling')
        headers_after = [w for w in words(bottom) if w['text'].lower() == 'php']
        if not headers_after:
            raise RuntimeError('Code header disappeared before at-end wheel test')
        shutil.copyfile(bottom, out / '15-long-code-body-end.png')
        shutil.copyfile(last_crop, out / '15-last-line-ocr.png')
        run('click', '--repeat', '30', '--delay', '30', '7'); time.sleep(.4)
        sideways = shot('15b-long-line-horizontal-end')
        # Dense repeated lines confuse whole-panel OCR. Read the actual
        # right-edge code pixels where the horizontal end markers appear.
        geometry = subprocess.check_output(['xdotool', 'getwindowgeometry', '--shell', window], env=env, text=True)
        values = dict(line.split('=', 1) for line in geometry.splitlines() if '=' in line)
        right = int(values['X']) + int(values['WIDTH'])
        marker_counts = {}
        for marker_name, marker_shot in [('before', bottom), ('after', sideways)]:
            marker_crop = out / f'15b-horizontal-marker-{marker_name}-ocr.png'
            Image.open(marker_shot).crop((right - 83, step_y + 20, right - 18, 650)).resize((390, (650 - step_y - 20) * 6)).save(marker_crop)
            marker_text = subprocess.run(['tesseract', str(marker_crop), 'stdout', '--psm', '6', '--dpi', '150'], text=True, capture_output=True, check=True, timeout=10).stdout
            marker_counts[marker_name] = len(re.findall(r'\bENDH\b', marker_text, re.IGNORECASE))
        print(f'HORIZONTAL_END markers={marker_counts}', flush=True)
        if marker_counts['before'] != 0:
            raise RuntimeError('Horizontal negative control already contained ending markers')
        if marker_counts['after'] < 2:
            raise RuntimeError('Long-line ending is not reachable by horizontal scroll')
        # The latest code card has twelve tail paragraphs after it, so the
        # transcript still has scroll range. Header must be visible first.
        if not any(w['text'].lower() == 'php' for w in words(sideways)):
            raise RuntimeError('Code header not visible before transcript-wheel test')
        # Baseline against this current screenshot, not the pre-scroll header.
        before_words = words(sideways)
        end_headers = [w for w in before_words if w['text'].lower() == 'php']
        before_tails = [(w['text'].lower(), int(w['top'])) for w in before_words if w['text'].lower() == 'tail']
        end_header_y = int(end_headers[-1]['top'])
        # Extra wheel movement at the code body's end must reach transcript.
        run('click', '--repeat', '20', '--delay', '30', '5'); time.sleep(.6)
        outer = shot('16-code-end-wheel-transcript')
        after_words = words(outer)
        moved_headers = [w for w in after_words if w['text'].lower() == 'php'
                         and int(w['top']) < end_header_y - 8]
        after_tails = [(w['text'].lower(), int(w['top'])) for w in after_words if w['text'].lower() == 'tail']
        if not moved_headers and not (before_tails and after_tails and before_tails != after_tails):
            raise RuntimeError('No positive transcript movement after code-body end wheel')
        if (project / 'example.php').read_text() != source:
            raise RuntimeError('Chat UI changed fixture source')
        (out / 'result.txt').write_text('Synthetic waiting, partial prose, partial code, completion, exact per-block Copy and theme/width captures completed. Actual pixels need inspection. No real model/provider integration claim.\n')
    finally:
        for process in [app, fixture, xvfb]:
            if process is not None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill(); process.wait(timeout=5)
        for item in home.rglob('*.log'):
            if item.is_file():
                shutil.copyfile(item, out / ('profile-' + '-'.join(item.relative_to(home).parts)))
