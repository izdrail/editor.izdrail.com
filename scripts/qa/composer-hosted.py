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
        # Explorer file row: pointer focus, then native Tab to the next file.
        # Activate only the disposable fixture, never create/rename/delete.
        run('mousemove', '95', '375', 'click', '1'); shot('00-explorer-row-focused')
        run('key', 'Tab'); shot('00-explorer-next-row-keyboard')
        run('key', 'Return'); shot('00-explorer-keyboard-open')
        # Open Editors row uses a disposable target already opened above.
        run('mousemove', '105', '145', 'click', '1'); shot('00-open-editors-row-focused')
        run('key', 'Tab'); shot('00-open-editors-next-focus')
        run('key', 'shift+Tab'); shot('00-open-editors-return-focus')
        run('key', 'Return'); shot('00-open-editors-keyboard-activate')
        # Existing terminal tab header, never the PTY content area.
        run('mousemove', '350', '490', 'click', '1'); shot('00-terminal-header-focus')
        run('key', 'Return'); shot('00-terminal-header-keyboard-activate')
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
            'routes/web.php': "<?php\nRoute::get('/users', fn () => null)->name('users.index');\n",
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
