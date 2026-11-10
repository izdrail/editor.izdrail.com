# Embedded web type routing

Inline script/style delegation reads exact HTML attributes, with spaces around `=`, single/double/unquoted values and quoted `>` characters. `data-type` and `data-src` do not stand in for `type` and `src`. The first duplicate attribute wins. JavaScript classic/module types and CSS remain supported; JSON import maps, speculation rules and unknown data types keep the host lexer. External scripts are not delegated. Template-body guards are unchanged.

This is a bounded highlighting helper, not a DOM parser. HTML character references inside attribute values are not decoded.

Sources checked: https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/script/type and https://html.spec.whatwg.org/multipage/scripting.html

Embedded bodies end only at an exact HTML `script`/`style` closing-tag name. Prefix look-alikes such as `</scripture>` and `</styles>` no longer cut off delegation. A genuine `</script>` still closes the HTML element even inside a JavaScript string, as HTML raw-text parsing requires.

A completed inline script/style opening tag also delegates through EOF while its closing tag is unfinished or missing. Adding the closing tag returns following markup to the host; removing it restores the same embedded body. Unsupported types, external scripts and template-containing bodies retain their existing guards. See https://html.spec.whatwg.org/multipage/parsing.html#rawtext-state .
