# Go to Vue / Svelte / JSX component

In a `.vue`, `.svelte`, `.tsx` or `.jsx` file, put the cursor on a tag name and run Go to Definition
(`<UserCard`, `<user-card`). The component file opens, found under `resources/js` (also
`resources/ts`, `resources/vue`). Kebab-case tags map to PascalCase file names.

If several files share the name, one inside a `Components` folder wins, then the shortest path.
Built-in tags (`<div>`, `<router-link>`) do nothing unless a file with that name exists. Not read:
import aliases, `components.d.ts`, auto-import config, and names registered under a different name.
Limits: 5000 directory entries, depth 8, `node_modules` and symlinks skipped. Nothing is executed.
If nothing is found, normal Go to Definition runs.

For Inertia page names in PHP, see [go-to-inertia.md](go-to-inertia.md).
