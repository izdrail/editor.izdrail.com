#!/usr/bin/env python3
"""Syntax-check real editor expansions without executing application code.

Each fragment must parse unchanged in a legal insertion context. This does not
resolve framework symbols, run migrations or establish application correctness.
Blade is deliberately excluded because it needs compilation, not raw PHP lint.
"""
import json
import subprocess
import sys


def contexts(text):
    if text.startswith("<?php"):
        return {"file": text}
    return {
        "statements": "<?php\n" + text,
        "class-members": "<?php\nclass SnippetParent { protected static function boot(): void {} }\nclass SnippetFixture extends SnippetParent {\n" + text + "\n}",
        "function-body": "<?php\nfunction snippetFixture() {\n" + text + "\n}",
        "array-entry": "<?php\n$value = [\n" + text + "\n];",
        "attribute": "<?php\n" + text + "\nclass SnippetFixture {}",
        "builder-chain": "<?php\n$value = app()" + text + ";",
    }


def main():
    entries = json.load(open(sys.argv[1], encoding="utf-8"))
    if not entries or any(not entry.get("prefix") or not isinstance(entry.get("text"), str)
                          for entry in entries):
        raise ValueError("Expected a non-empty list of named snippet expansions")
    failed = []
    for entry in entries:
        diagnostics = []
        for context, source in contexts(entry["text"]).items():
            result = subprocess.run(["php", "-l"], input=source, text=True,
                                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
            if result.returncode == 0:
                print(",".join(entry["prefix"]), context, "PASS")
                break
            diagnostics.append(context + ": " + result.stdout.strip())
        else:
            failed.append(entry["prefix"])
            print("FAIL", entry["prefix"], "\n".join(diagnostics))
    print(f"{len(entries) - len(failed)}/{len(entries)} Laravel expansions passed PHP syntax")
    return bool(failed)


if __name__ == "__main__":
    sys.exit(main())
