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
# The native index correctly excludes target/**. Keep the opened workspace
# outside the artifact directory (CI passes target/gui-acceptance as out).
project = Path(tempfile.mkdtemp(prefix='gravity-php-project-', dir='/tmp'))
source = '<?php\nnamespace App;\nfinal class UserService { public function name(): string { return "Synthetic"; } }\n'
(project / 'example.php').write_text(source)
for index in range(10):
    (project / f'multi_647_{index:02}.php').write_text('<?php // Synthetic mention navigation fixture.\n')
config = home / '.config/lapce-nightly'
config.mkdir(parents=True, exist_ok=True)
(config / 'settings.toml').write_text('[core]\nmodal = false\n[ai-chat]\nenabled = true\nbackend = "local"\nsave-history = false\n[ai-autocomplete]\nenabled = false\nauto-trigger = false\n')
# Test-only user binding invokes the existing panel command, not an icon coordinate.
(config / 'keymaps.toml').write_text('[[keymaps]]\nkey = \"ctrl+shift+F9\"\ncommand = \"toggle_source_control_focus\"\n')
# Only the deterministic local Python fixture runs. PHP and all package
# downloads remain disabled; the composer never contacts a language service.
lsp_log = out / 'navigation-lsp.log'
lsp_fixture = Path(__file__).with_name('navigation-lsp-fixture.py').resolve()
import json
(config / 'language-servers.toml').write_text(
    'enabled = true\nauto-install = false\n[servers.navigation-fixture]\n'
    'languages = ["python"]\ncommand = "python3"\nargs = '
    + json.dumps([str(lsp_fixture), str(lsp_log)]) + '\n')
env = os.environ | {'LAPCE_TERMINAL_UI_TRACE': '1', 'LAPCE_SCM_TRACE': '1', 'HOME': str(home), 'DISPLAY': ':97', 'XDG_CONFIG_HOME': str(home / '.config'), 'XDG_DATA_HOME': str(home / '.local/share')}
for name in ['LAPCE_LLM_API_KEY', 'OPENAI_API_KEY', 'ANTHROPIC_API_KEY']:
    env.pop(name, None)

def run(*args):
    subprocess.run(['xdotool', *args], env=env, check=True, timeout=10)

