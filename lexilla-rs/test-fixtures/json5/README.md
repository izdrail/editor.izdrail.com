# JSON5 lexer fixtures

- `package.json5`: real json5/json5 package manifest, retrieved from
  https://raw.githubusercontent.com/json5/json5/1ecc0125a053862df78ade515f72488d770f57ec/package.json5
- `upstream_valid.rs`: distinct successful inputs captured by wrapping the
  `parse` entry point while running json5/json5's `test/parse.test.js` at the
  same commit (69 tests passed). Source:
  https://github.com/json5/json5/blob/1ecc0125a053862df78ade515f72488d770f57ec/test/parse.test.js
- `features.json5`: hand-written representative coverage fixture.
- `UPSTREAM-LICENSE.md`: upstream MIT license for the manifest, parse-test
  inputs, and Unicode identifier data used by the JSON5-only supplement.

The lexer is a syntax highlighter, not a structural JSON5 validator. The
scanner supports JSON5 tokens, including Unicode 18 identifier additions,
without changing strict JSON's upstream state machine or scope tags.
