#!/usr/bin/env python3
"""Capture synthetic composer states. PNG review is a separate acceptance gate."""
import os
import shutil
from pathlib import Path
import subprocess
import sys
import tempfile
import time

binary = str(Path(sys.argv[1]).resolve())
out = Path(sys.argv[2]).resolve()
out.mkdir(parents=True, exist_ok=True)
home = Path(tempfile.mkdtemp(prefix='gravity-gui-', dir='/tmp'))
project = out / 'project'
project.mkdir(exist_ok=True)
source = '<?php\nnamespace App;\nfinal class UserService { public function name(): string { return "Synthetic"; } }\n'
(project / 'example.php').write_text(source)
for index in range(10):
    (project / f'multi_647_{index:02}.php').write_text('<?php // Synthetic mention navigation fixture.\n')
config = home / '.config/lapce-nightly'
config.mkdir(parents=True, exist_ok=True)
(config / 'settings.toml').write_text('[core]\nmodal = false\n[ai-chat]\nenabled = true\nbackend = "local"\nsave-history = false\n[ai-autocomplete]\nenabled = false\nauto-trigger = false\n')
# Only the deterministic local Python fixture runs. PHP and all package
# downloads remain disabled; the composer never contacts a language service.
lsp_log = out / 'navigation-lsp.log'
lsp_fixture = Path(__file__).with_name('navigation-lsp-fixture.py').resolve()
import json
(config / 'language-servers.toml').write_text(
    'enabled = true\nauto-install = false\n[servers.navigation-fixture]\n'
    'languages = ["python"]\ncommand = "python3"\nargs = '
    + json.dumps([str(lsp_fixture), str(lsp_log)]) + '\n')
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
        app = subprocess.Popen([binary, str(project), '--wait', '--new'], env=env, stdout=log, stderr=subprocess.STDOUT)
        window = None
        for _ in range(30):
            time.sleep(1)
            if app.poll() is not None:
                raise RuntimeError(f'Editor exited before GUI capture (status {app.returncode}); see preserved startup logs')
            found = subprocess.run(['xdotool', 'search', '--onlyvisible', '--class', 'lapce'], env=env, text=True, capture_output=True)
            if found.returncode == 0 and found.stdout.strip():
                window = found.stdout.splitlines()[0]
                break
        if window is None:
            raise RuntimeError('No editor window within 30 seconds')
        run('windowsize', window, '1443', '796')
        run('windowmap', '--sync', window)
        time.sleep(5)
        run('windowfocus', '--sync', window)
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', '--delay', '40', 'example.php')
        run('key', 'Return'); time.sleep(2)
        run('key', 'Escape', 'F1'); time.sleep(1)
        run('type', '--clearmodifiers', '--delay', '40', 'toggle ai chat')
        run('key', 'Return'); time.sleep(2)
        shot('00-initial')
        run('mousemove', '1250', '640', 'click', '1')
        shot('00a-empty-focused')
        run('type', '--clearmodifiers', 'Hosted synthetic draft, never send')
        shot('01-draft')
        run('key', 'Tab'); shot('02-mode')
        run('key', 'Down'); shot('03-mode-list')
        run('key', 'Page_Down'); shot('03a-mode-page')
        run('key', 'Home'); shot('03b-mode-home')
        run('key', 'Escape'); run('key', 'Tab', 'Right'); shot('04-model')
        run('key', 'Down'); shot('05-model-list')
        run('key', 'End'); shot('05a-model-end')
        run('key', 'Home'); shot('05b-model-home')
        run('key', 'Escape'); run('key', 'Tab', 'Right', 'Right'); shot('06-send-focus')
        run('key', 'Right'); run('type', '--clearmodifiers', ' after focus return'); shot('07-draft-return')
        run('key', 'shift+Return'); run('type', '--clearmodifiers', 'Second unsent line'); shot('07a-newline')
        run('key', 'shift+Tab'); shot('07b-reverse-send-focus')
        run('key', 'Right')
        run('key', 'ctrl+a'); run('type', '--clearmodifiers', '@example'); time.sleep(3); shot('07c-mention')
        run('key', 'shift+Tab'); shot('07d-mention-blur')
        run('key', 'Right'); time.sleep(1); shot('07e-mention-return')
        run('key', 'F5'); time.sleep(3); shot('07e1-mention-refresh')
        run('key', 'Tab'); shot('07f-mention-insert')
        run('key', 'ctrl+a'); run('type', '--clearmodifiers', '@no_such_synthetic_file_641'); shot('07g-mention-empty')
        run('key', 'shift+Tab'); shot('07h-empty-blur')
        run('key', 'Right'); run('key', 'ctrl+a'); run('type', '--clearmodifiers', '@newly_created_645'); shot('07i-before-new-file')
        added = project / 'newly_created_645.php'
        added_source = '<?php // Synthetic file created after the first index.\n'
        added.write_text(added_source)
        run('key', 'F5'); time.sleep(3); shot('07j-new-file-refreshed')
        run('key', 'Tab'); shot('07k-new-file-inserted')
        if added.read_text() != added_source:
            raise RuntimeError('New synthetic file changed')
        added.unlink()
        run('key', 'ctrl+a'); run('type', '--clearmodifiers', '@newly_created_645')
        run('key', 'F5'); time.sleep(3); shot('07l-deleted-file-refreshed')
        run('key', 'Tab'); shot('07m-deleted-file-tab')
        run('key', 'Escape'); run('key', 'ctrl+a'); run('type', '--clearmodifiers', '@multi_647_'); shot('07n-mention-multi')
        run('key', 'Page_Down'); shot('07o-mention-page')
        run('key', 'End'); shot('07p-mention-end')
        run('key', 'Home'); shot('07q-mention-home')
        run('type', '--clearmodifiers', '09'); shot('07q1-mention-filtered')
        run('key', 'BackSpace', 'BackSpace'); shot('07q2-mention-expanded')
        run('key', 'Tab'); shot('07r-mention-picked')
        run('key', 'ctrl+a'); run('type', '--clearmodifiers', '@multi_647_')
        run('key', 'End', 'Page_Up'); shot('07s-mention-page-up')
        run('key', 'Escape'); shot('07t-mention-dismissed')
        run('type', '--clearmodifiers', '09'); shot('07u-mention-reopened')
        run('key', 'shift+Tab'); shot('07v-mention-reverse-focus')
        run('key', 'Right'); shot('07w-mention-focus-restored')
        run('key', 'Left'); shot('07x-mention-caret-away')
        run('key', 'Right'); shot('07y-mention-caret-return')
        run('key', 'ctrl+a'); shot('07z-mention-selection')
        run('type', '--clearmodifiers', '@example'); shot('07za-mention-replaced')
        run('key', 'Tab'); run('type', '--clearmodifiers', 'caret after inserted path'); shot('07zb-mention-caret-after-insert')
        run('key', 'Escape')
        run('key', 'ctrl+a'); run('type', '--clearmodifiers', 'Final unsent synthetic draft')
        run('mousemove', '600', '220', 'click', '1'); shot('08-outside')
        if (project / 'example.php').read_text() != source:
            raise RuntimeError('Synthetic source changed')
        # Core editing acceptance uses a separate disposable source file.
        # All Return keys below target the file palette, never the composer.
        core_file = project / 'core_pairs.php'
        core_original = '<?php\n()\n()\n'
        core_file.write_text(core_original)
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', 'core_pairs.php')
        run('key', 'Return'); time.sleep(2)
        run('key', 'ctrl+Home', 'Down', 'Right', 'ctrl+alt+Down')
        shot('09-core-pair-carets')
        run('key', 'BackSpace'); shot('09a-core-pairs-deleted')
        run('key', 'ctrl+s'); time.sleep(1)
        if core_file.read_text() != '<?php\n\n\n':
            raise RuntimeError('Multi-caret Backspace did not remove both empty pairs')
        run('key', 'ctrl+z'); shot('09b-core-pairs-undo')
        run('key', 'ctrl+s'); time.sleep(1)
        if core_file.read_text() != core_original:
            raise RuntimeError('One undo did not restore both empty pairs')
        run('key', 'ctrl+shift+z'); shot('09c-core-pairs-redo')
        run('key', 'ctrl+s'); time.sleep(1)
        if core_file.read_text() != '<?php\n\n\n':
            raise RuntimeError('One redo did not delete both empty pairs')
        enter_file = project / 'core_enter.php'
        enter_file.write_text('<?php\n{}\n')
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', 'core_enter.php')
        run('key', 'Return'); time.sleep(2)
        run('key', 'ctrl+Home', 'Down', 'Right'); shot('09d-core-enter-before')
        run('key', 'Return'); shot('09e-core-enter-expanded')
        run('key', 'ctrl+s'); time.sleep(1)
        if enter_file.read_text() != '<?php\n{\n  \n}\n':
            raise RuntimeError('Smart Enter did not expand an empty PHP brace pair')
        run('key', 'ctrl+z'); shot('09f-core-enter-undo')
        run('key', 'ctrl+s'); time.sleep(1)
        if enter_file.read_text() != '<?php\n{}\n':
            raise RuntimeError('One undo did not restore the empty brace pair')
        run('key', 'ctrl+shift+z'); shot('09g-core-enter-redo')
        run('key', 'ctrl+s'); time.sleep(1)
        if enter_file.read_text() != '<?php\n{\n  \n}\n':
            raise RuntimeError('One redo did not restore the expanded PHP brace pair')
        jump_source = project / 'jump_source.php'
        jump_target = project / 'jump_target.php'
        jump_source.write_text("<?php\nrequire './jump_target.php';\n")
        jump_target.write_text("<?php // Ctrl-click destination fixture.\n")
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', 'jump_source.php')
        run('key', 'Return'); time.sleep(2)
        shot('10-file-reference-source')
        run('mousemove', '500', '112', 'keydown', 'ctrl', 'click', '1', 'keyup', 'ctrl')
        time.sleep(2); shot('10a-file-reference-target')
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', 'jump_source.php')
        run('key', 'Return'); time.sleep(2)
        run('mousemove', '500', '112', 'click', '1')
        time.sleep(.08)
        run('keydown', 'ctrl', 'click', '1', 'keyup', 'ctrl')
        time.sleep(2); shot('10b-file-reference-rapid-click')
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', 'jump_source.php')
        run('key', 'Return'); time.sleep(2)
        run('key', 'ctrl+Home', 'Down', 'End')
        run('type', '--clearmodifiers', ' // unsaved navigation check')
        shot('10c-file-reference-after-edit-source')
        run('mousemove', '450', '112', 'keydown', 'ctrl', 'click', '1', 'keyup', 'ctrl')
        time.sleep(2); shot('10d-file-reference-after-edit-target')
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', 'jump_source.php')
        run('key', 'Return'); time.sleep(2)
        # First plain click selects this source tab; a second click on the
        # same point carries a multi-click count while the modifier changes.
        run('mousemove', '440', '112', 'click', '1')
        time.sleep(.08)
        run('keydown', 'ctrl', 'click', '1', 'keyup', 'ctrl')
        time.sleep(2); shot('10e-file-reference-switched-target')
        if jump_source.read_text() != "<?php\nrequire './jump_target.php';\n":
            raise RuntimeError('Navigation unexpectedly saved the edited source')
        if jump_target.read_text() != "<?php // Ctrl-click destination fixture.\n":
            raise RuntimeError('Navigation changed the destination file')
        lsp_source = project / 'lsp_source.py'
        lsp_target = project / 'lsp_target.py'
        lsp_source.write_text('probe()\n# source symbol begins at the same coordinates as target\n')
        lsp_target.write_text("probe = 'Cross-file LSP destination at line 1 column 1'\n")
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', 'lsp_source.py')
        run('key', 'Return'); time.sleep(3)
        shot('11-lsp-equal-coordinates-source')
        run('mousemove', '355', '95', 'keydown', 'ctrl', 'click', '1', 'keyup', 'ctrl')
        time.sleep(3); shot('11a-lsp-equal-coordinates-target')
        events = [json.loads(line) for line in lsp_log.read_text().splitlines()]
        if not any(event['method'] == 'textDocument/definition' for event in events):
            raise RuntimeError('Ctrl-click never reached the local LSP definition fixture')
        if any(event['method'] == 'textDocument/references' for event in events):
            raise RuntimeError('Cross-file coordinate collision incorrectly requested references')
        if lsp_source.read_text() != 'probe()\n# source symbol begins at the same coordinates as target\n':
            raise RuntimeError('LSP navigation changed source bytes')
        delayed_source = project / 'lsp_delayed.py'
        delayed_source.write_text('probe()\n# delayed definition response fixture\n')
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', 'lsp_delayed.py')
        run('key', 'Return'); time.sleep(2)
        run('mousemove', '355', '95', 'keydown', 'ctrl', 'click', '1', 'keyup', 'ctrl')
        # Wait for provider receipt, then change the document before its
        # intentionally delayed reply. The old response must not navigate.
        for _ in range(30):
            delayed_events = [json.loads(line) for line in lsp_log.read_text().splitlines()]
            received = any(event['method'] == 'textDocument/definition'
                and event['params']['textDocument']['uri'].endswith('/lsp_delayed.py')
                for event in delayed_events)
            if received:
                break
            time.sleep(.05)
        if not received:
            raise RuntimeError('Delayed definition fixture never received the click')
        run('key', 'End'); run('type', '--clearmodifiers', ' # unsaved revision after request')
        time.sleep(3); shot('11b-lsp-stale-response-after-edit')
        run('mousemove', '355', '95', 'keydown', 'ctrl', 'click', '1', 'keyup', 'ctrl')
        time.sleep(3); shot('11c-lsp-fresh-response-after-edit')
        delayed_events = [json.loads(line) for line in lsp_log.read_text().splitlines()]
        delayed_requests = [event for event in delayed_events
            if event['method'] == 'textDocument/definition'
            and event['params']['textDocument']['uri'].endswith('/lsp_delayed.py')]
        if len(delayed_requests) != 2:
            raise RuntimeError('Expected both stale and fresh delayed definition requests')
        if delayed_source.read_text() != 'probe()\n# delayed definition response fixture\n':
            raise RuntimeError('Navigation unexpectedly saved the revised LSP source')
        settings_file = config / 'settings.toml'
        initial_settings = settings_file.read_text()
        settings_file.write_text(initial_settings + '\n[editor]\nauto-close-quotes = false\nauto-close-brackets = true\nskip-existing-closing = true\n')
        time.sleep(3)
        pairing_file = project / 'pairing_controls.php'
        pairing_file.write_text('')
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', 'pairing_controls.php')
        run('key', 'Return'); time.sleep(2)
        run('type', '--clearmodifiers', '(')
        run('key', 'ctrl+s'); time.sleep(1); shot('12-pairing-brackets-enabled')
        if pairing_file.read_text() != '()':
            raise RuntimeError('Bracket pairing did not stay enabled independently')
        run('key', 'ctrl+a', 'BackSpace'); run('type', '--clearmodifiers', '"')
        run('key', 'ctrl+s'); time.sleep(1); shot('12a-pairing-quotes-disabled')
        if pairing_file.read_text() != '"':
            raise RuntimeError('Quote pairing setting did not reload or disable insertion')
        settings_file.write_text(initial_settings + '\n[editor]\nauto-close-quotes = true\nauto-close-brackets = true\nskip-existing-closing = false\n')
        time.sleep(3)
        run('key', 'ctrl+a', 'BackSpace'); time.sleep(1); run('type', '--clearmodifiers', '(')
        run('type', '--clearmodifiers', ')')
        run('key', 'ctrl+s'); time.sleep(1); shot('12b-pairing-closer-skip-disabled')
        if pairing_file.read_text() != '())':
            raise RuntimeError('Closer skip setting did not disable skipping independently')
        settings_file.write_text(initial_settings + '\n[editor]\nsmart-enter = false\nsmart-indent = true\n')
        time.sleep(3)
        enter_controls = project / 'enter_controls.php'
        enter_controls.write_text('<?php\n  {}\n')
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', 'enter_controls.php')
        run('key', 'Return'); time.sleep(2)
        run('key', 'ctrl+Home', 'Down', 'End', 'Left', 'Return')
        run('key', 'ctrl+s'); time.sleep(1); shot('12c-smart-enter-disabled-indent-kept')
        if enter_controls.read_text() != '<?php\n  {\n  }\n':
            raise RuntimeError('Disabled Smart Enter should retain indentation without pair expansion')
        settings_file.write_text(initial_settings + '\n[editor]\nsmart-enter = false\nsmart-indent = false\n')
        time.sleep(3)
        run('key', 'ctrl+z'); run('key', 'Return')
        run('key', 'ctrl+s'); time.sleep(1); shot('12d-smart-indent-disabled')
        if enter_controls.read_text() != '<?php\n  {\n}\n':
            raise RuntimeError('Disabled smart indentation should insert a plain newline')
        settings_file.write_text(initial_settings + '\n[editor]\nauto-surround = false\n')
        time.sleep(3)
        literal_file = project / 'quote_literals.php'
        literal_file.write_text('\\"')
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', 'quote_literals.php')
        run('key', 'Return'); time.sleep(2)
        run('key', 'ctrl+Home', 'Right')
        run('type', '--clearmodifiers', '"')
        run('key', 'ctrl+s'); time.sleep(1); shot('12e-escaped-quote-literal')
        if literal_file.read_text() != '\\""':
            raise RuntimeError('Escaped quote was skipped instead of inserted literally')
        run('key', 'ctrl+End', 'ctrl+shift+Home')
        run('type', '--clearmodifiers', ')')
        run('key', 'ctrl+s'); time.sleep(1); shot('12f-selection-replaced-by-closer')
        if literal_file.read_text() != ')':
            raise RuntimeError('Explicit selected range was not replaced by closer')
        context_file = project / 'pairing_context.php'
        context_text = '<?php\n// comment \n$value = "a value";\n\n'
        context_file.write_text(context_text)
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', 'pairing_context.php')
        run('key', 'Return'); time.sleep(3)
        run('key', 'ctrl+Home', 'Down', 'End')
        run('type', '--clearmodifiers', '(')
        run('key', 'ctrl+s'); time.sleep(1); shot('13-pairing-comment-literal')
        expected_context = context_text.replace('// comment ', '// comment (')
        if context_file.read_text() != expected_context:
            raise RuntimeError('Comment context added an unwanted partner')
        run('key', 'ctrl+Home', 'Down', 'Down', 'End', 'Left', 'Left')
        run('type', '--clearmodifiers', '(')
        run('key', 'ctrl+s'); time.sleep(1); shot('13a-pairing-string-literal')
        expected_context = expected_context.replace('a value', 'a value(')
        if context_file.read_text() != expected_context:
            raise RuntimeError('String interior added an unwanted partner')
        run('key', 'ctrl+End')
        run('type', '--clearmodifiers', '(')
        run('key', 'ctrl+s'); time.sleep(1); shot('13b-pairing-code-partner')
        if context_file.read_text() != expected_context + '()':
            raise RuntimeError('Current PHP code context did not add a partner')
        bracket_file = project / 'bracket_highlights.php'
        bracket_text = '<?php\n{[()]}\n'
        bracket_file.write_text(bracket_text)
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', 'bracket_highlights.php')
        run('key', 'Return'); time.sleep(3)
        run('key', 'ctrl+Home', 'Down', 'Home'); shot('14-bracket-opening-highlight')
        run('key', 'Right', 'Right'); shot('14a-bracket-inner-highlight')
        run('key', 'End', 'Left'); shot('14b-bracket-closing-highlight')
        if bracket_file.read_text() != bracket_text:
            raise RuntimeError('Bracket highlight navigation changed source bytes')
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
        # Keep diagnostics from inside the isolated profile as well as
        # the launcher's stdout, especially if graphics startup exits.
        for item in home.rglob('*.log'):
            if item.is_file():
                relative = item.relative_to(home)
                shutil.copyfile(item, out / ('profile-' + '-'.join(relative.parts)))