def assert_composer_text(expected):
    # Read, never paste; clipboard belongs to this disposable X server only.
    run('key', 'ctrl+a', 'ctrl+c'); time.sleep(0.3)
    copied = subprocess.run(['xclip', '-selection', 'clipboard', '-o'], env=env, text=True, capture_output=True, check=True, timeout=5).stdout
    if copied != expected:
        raise RuntimeError('Composer draft focus/copy mismatch; inspect preserved PNGs and logs')

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
        # Pointer opening must survive the composer help footer changing height.
        run('mousemove', '1195', '727', 'click', '1'); time.sleep(2); shot('00b-pointer-mode-menu')
        run('mousemove', '600', '220', 'click', '1'); shot('00b1-mode-outside-dismissed')
        run('key', 'Escape'); time.sleep(1)
        run('mousemove', '1290', '727', 'click', '1'); time.sleep(2); shot('00c-pointer-model-menu')
        run('mousemove', '600', '220', 'click', '1'); shot('00c1-model-outside-dismissed')
        # Keep an already-open overlay visible while resizing: this tests
        # live window-bound clamping, not a guessed narrow trigger position.
        run('mousemove', '1290', '727', 'click', '1'); time.sleep(1)
        run('windowsize', window, '850', '640'); shot('00c2-model-menu-narrow')
        run('mousemove', '350', '180', 'click', '1'); shot('00c3-model-narrow-dismissed')
        run('windowsize', window, '1443', '796'); time.sleep(2)
        run('mousemove', '1195', '727', 'click', '1'); time.sleep(1)
        run('windowsize', window, '850', '640'); shot('00b2-mode-menu-narrow')
        run('mousemove', '350', '180', 'click', '1'); shot('00b3-mode-narrow-dismissed')
        run('windowsize', window, '1443', '796'); time.sleep(2)
        run('key', 'Escape'); time.sleep(1)
        run('mousemove', '1250', '675', 'click', '1'); time.sleep(1)
        shot('00a-empty-focused')
        run('type', '--clearmodifiers', 'Hosted synthetic draft, never send')
        shot('01-draft')
        assert_composer_text('Hosted synthetic draft, never send')
        run('key', 'End')
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
        assert_composer_text('Final unsent synthetic draft')
        run('key', 'End')
        run('mousemove', '600', '220', 'click', '1'); shot('08-outside')
        # Core editing acceptance uses a separate disposable source file.
        # Save only this disposable fixture to expose accidental *buffer* edits,
        # which a disk-only unchanged assertion cannot see.
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', 'example.php'); run('key', 'Return'); time.sleep(1)
        run('key', 'ctrl+s'); time.sleep(1)
        if (project / 'example.php').read_text() != source:
            raise RuntimeError('Composer traversal changed the disposable source buffer')
        # Keep the source tab so the Open Editors scenario has two real rows.
        run('mousemove', '410', '51', 'click', '--repeat', '2', '--delay', '100', '1')
        # Explorer file row: pointer focus, then native Tab to the next file.
        # Activate only the disposable fixture, never create/rename/delete.
        run('mousemove', '95', '375', 'click', '1'); shot('00-explorer-row-focused')
        run('key', 'Tab'); shot('00-explorer-next-row-keyboard')
        run('key', 'Return'); shot('00-explorer-keyboard-open')
        # Open Editors row uses a disposable target already opened above.
        run('mousemove', '105', '145', 'click', '1'); shot('00-open-editors-row-focused')
        run('key', 'Tab', 'Tab'); shot('00-open-editors-next-focus')
        run('key', 'shift+Tab', 'Tab'); shot('00-open-editors-return-focus')
        run('key', 'Return'); shot('00-open-editors-keyboard-activate')
        # Existing terminal tab header, never the PTY content area.
        def terminal_events():
            log.flush()
            return [line.split('TERMINAL_UI ', 1)[1].strip() for line in (out / 'editor.log').read_text().splitlines() if 'TERMINAL_UI ' in line]
        start = len(terminal_events())
        run('mousemove', '350', '490', 'mousedown', '1')
        run('mousemove', '600', '220', 'mouseup', '1'); time.sleep(0.5)
        shot('00-terminal-header-focus')
        headers = [e for e in terminal_events()[start:] if e.startswith('event=focus:header:')]
        if not headers:
            raise RuntimeError('Terminal header focus seed missed')
        header_identity = headers[-1]
        run('key', 'Tab'); shot('00-terminal-header-next-focus')
        if not terminal_events()[-1].startswith('event=focus:close:'):
            raise RuntimeError('Terminal header Tab missed close')
        close_identity = terminal_events()[-1]
        run('key', 'shift+Tab'); shot('00-terminal-header-return-focus')
        if terminal_events()[-1] != header_identity:
            raise RuntimeError('Terminal reverse traversal did not return to same header')
        run('key', 'Return'); shot('00-terminal-header-keyboard-activate')
        run('key', 'space'); time.sleep(0.5)
        events = terminal_events()[start:]
        if events != [header_identity, close_identity, header_identity, 'event=activate:header', 'event=activate:header']:
            raise RuntimeError('Terminal header sequence or activation isolation failed')
        (out / 'terminal-keyboard-assertions.log').write_text('same_header_reverse=true\nclose_not_activated=true\nenter_space_header_only=true\n' + '\n'.join(events))
        # All Return keys below target the file palette, never the composer.
        core_file = project / 'core_pairs.php'
        core_original = '<?php\n()\n()\n'
        core_file.write_text(core_original)
        # Terminal owns ordinary typing after header activation. Reacquire
        # the editor before invoking its file palette, never type a path at PTY.
        run('mousemove', '600', '220', 'click', '1'); time.sleep(0.5)
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
        run('key', 'Right'); shot('14c-bracket-after-closing-highlight')
        run('key', 'ctrl+shift+backslash'); shot('14d-bracket-jump-from-adjacent')
        # The palette invokes the same existing movement without relying on
        # keyboard-layout translation of the backslash accelerator.
        run('key', 'F1'); time.sleep(1)
        run('type', '--clearmodifiers', 'match pairs'); shot('14e-match-pairs-command')
        run('key', 'Return'); time.sleep(1); shot('14f-match-pairs-command-result')
        run('key', 'ctrl+shift+backslash'); shot('14e-bracket-jump-back')
        run('key', 'End', 'shift+Left'); shot('14f-shift-arrow-still-selects')
        run('key', 'ctrl+comma'); time.sleep(3); shot('16-settings-dark-wide')
        run('type', '--clearmodifiers', 'zz_no_setting_677')
        shot('16a-settings-no-results')
        run('key', 'ctrl+a'); run('type', '--clearmodifiers', 'font size')
        shot('16b-settings-filtered-sections')
        run('key', 'ctrl+a'); run('type', '--clearmodifiers', 'editor')
        shot('16c-settings-section-search')
        run('windowsize', window, '850', '640'); time.sleep(2)
        shot('16d-settings-dark-narrow')
        run('key', 'ctrl+a', 'BackSpace')
        shot('16e-settings-dark-narrow-reset')
        settings_file.write_text(settings_file.read_text().replace('[core]\n', '[core]\ncolor-theme = "Lapce Light"\n', 1))
        time.sleep(3); shot('16f-settings-light-narrow')
        run('windowsize', window, '1443', '796'); time.sleep(2)
        shot('16g-settings-light-wide')
        run('type', '--clearmodifiers', 'auto close quotes')
        shot('17-settings-toggle-filtered')
        run('key', 'Tab'); shot('17a-settings-toggle-keyboard-focus')
        run('key', 'space'); time.sleep(2); shot('17b-settings-toggle-keyboard-off')
        if 'auto-close-quotes = false' not in settings_file.read_text():
            raise RuntimeError('Keyboard settings toggle did not persist false')
        # Persistence reload rebuilds the boolean row, so reacquire the
        # visible search field before testing an independent dropdown.
        run('mousemove', '650', '99', 'click', '1')
        run('key', 'ctrl+a'); run('type', '--clearmodifiers', 'keymap preset')
        shot('17c-settings-dropdown-filtered')
        run('key', 'Tab', 'space'); shot('17d-settings-dropdown-keyboard-open')
        run('key', 'Down'); shot('17e-settings-dropdown-keyboard-moved')
        run('key', 'Escape'); shot('17f-settings-dropdown-keyboard-cancel')
        run('mousemove', '650', '99', 'click', '1')
        run('key', 'ctrl+a'); run('type', '--clearmodifiers', 'lexer')
        shot('17g-lexer-setting-default')
        run('key', 'Tab', 'space'); shot('17h-lexer-setting-options')
        run('key', 'Down', 'Return'); time.sleep(2)
        shot('17i-lexer-setting-lexilla')
        if 'lexer = "lexilla"' not in settings_file.read_text():
            raise RuntimeError('Lexer dropdown did not persist Lexilla')
        run('mousemove', '650', '99', 'click', '1')
        run('key', 'ctrl+a'); run('type', '--clearmodifiers', 'lexer')
        run('key', 'Tab', 'space', 'Up', 'Return'); time.sleep(2)
        shot('17j-lexer-setting-logos-restored')
        if 'lexer = "logos"' not in settings_file.read_text():
            raise RuntimeError('Lexer dropdown did not persist Logos')

        settings_file.write_text(settings_file.read_text().replace('color-theme = "Lapce Light"', 'color-theme = "Lapce Dark"'))
        time.sleep(3); shot('18-composer-footer-dark-wide')
        run('mousemove', '1250', '660', 'click', '1')
        run('type', '--clearmodifiers', 'Footer wrapping preview only, never send')
        shot('18b-composer-footer-focused-wide')

        run('windowsize', window, '850', '640'); time.sleep(2)
        shot('18a-composer-footer-dark-narrow')
        run('mousemove', '690', '505', 'click', '1')
        shot('18c-composer-footer-focused-narrow')
        run('windowsize', window, '1443', '796'); time.sleep(2)
        run('mousemove', '650', '99', 'click', '1')
        run('key', 'ctrl+a'); run('type', '--clearmodifiers', 'font size')
        shot('19-settings-search-before-section')
        run('mousemove', '300', '236', 'click', '1'); time.sleep(2)
        shot('19a-settings-section-clears-search')
        run('mousemove', '650', '99', 'click', '1')
        run('type', '--clearmodifiers', 'keymap preset')
        run('key', 'Tab', 'space', 'Down', 'Return'); time.sleep(2)
        shot('20-settings-dropdown-accepted')
        if 'keymap-preset = "vscode"' not in settings_file.read_text():
            raise RuntimeError('Dropdown keyboard acceptance did not persist vscode')
        run('mousemove', '650', '99', 'click', '1')
        run('key', 'ctrl+a'); run('type', '--clearmodifiers', 'editor: font size')
        time.sleep(1); shot('21-settings-numeric-before')
        numeric_settings_before = settings_file.read_text()
        run('key', 'Tab')
        run('key', 'ctrl+a'); run('type', '--clearmodifiers', 'invalid-number')
        time.sleep(1); shot('21a-settings-numeric-invalid')
        if settings_file.read_text() != numeric_settings_before:
            raise RuntimeError('Invalid numeric setting changed persisted settings')
        # A read-only isolated settings file exercises real write failures.
        # Restore permissions even when the acceptance assertion fails.
        settings_file.chmod(0o444)
        try:
            run('mousemove', '650', '99', 'click', '1')
            run('key', 'ctrl+a'); run('type', '--clearmodifiers', 'auto close quotes')
            time.sleep(1); run('key', 'Tab', 'space')
            time.sleep(1); shot('22-settings-toggle-save-failed')
            if settings_file.read_text() != numeric_settings_before:
                raise RuntimeError('Failed toggle write changed settings')
            run('mousemove', '650', '99', 'click', '1')
            run('key', 'ctrl+a'); run('type', '--clearmodifiers', 'keymap preset')
            time.sleep(1); run('key', 'Tab', 'space', 'Home', 'Return')
            time.sleep(1); shot('22a-settings-dropdown-save-failed')
            if settings_file.read_text() != numeric_settings_before:
                raise RuntimeError('Failed dropdown write changed settings')
        finally:
            settings_file.chmod(0o644)
        # Escape returns to the menu trigger, preserving a predictable path.
        run('mousemove', '650', '14', 'click', '1'); run('key', 'Escape')
        # The palette owns focus after Escape; Tab starts toolbar traversal.
        run('key', 'Tab'); shot('23-toolbar-main-menu-focus')
        run('key', 'space'); shot('23a-toolbar-main-menu-open')
        run('key', 'Escape'); shot('26-main-menu-return-focus')
        run('key', 'Tab'); shot('26a-remote-menu-focus')
        run('key', 'space'); shot('26b-remote-menu-open')
        run('key', 'Escape'); shot('26c-remote-menu-return-focus')
        # Keyboard menu navigation only. Never activate a remote connection.
        run('key', 'space', 'Down'); shot('27-remote-menu-selected')
        run('key', 'Escape', 'shift+Tab', 'space', 'Down', 'Right')
        shot('27a-main-submenu-keyboard')
        run('key', 'Left'); shot('27b-main-submenu-back')
        run('key', 'Right', 'Down'); shot('28-menu-skips-separator')
        run('key', 'Right'); shot('28a-settings-submenu')
        run('key', 'End'); shot('28b-keyboard-shortcuts-selected')
        run('key', 'Return'); time.sleep(2); shot('28c-keyboard-shortcuts-activated')
        run('key', 'ctrl+shift+f'); time.sleep(2)
        run('type', '--clearmodifiers', 'zz_no_search_match_2026')
        time.sleep(2); shot('29-search-no-results')
        run('key', 'ctrl+a', 'BackSpace'); time.sleep(1); shot('29a-search-empty')
        run('type', '--clearmodifiers', 'Synthetic'); time.sleep(2)
        shot('30-search-matches')
        run('mousemove', '650', '499', 'click', '1')
        run('key', 'ctrl+a'); run('type', '--clearmodifiers', '[')
        run('key', 'Tab', 'Tab', 'Tab'); shot('31-search-regex-keyboard-focus')
        run('key', 'space'); time.sleep(2); shot('31a-search-regex-toggle')
        run('mousemove', '650', '499', 'click', '1')
        run('key', 'ctrl+a'); run('type', '--clearmodifiers', '(')
        time.sleep(2); shot('30a-search-invalid-regex')
        run('mousemove', '650', '499', 'click', '1')
        run('key', 'ctrl+a', 'BackSpace'); time.sleep(1)
        shot('30b-search-invalid-cleared')
        run('type', '--clearmodifiers', 'Synthetic')
        time.sleep(2); run('key', 'Tab', 'Tab', 'Tab', 'Tab')
        shot('32-search-file-keyboard-focus')
        run('key', 'space'); shot('32a-search-file-keyboard-collapsed')
        run('key', 'space', 'Tab'); shot('32b-search-result-keyboard-focus')
        run('key', 'Return'); time.sleep(2); shot('32c-search-result-keyboard-opened')
        run('key', 'Escape')
        run('mousemove', '120', '93', 'click', '1')
        shot('33-open-editors-header-collapsed')
        run('key', 'space'); shot('33a-open-editors-header-keyboard-expanded')
        run('key', 'space'); shot('33b-open-editors-header-keyboard-collapsed')
        (project / 'search_long.php').write_text('<?php // ' + 'é' * 160 + ' UNIQUE_LONG_MATCH ' + 'z' * 160 + '\n')
        run('mousemove', '650', '499', 'click', '1')
        run('key', 'ctrl+a'); run('type', '--clearmodifiers', 'UNIQUE_LONG_MATCH')
        time.sleep(2); shot('34-search-long-preview')
        run('key', 'Tab', 'Tab', 'Tab', 'Tab', 'Tab')
        shot('34a-search-long-keyboard-focus')
        run('key', 'Return'); time.sleep(2); shot('34b-search-long-column-destination')
        run('key', 'Escape'); shot('34c-search-long-destination-unobscured')
        run('mousemove', '1377', '783', 'click', '1')
        time.sleep(1); shot('35-status-line-ending-palette')
        run('key', 'Escape'); time.sleep(1)
        run('key', 'Tab'); shot('35a-status-language-keyboard-focus')
        run('key', 'Return'); time.sleep(1); shot('35b-status-language-palette')
        run('key', 'Escape')
        time.sleep(1); run('key', 'shift+Tab', 'shift+Tab')
        shot('36-status-cursor-keyboard-focus')
        run('key', 'Return'); time.sleep(1); shot('36a-status-goto-line-palette')
        run('key', 'Escape'); time.sleep(1)
        run('key', 'shift+Tab', 'shift+Tab', 'shift+Tab', 'shift+Tab')
        shot('37-status-diagnostics-keyboard-focus')
        run('key', 'Return'); time.sleep(1); shot('37a-status-problems-panel')
        run('key', 'shift+Tab'); shot('38-status-branch-keyboard-focus')
        run('key', 'Return'); time.sleep(1); shot('38a-status-branch-palette')
        run('key', 'Escape'); time.sleep(1); shot('38b-status-branch-focus-restored')
        run('key', 'Tab', 'Return'); time.sleep(1); shot('39-problems-empty-guidance')
        (project / 'related_target.py').write_text('# first related target\n# middle\n#   second related target\n')
        (project / 'diagnostics.py').write_text('# ERROR synthetic error\n# WARN synthetic warning\n')
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', 'diagnostics.py'); run('key', 'Return')
        time.sleep(3); shot('40-problems-published')
        run('mousemove', '820', '518', 'click', '1'); shot('41-problems-warning-group-collapsed')
        run('key', 'space'); shot('41a-problems-warning-group-reopened')
        run('key', 'Tab'); shot('41b-problems-warning-keyboard-focus')
        run('key', 'Return'); time.sleep(1); shot('41c-problems-warning-destination')
        run('mousemove', '920', '540', 'click', '1')
        run('key', 'Tab'); shot('42-problems-related-first-focus')
        run('key', 'Tab'); shot('42a-problems-related-second-focus')
        run('key', 'Return'); time.sleep(1); shot('42b-problems-related-destination')
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', 'diagnostics.py'); run('key', 'Return'); time.sleep(1)


        run('key', 'ctrl+a'); run('type', '--clearmodifiers', '# cleared synthetic diagnostics')
        time.sleep(3); shot('40a-problems-cleared')

        for relative, contents in {
            'app/Models/Post.php': '<?php\nnamespace App\\Models;\nclass Post {}\n',
            'app/Services/ReceiptService.php': '<?php\nnamespace App\\Services;\nclass ReceiptService {}\n',
            'routes/web.php': "<?php\nRoute::get('/users', fn () => null)->name('users.index');\nRoute::get('/named', fn () => null)->name('native.example');\n",
            'model_usage.php': '<?php\nPost::class;\n',
            'class_usage.php': '<?php\nReceiptService::class;\n',
            'route_usage.php': "<?php\nroute('users.index');\n",
        }.items():
            fixture = project / relative
            fixture.parent.mkdir(parents=True, exist_ok=True)
            fixture.write_text(contents)
        for source_name, x, capture in [('model_usage.php', 355, '44-model'), ('class_usage.php', 380, '45-class'), ('route_usage.php', 420, '46-route')]:
            run('key', 'ctrl+p'); time.sleep(1)
            run('type', '--clearmodifiers', source_name); run('key', 'Return'); time.sleep(2)
            shot(capture + '-source')
            run('mousemove', str(x), '112', 'keydown', 'ctrl', 'click', '1', 'keyup', 'ctrl')
            time.sleep(2); shot(capture + '-new-tab-target')

        (project / 'model_import_usage.php').write_text('<?php\nuse App\\Models\\Post;\nPost::query();\n')
        (project / 'class_import_usage.php').write_text('<?php\nuse App\\Services\\ReceiptService;\nnew ReceiptService();\n')
        for source_name, x, capture in [('model_import_usage.php', 355, '47-imported-model'), ('class_import_usage.php', 410, '48-imported-class')]:
            run('key', 'ctrl+p'); time.sleep(1)
            run('type', '--clearmodifiers', source_name); run('key', 'Return'); time.sleep(2)
            shot(capture + '-source')
            run('mousemove', str(x), '130', 'keydown', 'ctrl', 'click', '1', 'keyup', 'ctrl')
            time.sleep(2); shot(capture + '-destination-or-failure')
        # A fresh target outside Laravel's conventional directories proves that
        # composer mapping and import aliases work, not a basename search.
        (project / 'composer.json').write_text(json.dumps({'autoload': {'psr-4': {'App\\': 'app/', 'Domain\\': 'domain/'}}}))
        (project / 'domain').mkdir(exist_ok=True)
        (project / 'domain/Billing.php').write_text('<?php\nnamespace Domain;\nclass Billing {}\n')
        (project / 'alias_usage.php').write_text('<?php\nuse Domain\\Billing as Invoice;\nnew Invoice();\n')
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', 'alias_usage.php'); run('key', 'Return'); time.sleep(2)
        shot('49-composer-alias-source')
        run('mousemove', '395', '130', 'keydown', 'ctrl', 'click', '1', 'keyup', 'ctrl')
        time.sleep(2); shot('49a-composer-alias-new-target')
        # Laravel static detection requires its composer requirement on disk.
        composer_file = project / 'composer.json'
        composer_data = json.loads(composer_file.read_text())
        composer_data['require'] = {'laravel/framework': '^12.0'}
        composer_file.write_text(json.dumps(composer_data))
        (project / 'snippet_tab.php').write_text('<?php\n')
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', 'snippet_tab.php'); run('key', 'Return'); time.sleep(2)
        run('key', 'ctrl+End'); run('type', '--clearmodifiers', 'lv-livewire-component'); run('key', 'ctrl+space'); time.sleep(2)
        shot('51-snippet-completion-before-tab')
        run('key', 'Tab'); time.sleep(2); shot('51a-snippet-expanded-after-tab')
        run('key', 'Escape')
        comment_text = '<?php\n/** Documentation\n * Details to collapse\n */\n// First line comment\n// Second line comment\n# First hash comment\n# Second hash comment\n/* Block comment\nMore details\n*/\nfunction visible_after_comments() {}\n'
        (project / 'comment_folds.php').write_text(comment_text)
        run('key', 'ctrl+p'); time.sleep(1)
        run('type', '--clearmodifiers', 'comment_folds.php'); run('key', 'Return'); time.sleep(2)
        shot('50-comments-expanded')
        run('key', 'F1'); time.sleep(1); run('type', '--clearmodifiers', 'Fold All'); run('key', 'Return'); time.sleep(2)
        shot('50a-comments-folded')
        run('key', 'F1'); time.sleep(1); run('type', '--clearmodifiers', 'Unfold All'); run('key', 'Return'); time.sleep(2)
        shot('50b-comments-unfolded')
        if (project / 'comment_folds.php').read_text() != comment_text:
            raise RuntimeError('Folding changed comment source bytes')
        run('windowsize', window, '850', '640'); time.sleep(2); shot('43-problems-empty-narrow-dark')
        settings_file.write_text(settings_file.read_text().replace('color-theme = "Lapce Dark"', 'color-theme = "Lapce Light"'))
        time.sleep(3); shot('43a-problems-empty-narrow-light')
        run('windowsize', window, '1443', '796'); time.sleep(2); shot('43b-problems-empty-wide-light')
        settings_file.write_text(settings_file.read_text().replace('color-theme = "Lapce Light"', 'color-theme = "Lapce Dark"'))
        time.sleep(3); shot('43c-problems-empty-wide-dark')














        # Native PHP completion, PHP LSP deliberately absent in this profile.
        native_cases = [
            ('52-native-this', '<?php\nnamespace App; class NativeFixture { public function nativeSave() {} public function run() { $this->'),
            ('52a-native-new', '<?php\nnamespace App; class NativeModel {}\nnew '),
            ('52b-native-use', '<?php\nnamespace App; class NativeModel {}\nuse '),
            ('52c-project-route-string', "<?php\nroute('na"),
            ('52d-html-region', '<html><script>obj->'),
        ]
        for capture, text in native_cases:
            fixture = project / (capture + '.php')
            fixture.write_text(text)
            run('key', 'Escape', 'ctrl+p'); time.sleep(0.6)
            run('type', '--clearmodifiers', fixture.name); run('key', 'Return'); time.sleep(1.5)
            run('key', 'ctrl+End', 'ctrl+space'); time.sleep(2)
            shot(capture)
            run('key', 'Escape')
            if fixture.read_text() != text:
                raise RuntimeError('Native completion capture changed source without acceptance')

        # Apply readable diagnostic text only after existing settings/coordinate
        # fixtures. Their temporary [editor] tables must not become duplicates.
        settings_file.write_text(initial_settings + '\n[editor]\nfont-size = 15\nenable-error-lens = true\nerror-lens-font-size = 15\n')
        time.sleep(3)
        # P4 typed-receiver and conservative local-diagnostic checks, no PHP LSP.
        scope_cases = [
            ('53-typed-parameter', '<?php\nnamespace App; class ScopeUser {public function name(){}} function run(ScopeUser $user) {$user->na;}', 2),
            ('53a-inferred-new', '<?php\nnamespace App; class ScopeUser {public function name(){}} function run() {$user=new ScopeUser(); $user->na;}', 2),
            ('53b-phpdoc-var', '<?php\nnamespace App; class ScopeUser {public function name(){}} function run() {/** @var ScopeUser $user */ $user=find(); $user->na;}', 2),
            ('53c-scope-diagnostics', '<?php\nfunction localCheck() {\n    $unused=1;\n    echo $missing;\n    return;\n    echo 2;\n}\n', 0),
        ]
        for capture, text, back in scope_cases:
            fixture = project / (capture + '.php')
            fixture.write_text(text)
            run('key', 'Escape', 'ctrl+p'); time.sleep(0.6)
            run('type', '--clearmodifiers', fixture.name); run('key', 'Return'); time.sleep(1.5)
            run('key', 'ctrl+End')
            if back:
                run('key', *(['Left'] * back)); run('key', 'ctrl+space')
            time.sleep(2); shot(capture)
            # Read rendered pixels, not source labels or successful key dispatch.
            image = out / (capture + '.png')
            from PIL import Image
            ocr_image = out / (capture + '-ocr.png')
            pixels = Image.open(image)
            if back:
                pixels.crop((219, 76, 1000, 412)).resize((1562, 672)).save(ocr_image)
            else:
                # Warnings render in the Problems panel when inline error-lens
                # styling is hidden. Assert the actual visible diagnostic there.
                pixels.crop((628, 412, 1000, 675)).resize((1116, 789)).save(ocr_image)
            rendered = subprocess.run(['tesseract', str(ocr_image), 'stdout', '--psm', '11'], env=env, text=True, capture_output=True, check=True, timeout=20).stdout.lower()
            expected = 'native php' if back else 'undefined local variable'
            if expected not in rendered:
                raise RuntimeError('P4 rendered UI assertion failed: ' + capture + ': expected ' + expected)
            run('key', 'Escape')
            if fixture.read_text() != text:
                raise RuntimeError('Scope capture changed source without acceptance')

        # P5 same-index Laravel scopes/facade members. Non-conventional domain
        # root prevents the old app/Models filename scanner satisfying the test.
        (project / 'domain').mkdir(exist_ok=True)
        composer_data = json.loads(composer_file.read_text())
        composer_data.setdefault('require', {})['laravel/framework'] = '^12.0'
        composer_file.write_text(json.dumps(composer_data))
        model_text = '<?php\nnamespace Domain;\nclass Ledger extends \\Illuminate\\Database\\Eloquent\\Model {\n    public function scopeSettled($query) { return $query; }\n}\n'
        (project / 'domain/Ledger.php').write_text(model_text)
        facade_text = '<?php\nnamespace Domain;\n/** @method static string deliver(string $text) */\nclass Relay extends \\Illuminate\\Support\\Facades\\Facade {}\n'
        (project / 'domain/Relay.php').write_text(facade_text)
        # A real native facade contract overlaps the bundled Cache pack.
        (project / 'domain/Cache.php').write_text('<?php\nnamespace Illuminate\\Support\\Facades;\n/** @method static mixed get(string $key) */\nclass Cache extends Facade {}\n')
        (project / 'domain/User.php').write_text('<?php\nnamespace Domain; class User {}\n')
        time.sleep(3)
        laravel_cases = [
            ('54e-native-qualified-class', '<?php\nnew User', 'user'),
            ('54d-cache-dedupe', '<?php\nuse Illuminate\\Support\\Facades\\Cache;\nCache::ge', 'get'),
            ('54-laravel-scope', '<?php\nuse Domain\\Ledger as Account;\nAccount::sett', 'settled'),
            ('54a-laravel-facade', '<?php\nuse Domain\\Relay as Mailer;\nMailer::deli', 'deliver'),
        ]
        for capture, text, expected in laravel_cases:
            fixture = project / (capture + '.php'); fixture.write_text(text)
            run('key', 'Escape', 'ctrl+p'); time.sleep(0.6)
            run('type', '--clearmodifiers', fixture.name); run('key', 'Return'); time.sleep(1.5)
            run('key', 'ctrl+End', 'ctrl+space'); time.sleep(2); shot(capture)
            pixels = Image.open(out / (capture + '.png'))
            ocr_image = out / (capture + '-ocr.png')
            pixels.crop((219, 76, 1100, 412)).resize((1762, 672)).save(ocr_image)
            rendered = subprocess.run(['tesseract', str(ocr_image), 'stdout', '--psm', '11'], env=env, text=True, capture_output=True, check=True, timeout=20).stdout.lower()
            if expected not in rendered or (capture != '54d-cache-dedupe' and 'native php' not in rendered):
                raise RuntimeError('P5 rendered native completion missing: ' + capture)
            if capture == '54d-cache-dedupe':
                # Popup only, source text itself is outside this crop.
                rows = [line.strip() for line in rendered.splitlines() if line.strip() == 'get']
                if len(rows) > 1:
                    raise RuntimeError('Duplicate Cache::get rows rendered')
            run('key', 'Escape')
            if fixture.read_text() != text:
                raise RuntimeError('Laravel completion changed source without acceptance')
        # Exact indexed method/comment byte targets, never a basename search.
        for capture, text, expected_source in [
            ('54b-laravel-definition-target', '<?php\nuse Domain\\Ledger as Account;\nAccount::settled();\n', model_text),
            ('54c-facade-definition-target', '<?php\nuse Domain\\Relay as Mailer;\nMailer::deliver();\n', facade_text),
        ]:
            fixture = project / (capture + '.php'); fixture.write_text(text)
            run('key', 'ctrl+p'); time.sleep(0.6)
            run('type', '--clearmodifiers', fixture.name); run('key', 'Return'); time.sleep(1.5)
            run('key', 'ctrl+End', 'Up', 'End', 'Left', 'Left', 'Left', 'Left', 'F12'); time.sleep(2)
            shot(capture)
            run('key', 'ctrl+a', 'ctrl+c'); time.sleep(0.3)
            copied = subprocess.run(['xclip', '-selection', 'clipboard', '-o'], env=env, text=True, capture_output=True, check=True, timeout=5).stdout
            run('key', 'Escape')
            if copied != expected_source:
                raise RuntimeError('P5 definition did not open exact indexed target: ' + capture)

        # P6 event-path responsiveness during a real background index burst.
        responsive = project / '55-indexing-responsive.php'
        responsive.write_text('<?php\n// ')
        run('key', 'ctrl+p'); time.sleep(0.6)
        run('type', '--clearmodifiers', responsive.name); run('key', 'Return'); time.sleep(1)
        run('key', 'ctrl+End')
        bulk = project / 'domain/Bulk'; bulk.mkdir(exist_ok=True)
        burst_start = time.monotonic()
        for index in range(2000):
            (bulk / ('Item' + str(index) + '.php')).write_text('<?php namespace Domain\\Bulk; class Item' + str(index) + ' {public function save(){}}\n')
        input_start = time.monotonic()
        run('type', '--clearmodifiers', 'responsive-during-index-burst')
        run('key', 'ctrl+a', 'ctrl+c'); time.sleep(0.3)
        copied = subprocess.run(['xclip', '-selection', 'clipboard', '-o'], env=env, text=True, capture_output=True, check=True, timeout=5).stdout
        event_ms = (time.monotonic()-input_start)*1000
        shot('55-indexing-responsive')
        if copied != '<?php\n// responsive-during-index-burst':
            raise RuntimeError('P6 UI input lost during index burst')
        run('key', 'Escape')
        (out / 'php-ui-indexing-benchmark.log').write_text('files=2000 event_type_select_copy_ms=' + str(event_ms) + ' burst_to_capture_ms=' + str((time.monotonic()-burst_start)*1000) + ' includes_xdotool_and_300ms_clipboard_wait=true\n')

        # Disposable Git repository. No remote or destructive UI action.
        def git_fixture(*args):
            return subprocess.run(['git', '-C', str(project), *args], env=env, check=True, capture_output=True, text=True).stdout
        git_fixture('init')
        git_fixture('config', 'user.name', 'Synthetic QA')
        git_fixture('config', 'user.email', 'qa@example.invalid')
        scm_file = project / 'a_scm_keyboard.php'
        scm_file.write_text('<?php // before keyboard diff fixture\n')
        (project / '.gitignore').write_text('*\n!.gitignore\n!a_scm_keyboard.php\n!a_scm_staged.php\n')
        git_fixture('add', 'a_scm_keyboard.php', '.gitignore')
        git_fixture('commit', '-m', 'Synthetic baseline')
        scm_after = '<?php // after keyboard diff fixture\n'
        scm_file.write_text(scm_after)
        staged_file = project / 'a_scm_staged.php'
        staged_after = '<?php // staged keyboard diff fixture\n'
        staged_file.write_text(staged_after)
        git_fixture('add', 'a_scm_staged.php')
        before_status = git_fixture('status', '--porcelain=v1')
        time.sleep(5)
        run('key', 'ctrl+shift+F9'); time.sleep(2)
        shot('56-scm-keyboard-baseline')
        def scm_events():
            log.flush()
            return [line.split('SCM_UI ', 1)[1].strip() for line in (out / 'editor.log').read_text().splitlines() if 'SCM_UI ' in line]
        # Locate the actual named row, never guess a position near Commit.
        import csv, io
        from PIL import Image
        Image.open(out / '56-scm-keyboard-baseline.png').crop((0, 0, 249, 796)).resize((996, 3184)).save(out / '56-scm-ocr.png')
        tsv = subprocess.run(['tesseract', str(out / '56-scm-ocr.png'), 'stdout', '--psm', '6', 'tsv'], check=True, capture_output=True, text=True).stdout
        targets = [r for r in csv.DictReader(io.StringIO(tsv), delimiter='\t')
                   if 'a_scm_keyboard' in r['text'] ]
        if len(targets) != 1:
            raise RuntimeError('Cannot locate unique SCM fixture row in actual baseline pixels')
        target = targets[0]
        row_x = (int(target['left']) + int(target['width']) // 2) // 4
        row_y = (int(target['top']) + int(target['height']) // 2) // 4
        def focus_scm_row():
            # PointerDown seeds focus, releasing outside cancels Click, so
            # it cannot open the diff and transfer focus into its editor.
            seed_start = len(scm_events())
            run('mousemove', str(row_x), str(row_y), 'mousedown', '1')
            run('mousemove', '600', '200', 'mouseup', '1'); time.sleep(1)
            if any('open-diff' in e or 'diff-tab' in e for e in scm_events()[seed_start:]):
                raise RuntimeError('SCM pointer focus seed accidentally activated a diff')
        before_seed = len(scm_events())
        focus_scm_row()
        seeded = scm_events()[before_seed:]
        focused = [e for e in seeded if e.startswith('event=focus:row:')]
        if not focused:
            raise RuntimeError('SCM row focus seed missed; inspect56 baseline before adjusting coordinates')
        row_focus = focused[-1]
        run('key', 'Tab'); shot('56a-scm-next-focus')
        if not scm_events()[-1].startswith('event=focus:checkbox:'):
            raise RuntimeError('SCM Tab did not reach its commit checkbox')
        run('key', 'shift+Tab'); shot('56b-scm-return-focus')
        if scm_events()[-1] != row_focus:
            raise RuntimeError('SCM reverse traversal did not return to same row')
        before_open = len(scm_events())
        run('key', 'Return'); time.sleep(2); shot('56c-scm-open-diff')
        if 'event=diff-tab-opened' not in scm_events()[before_open:]:
            raise RuntimeError('SCM Return did not open an actual diff tab')
        # Return moves editor focus, so reacquire and validate row first.
        focus_scm_row()
        before_space = len(scm_events())
        run('key', 'space'); time.sleep(1); shot('56d-scm-space-open-diff')
        if 'event=diff-tab-opened' not in scm_events()[before_space:]:
            raise RuntimeError('SCM Space did not open an actual diff tab')
        focus_scm_row()
        run('key', 'Tab'); time.sleep(1)
        if not scm_events()[-1].startswith('event=focus:checkbox:'):
            raise RuntimeError('SCM checkbox focus missing before Space')
        checkbox_focus = scm_events()[-1]
        before_check = len(scm_events())
        run('key', 'space'); time.sleep(1); shot('56e-scm-checkbox-toggled')
        check_events = scm_events()[before_check:]
        if not any(e.startswith('event=checkbox ') for e in check_events) or any('diff-tab' in e for e in check_events):
            raise RuntimeError('SCM checkbox Space failed or opened a diff')
        run('key', 'Tab', 'shift+Tab'); time.sleep(1); shot('56f-scm-checkbox-focus-retained')
        if scm_events()[-1] != checkbox_focus:
            raise RuntimeError('SCM checkbox toggle destroyed its focus identity')
        run('key', 'space'); time.sleep(1)
        checks = [e for e in scm_events()[before_check:] if e.startswith('event=checkbox ')]
        if len(checks) != 2 or checks[0] == checks[1]:
            raise RuntimeError('SCM checkbox state did not toggle and restore')
        # Exercise the staged row separately; source reuse is not acceptance.
        staged_targets = [r for r in csv.DictReader(io.StringIO(tsv), delimiter='\t')
                          if 'a_scm_staged' in r['text']]
        if len(staged_targets) != 1:
            raise RuntimeError('Cannot locate unique staged SCM row')
        staged_target = staged_targets[0]
        staged_x = (int(staged_target['left']) + int(staged_target['width']) // 2) // 4
        staged_y = (int(staged_target['top']) + int(staged_target['height']) // 2) // 4
        before_seed = len(scm_events())
        run('mousemove', str(staged_x), str(staged_y), 'mousedown', '1')
        run('mousemove', '600', '200', 'mouseup', '1'); time.sleep(1)
        seeded = scm_events()[before_seed:]
        if any('open-diff' in e or 'diff-tab' in e for e in seeded) or not seeded or not seeded[-1].startswith('event=focus:row:'):
            raise RuntimeError('SCM staged focus seed failed or activated diff')
        staged_focus = seeded[-1]
        run('key', 'Tab', 'shift+Tab'); time.sleep(1); shot('57-scm-staged-return-focus')
        if scm_events()[-1] != staged_focus:
            raise RuntimeError('SCM staged reverse traversal lost row identity')
        before_staged_open = len(scm_events())
        run('key', 'Return'); time.sleep(2); shot('57a-scm-staged-diff')
        if 'event=diff-tab-opened' not in scm_events()[before_staged_open:]:
            raise RuntimeError('SCM staged Enter did not open actual diff')
        # Long nested names are separate from the keyboard target rows.
        long_dir = project / 'z_scm_layout_fixture' / ('nested_directory_' * 5)
        long_dir.mkdir(parents=True, exist_ok=True)
        long_staged = long_dir / ('staged_filename_' * 6 + '.php')
        long_unstaged = long_dir / ('unstaged_filename_' * 6 + '.php')
        long_staged.write_text('<?php // staged layout fixture\n')
        long_unstaged.write_text('<?php // before layout fixture\n')
        git_fixture('add', '-f', str(long_unstaged.relative_to(project)))
        git_fixture('commit', '--only', '-m', 'Synthetic long-name baseline', str(long_unstaged.relative_to(project)))
        long_unstaged.write_text('<?php // after layout fixture\n')
        git_fixture('add', '-f', str(long_staged.relative_to(project)))
        before_status = git_fixture('status', '--porcelain=v1')
        time.sleep(3)
        # Last visual case. Preserve actual geometry and theme settings.
        original_theme_settings = settings_file.read_text()
        geometry = subprocess.run(['xdotool', 'getwindowgeometry', '--shell', window], env=env, check=True, capture_output=True, text=True).stdout
        dimensions = dict(line.split('=', 1) for line in geometry.splitlines() if '=' in line)
        original_width, original_height = dimensions['WIDTH'], dimensions['HEIGHT']
        def staged_visual_focus():
            seed_start = len(scm_events())
            run('mousemove', str(staged_x), str(staged_y), 'mousedown', '1')
            run('mousemove', '600', '200', 'mouseup', '1'); time.sleep(1)
            if any('open-diff' in e or 'diff-tab' in e for e in scm_events()[seed_start:]):
                raise RuntimeError('Theme focus seed activated staged diff')
            run('key', 'Tab', 'shift+Tab'); time.sleep(1)
            if not scm_events()[-1].startswith('event=focus:row:'):
                raise RuntimeError('SCM staged focus lost across resize/theme reload')
        def light_pixels(name):
            # Blank native title strip, away from text and active editor state.
            image = Image.open(out / (name + '.png')).convert('RGB')
            patch = image.crop((300, 2, 350, 14))
            pixels = list(patch.getdata())
            if sum(sum(px) / 3 for px in pixels) / len(pixels) < 170:
                raise RuntimeError('Expected actual light-theme pixels for ' + name)
        staged_visual_focus(); shot('58-scm-focus-outline-dark')
        run('windowsize', window, '850', '640'); time.sleep(2)
        staged_visual_focus(); shot('58a-scm-focus-dark-narrow')
        theme_text = original_theme_settings
        if 'color-theme = "Lapce Dark"' in theme_text:
            theme_text = theme_text.replace('color-theme = "Lapce Dark"', 'color-theme = "Lapce Light"')
        elif 'color-theme = "Lapce Light"' not in theme_text:
            theme_text = theme_text.replace('[core]\n', '[core]\ncolor-theme = "Lapce Light"\n', 1)
        settings_file.write_text(theme_text)
        time.sleep(3); staged_visual_focus(); shot('58b-scm-focus-light-narrow')
        light_pixels('58b-scm-focus-light-narrow')
        run('windowsize', window, original_width, original_height); time.sleep(2)
        staged_visual_focus(); shot('58c-scm-focus-light-wide')
        light_pixels('58c-scm-focus-light-wide')
        # New staged row shifts Changes below its original position.
        Image.open(out / '58c-scm-focus-light-wide.png').crop((0, 0, 249, 796)).resize((996, 3184)).save(out / '58c-scm-ocr.png')
        tsv = subprocess.run(['tesseract', str(out / '58c-scm-ocr.png'), 'stdout', '--psm', '6', 'tsv'], check=True, capture_output=True, text=True).stdout
        targets = [r for r in csv.DictReader(io.StringIO(tsv), delimiter='\t') if 'a_scm_keyboard' in r['text']]
        if len(targets) != 1:
            raise RuntimeError('Cannot locate shifted SCM fixture row in light pixels')
        target = targets[0]
        row_x = (int(target['left']) + int(target['width']) // 2) // 4
        row_y = (int(target['top']) + int(target['height']) // 2) // 4
        focus_scm_row(); run('key', 'Tab'); time.sleep(1)
        if not scm_events()[-1].startswith('event=focus:checkbox:'):
            raise RuntimeError('Light-theme checkbox focus lost')
        shot('58d-scm-checkbox-light-focus')
        run('mousemove', '600', '200', 'click', '1'); time.sleep(1)
        shot('58e-scm-rest-light')
        run('mousemove', str(row_x), str(row_y)); shot('58f-scm-hover-light')
        settings_file.write_text(original_theme_settings)
        time.sleep(3)
        if long_staged.read_text() != '<?php // staged layout fixture\n' or long_unstaged.read_text() != '<?php // after layout fixture\n':
            raise RuntimeError('SCM long-name layout changed source')
        if scm_file.read_text() != scm_after or staged_file.read_text() != staged_after:
            raise RuntimeError('SCM keyboard workflow changed fixture source')
        if git_fixture('status', '--porcelain=v1') != before_status:
            raise RuntimeError('SCM keyboard workflow changed Git index or worktree')
        (out / 'scm-keyboard-assertions.log').write_text('staged_row_keyboard_diff=true\nsame_row_focus=true\ncheckbox_focus_retained=true\ncheckbox_toggled_restored=true\nreturn_and_space_actual_diff=true\nindex_worktree_unchanged=true\n' + '\n'.join(scm_events()))

        # UI-040 deliberate actions are confined to this synthetic local repo.
        def focus_scm_button(token, capture):
            shot(capture + '-locate')
            image = Image.open(out / (capture + '-locate.png'))
            image.crop((0, 0, 249, 320)).resize((996, 1280)).save(out / (capture + '-ocr.png'))
            targets = []
            for psm in ['11', '3']:
                tsv = subprocess.run(['tesseract', str(out / (capture + '-ocr.png')), 'stdout', '--psm', psm, 'tsv'], check=True, capture_output=True, text=True).stdout
                targets = [r for r in csv.DictReader(io.StringIO(tsv), delimiter='\t') if r['text'] == token and int(r['top']) > 800]
                if len(targets) == 1:
                    break
            if len(targets) != 1:
                raise RuntimeError('Cannot locate unique SCM action: ' + token)
            item = targets[0]
            x = (int(item['left']) + int(item['width']) // 2) // 4
            y = (int(item['top']) + int(item['height']) // 2) // 4
            start = len(scm_events())
            run('mousemove', str(x), str(y), 'mousedown', '1')
            run('mousemove', '600', '200', 'mouseup', '1'); time.sleep(0.5)
            run('key', 'Tab', 'shift+Tab'); time.sleep(0.5)
            kind = 'commit' if token == 'Commit' else 'suggest'
            if not scm_events()[-1].startswith('event=focus:' + kind + ':'):
                raise RuntimeError('SCM button reverse traversal missed: ' + token)
            if any(e.startswith(('event=commit ', 'event=suggest ')) for e in scm_events()[start:]):
                raise RuntimeError('SCM focus seed activated button')
            shot(capture)
        before_commit = git_fixture('rev-parse', 'HEAD')
        focus_scm_button('Commit', '59-commit-focus-dark-wide')
        start = len(scm_events())
        run('key', 'ctrl+space'); time.sleep(0.5)
        if any(e.startswith(('event=commit ', 'event=suggest ')) for e in scm_events()[start:]):
            raise RuntimeError('Modified Space activated SCM action')
        run('key', 'Escape')
        focus_scm_button('Commit', '59-commit-focus-dark-wide')
        commit_identity = scm_events()[-1]
        traversal_start = len(scm_events())
        for steps in range(1, 13):
            run('key', 'Tab'); time.sleep(0.2)
            events = scm_events()[traversal_start:]
            if any(e.startswith(('event=focus:suggest:', 'event=suggest ')) for e in events):
                raise RuntimeError('Hidden AI action reached during Tab traversal')
            if any(e.startswith('event=focus:row:') for e in events):
                break
        else:
            raise RuntimeError('Commit traversal did not reach a visible SCM row')
        forward_events = scm_events()[traversal_start:]
        if len(forward_events) != 2 or not forward_events[0].startswith('event=focus:header:') or not forward_events[1].startswith('event=focus:row:'):
            raise RuntimeError('Expected Commit to header to first row sequence')
        shot('59f-header-forward-row')
        run('key', 'shift+Tab'); time.sleep(0.2)
        if scm_events()[-1] != forward_events[0]:
            raise RuntimeError('First row did not reverse to same header')
        shot('59g-header-reverse')
        run('key', 'shift+Tab'); time.sleep(0.2)
        if scm_events()[-1] != commit_identity or any(e.startswith(('event=focus:suggest:', 'event=suggest ')) for e in scm_events()[traversal_start:]):
            raise RuntimeError('Hidden AI reverse traversal failed')
        start = len(scm_events()); run('key', 'Return', 'space'); time.sleep(1)
        if sum(e.startswith('event=commit ') for e in scm_events()[start:]) != 2:
            raise RuntimeError('Empty Commit Enter/Space did not each activate once')
        if git_fixture('rev-parse', 'HEAD') != before_commit or git_fixture('status', '--porcelain=v1') != before_status:
            raise RuntimeError('Empty Commit changed Git state')
        # Empty-message guard is tested before typing the deliberate message.
        run('mousemove', '70', '100', 'click', '1')
        run('type', '--clearmodifiers', 'Synthetic UI-040 keyboard commit')
        focus_scm_button('Commit', '59a-commit-ready')
        run('key', 'Return')
        for _ in range(30):
            time.sleep(0.2)
            if git_fixture('rev-parse', 'HEAD') != before_commit:
                break
        if git_fixture('log', '-1', '--format=%s').strip() != 'Synthetic UI-040 keyboard commit':
            raise RuntimeError('Deliberate keyboard commit did not land')
        if git_fixture('status', '--porcelain=v1'):
            raise RuntimeError('Deliberate commit left unexpected tracked changes')
        for item, expected in [(scm_file, scm_after), (staged_file, staged_after), (long_staged, '<?php // staged layout fixture\n'), (long_unstaged, '<?php // after layout fixture\n')]:
            if git_fixture('show', 'HEAD:' + item.relative_to(project).as_posix()) != expected or item.read_text() != expected:
                raise RuntimeError('Deliberate commit tree/source mismatch')
        shot('59b-commit-complete')
        # With no checked diff, Suggest must return before a backend request.
        clean_head = git_fixture('rev-parse', 'HEAD')
        clean_settings = settings_file.read_text()
        settings_file.write_text(clean_settings.replace('backend = "local"', 'backend = "ollama"'))
        time.sleep(3)
        focus_scm_button('Suggest', '59c-suggest-focus-dark-wide')
        start = len(scm_events()); run('key', 'Return', 'space'); time.sleep(1)
        if sum(e.startswith('event=suggest ') for e in scm_events()[start:]) != 2:
            raise RuntimeError('Suggest Enter/Space did not each activate once')
        light_settings = settings_file.read_text().replace('color-theme = "Lapce Dark"', 'color-theme = "Lapce Light"')
        if 'color-theme = "Lapce Light"' not in light_settings:
            light_settings = light_settings.replace('[core]\n', '[core]\ncolor-theme = "Lapce Light"\n', 1)
        settings_file.write_text(light_settings); time.sleep(3)
        run('windowsize', window, '850', '640'); time.sleep(2)
        focus_scm_button('Suggest', '59d-suggest-focus-light-narrow')
        light_pixels('59d-suggest-focus-light-narrow')
        focus_scm_button('Commit', '59e-commit-focus-light-narrow')
        light_pixels('59e-commit-focus-light-narrow')
        if git_fixture('rev-parse', 'HEAD') != clean_head or git_fixture('status', '--porcelain=v1'):
            raise RuntimeError('Post-commit traversal/suggestion changed Git state')
        settings_file.write_text(clean_settings)
        run('windowsize', window, original_width, original_height); time.sleep(3)
        (out / 'scm-button-assertions.log').write_text('empty_commit_no_mutation=true\ndeliberate_local_commit_exact_tree=true\nbutton_reverse_traversal=true\nsuggest_empty_diff_two_activations=true\npost_commit_git_unchanged=true\nreal_suggestion_provider_unverified=true\n' + '\n'.join(scm_events()))

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
