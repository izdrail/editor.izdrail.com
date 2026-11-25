# Experimental Symfony conventions (L6)

Both the `logos-php` build feature and runtime `GRAVITY_LEXER=logos` opt-in are required. See [the current guide](logos-lexer.md).

Existing `ProjectProfile.symfony_app` evidence plus the `logos-php` flag selects a Symfony conventions overlay over PHP/HTML byte tokens. Lexilla stays default. Laravel retains its explicit precedence when both profiles apply; WordPress-only projects are unchanged.

Supported lexical contexts include route/controller/DI attributes, command attributes and console types, event listeners/subscribers, Messenger handlers/bus calls, form types/builders and validation attributes, and Doctrine entity/repository/relationship attributes. Metadata types are drawn from the existing exact-version Symfony symbol selection; a small convention set supports common framework shapes independently of semantic resolution.

Existing Twig composition is reused. In the opt-in Symfony profile it receives Symfony bridge function vocabulary for routing, assets, CSRF/security, forms and rendering. HTML/script/style/Twig handling is not replaced with a new template scanner.

Comments, ordinary strings, variables and HTML preserve base styles. Attributes are recognised from lexical bracket context, with nested bracket tracking. New validated overlay IDs45-47 are distinct from base PHP/HTML and Laravel IDs. PHP comment toggling understands them.

This is not a parser or symbol resolver. Namespace aliases and same-name user methods can only be handled as lexical conventions, not proven Symfony/Doctrine symbols. Custom attributes are not claimed. There is no PHP execution, runtime discovery, dependency addition or inference change.

Validation includes category fixtures, false-positive guards, UTF-8 prefixes/unfinished input, import checks, feature-gated PHP/Twig profile integration, and the full hosted acceptance workflow.
