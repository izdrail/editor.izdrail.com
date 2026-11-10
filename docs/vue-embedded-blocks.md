# Vue SFC embedded blocks

Vue files delegate inline script blocks to the existing JS or TypeScript lexical mode and style blocks to CSS or SCSS. `setup`, `scoped` and `module` keep their normal block behavior. Completed opening tags keep their language through EOF during unfinished edits. Recorded regions give JS/TS `//` comments and CSS/SCSS `/* */` comments.

Supported `lang` values: scripts default to JavaScript, with `js`, `javascript`, `ts` and `typescript`; styles default to CSS, with `css` and `scss`. Unknown preprocessors and blocks with `src` stay with the host. Existing template-content guards remain in place. This is lexical composition, not Vue type checking, template-expression parsing, or a preprocessor compiler. HTML files do not interpret `lang` as a Vue preprocessor.

Sources: https://vuejs.org/api/sfc-spec.html and https://vuejs.org/guide/typescript/overview.html
