#!/usr/bin/env python3
"""Local-only regression checks for Linux packaging and build caches."""
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]

class LinuxBuildTests(unittest.TestCase):
    def test_dark_theme_default_and_compact_composer_footer(self):
        self.assertIn('color-theme = "Lapce Dark"', (ROOT / 'defaults/settings.toml').read_text())
        script = (ROOT / 'scripts/qa/composer-hosted.py').read_text()
        self.assertIn("shot('18-composer-footer-dark-wide')", script)
        self.assertIn("shot('18a-composer-footer-dark-narrow')", script)
        source = (ROOT / 'lapce-app/src/panel/chat_view.rs').read_text()
        footer = source[source.index('// Reuse the footer for temporary completion feedback.'):source.index('// Absolute and sized to the panel')]
        self.assertNotIn('min_height(48.0)', footer)

    def test_gravity_is_default_and_only_main_linux_bundle_binary(self):
        import re
        manifest = (ROOT / 'Cargo.toml').read_text()
        self.assertRegex(manifest, r'default-run\s*=\s*"gravity"')
        self.assertRegex(manifest, r'\[\[bin\]\]\s*name\s*=\s*"gravity"')
        for name in ['linux-build.yml']:
            workflow = (ROOT / '.github/workflows' / name).read_text()
            self.assertIn('release/gravity ', workflow)
            self.assertIn('Run ./gravity.', workflow)
            self.assertNotIn('release/lapce ', workflow)
            self.assertNotIn('Run ./lapce.', workflow)
        for name in ['build-linux-hosted.sh', 'build-linux-portable.sh']:
            script = (ROOT / 'scripts' / name).read_text()
            self.assertIn('for bin in gravity lapce-proxy', script)
        desktop = (ROOT / 'extra/linux/dev.lapce.lapce.desktop').read_text()
        self.assertIn('Exec=gravity %F', desktop)
        self.assertNotIn('Exec=lapce ', desktop)

    def test_hosted_acceptance_is_main_only_and_precedes_release(self):
        workflow = (ROOT / '.github/workflows/linux-build.yml').read_text()
        self.assertIn('branches: ["main"]', workflow)
        self.assertNotIn('tags:', workflow)
        self.assertNotIn('paths-ignore:', workflow)
        self.assertIn('needs: tests', workflow)
        self.assertIn("if: github.ref == 'refs/heads/main'", workflow)
        self.assertIn('bash scripts/test-linux-hosted.sh', workflow)
        self.assertNotIn('gh release', workflow)
        self.assertFalse((ROOT / '.github/workflows/linux-release.yml').exists())
        script = (ROOT / 'scripts/test-linux-hosted.sh').read_text()
        for gate in ['editor_review_binding', 'quiet_conversation_tests', 'cargo test --locked --workspace -- --test-threads=2']:
            self.assertIn(gate, script)
        self.assertIn('set -euo pipefail', script)
        self.assertIn('ulimit -n 65536', script)

    def test_hosted_gui_captures_are_synthetic_and_not_auto_acceptance(self):
        workflow = (ROOT / '.github/workflows/linux-build.yml').read_text()
        self.assertIn('python3 scripts/qa/composer-hosted.py', workflow)
        self.assertIn('name: composer-gui-acceptance', workflow)
        script = (ROOT / 'scripts/qa/composer-hosted.py').read_text()
        compile(script, 'composer-hosted.py', 'exec')
        self.assertIn('Inspect PNGs before visual acceptance', script)
        self.assertIn('source unchanged', script)
        self.assertNotIn("run('key', 'Return')", script[script.index("shot('00-initial')"):script.index("# Core editing acceptance")])
        self.assertLess(script.index("shot('07i-before-new-file')"), script.index('added.write_text(added_source)'))
        self.assertLess(script.index('added.write_text(added_source)'), script.index("shot('07j-new-file-refreshed')"))
        self.assertIn("shot('07k-new-file-inserted')", script)
        self.assertIn("if added.read_text() != added_source:", script)
        self.assertLess(script.index('added.unlink()'), script.index("shot('07l-deleted-file-refreshed')"))
        self.assertIn("shot('07m-deleted-file-tab')", script)
        self.assertIn('for index in range(10):', script)
        for state in ['07n-mention-multi', '07o-mention-page', '07p-mention-end', '07q-mention-home', '07r-mention-picked']:
            self.assertIn(f"shot('{state}')", script)
        for state in ['07q1-mention-filtered', '07q2-mention-expanded', '07s-mention-page-up', '07t-mention-dismissed', '07u-mention-reopened', '07v-mention-reverse-focus', '07w-mention-focus-restored']:
            self.assertIn(f"shot('{state}')", script)
        for state in ['07x-mention-caret-away', '07y-mention-caret-return', '07z-mention-selection', '07za-mention-replaced', '07zb-mention-caret-after-insert']:
            self.assertIn(f"shot('{state}')", script)
        self.assertIn("run('key', 'End', 'Page_Up')", script)
        self.assertIn("'--wait', '--new'", script)
        for state in ['09-core-pair-carets', '09a-core-pairs-deleted', '09b-core-pairs-undo', '09c-core-pairs-redo']:
            self.assertIn(f"shot('{state}')", script)
        self.assertIn("'ctrl+Home', 'Down', 'Right', 'ctrl+alt+Down'", script)
        for state in ['10b-file-reference-rapid-click', '10c-file-reference-after-edit-source', '10d-file-reference-after-edit-target', '10e-file-reference-switched-target']:
            self.assertIn(f"shot('{state}')", script)
        self.assertIn("if core_file.read_text() != core_original:", script)
        for state in ['09d-core-enter-before', '09e-core-enter-expanded', '09f-core-enter-undo', '09g-core-enter-redo', '10-file-reference-source', '10a-file-reference-target']:
            self.assertIn(f"shot('{state}')", script)
        self.assertIn("home.rglob('*.log')", script)
        self.assertIn('app.terminate()', script)
        self.assertIn('xvfb.terminate()', script)

    def test_source_mtimes_preserve_only_identical_bytes(self):
        script = (ROOT / 'scripts/restore-source-mtimes.py').read_text()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'same.rs').write_text('same')
            (root / 'changed.rs').write_text('before')
            code = script.replace("root = Path('/src')", f'root = Path({tmp!r})')
            exec(compile(code, 'cache-test', 'exec'), {})
            original = (root / 'same.rs').stat().st_mtime_ns
            time.sleep(.02)
            (root / 'same.rs').touch()
            (root / 'changed.rs').write_text('after')
            changed = (root / 'changed.rs').stat().st_mtime_ns
            exec(compile(code, 'cache-test', 'exec'), {})
            self.assertEqual((root / 'same.rs').stat().st_mtime_ns, original)
            self.assertEqual((root / 'changed.rs').stat().st_mtime_ns, changed)

    def test_portable_build_cold_warm_failure_and_low_disk(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'scripts').mkdir()
            (root / 'scripts/linux-builder.Dockerfile').write_text('FROM test\n')
            (root / 'bin').mkdir()
            commands = {
                'df': '#!/bin/sh\nprintf "Filesystem Blocks Used Available Capacity Mount\\nmock 100000000 1 %s 1%% /\\n" "${FREE_KB:-90000000}"\n',
                'git': '#!/bin/sh\nprintf archive\n',
                'docker': '''#!/bin/bash
printf '%s\\n' "$*" >> "$MOCK_LOG"
case "$1" in
 image) test "${CACHE_HIT:-0}" = 1 ;;
 build) cat > "$MOCK_DIR/recipe" ;;
 create) echo mock-container ;;
 cp) if [ "$2" = - ]; then cat > "$MOCK_DIR/archive"; else printf binary > "$3"; fi ;;
 exec) test "${FAIL_EXEC:-0}" != 1 ;;
 *) echo mock-ok ;;
esac
''',
            }
            for name, body in commands.items():
                path = root / 'bin' / name
                path.write_text(body)
                path.chmod(0o755)
            script = ROOT / 'scripts/build-linux-portable.sh'
            env = os.environ | {'PATH': str(root / 'bin') + ':' + os.environ['PATH'], 'MOCK_LOG': str(root / 'calls'), 'MOCK_DIR': tmp}
            for hit in ['0', '1']:
                env['CACHE_HIT'] = hit
                subprocess.run(['bash', str(script)], cwd=root, env=env, check=True)
                self.assertEqual((root / 'target/ubuntu-22.04/release/gravity').read_text(), 'binary')
            calls = (root / 'calls').read_text()
            self.assertEqual(calls.count('build --tag'), 1)
            self.assertIn('gravity-editor-target-1980', calls)
            self.assertEqual(calls.count('rm -f mock-container'), 2)
            env['FAIL_EXEC'] = '1'
            self.assertNotEqual(subprocess.run(['bash', str(script)], cwd=root, env=env).returncode, 0)
            calls = (root / 'calls').read_text()
            self.assertEqual(calls.count('rm -f mock-container'), 3)
            env['FREE_KB'] = '1'
            self.assertNotEqual(subprocess.run(['bash', str(script)], cwd=root, env=env).returncode, 0)
            self.assertEqual((root / 'calls').read_text(), calls)

    def test_installer_with_spaces_and_invalid_prefix(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bundle = root / 'bundle with spaces'
            bundle.mkdir()
            prefix = root / 'install'
            (bundle / 'gravity').write_text('#!/bin/sh\n')
            (bundle / 'gravity').chmod(0o755)
            for name in ['logo.svg', 'window-icon.png']:
                (bundle / name).write_bytes(b'test')
            script = bundle / 'install-desktop.sh'
            script.write_bytes((ROOT / 'extra/linux/install-desktop.sh').read_bytes())
            subprocess.run(['sh', str(script), str(prefix)], check=True)
            desktop = (prefix / 'share/applications/dev.lapce.lapce.desktop').read_text()
            self.assertIn(f'Exec="{bundle}/gravity" %F', desktop)
            self.assertIn('Icon=dev.lapce.lapce', desktop)
            self.assertEqual((prefix / 'share/icons/hicolor/128x128/apps/dev.lapce.lapce.png').read_bytes(), b'test')
            self.assertNotEqual(subprocess.run(['sh', str(script), 'relative-prefix']).returncode, 0)

if __name__ == '__main__':
    unittest.main()
