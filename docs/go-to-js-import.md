# Go to JS / TS / Vue import

In a `.js`, `.ts`, `.tsx`, `.jsx`, `.mjs`, `.vue` or `.svelte` file, put the cursor on the path in an
import and run Go to Definition:

- `import Card from '@/Components/Card.vue'` - `@/` and `~/` map to `resources/js` (then
  `resources/ts`, `resources/vue`), the Laravel Vite / Inertia starter-kit alias.
- `import('./Chart')`, `require('../util')` - relative to the current file.

A missing extension is tried as `.vue .ts .tsx .js .jsx .svelte .mjs`, then `index.*` in a folder.
Not resolved: package names, URLs, other aliases, `vite.config.js` / `tsconfig.json` paths. The file
must be inside the workspace; symlinked files are skipped. Nothing is executed. If nothing is found,
normal Go to Definition runs.

See also [go-to-vue-component.md](go-to-vue-component.md) if present.
