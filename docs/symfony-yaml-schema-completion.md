# Symfony YAML config schema completion (item 445)

Key and value completion inside Symfony YAML config files, driven by the
JSON schemas Symfony merged in symfony/symfony#61282 (7.4, MIT).

## What completes

| File | Pack | Completes |
| --- | --- | --- |
| `config/services.yaml`, `config/services_*.yaml` | services | `imports` / `parameters` / `services` at root; service definition keys (`arguments`, `autowire`, `tags`, `calls`, `factory`, ...) under a service id; `_defaults` / `_instanceof` shapes |
| `config/packages/**.yaml` | services | the same root keys (`when@<env>` blocks included) |
| `config/routes.yaml`, `config/routes/**.yaml` | routing | route definition keys (`path`, `controller`, `methods`, `requirements`, ...), import and alias shapes |

Values complete from schema enums (`decoration_on_invalid: exception |
ignore`) and from boolean types (`autowire: true | false`).

## How it works

- `scripts/generate-symfony-yaml-schemas.py` flattens the two pinned
  schema files into `metadata/web/symfony-yaml-schemas.json`: local
  `$ref`s (including the `when@` self-reference) and `oneOf`/`anyOf`
  unions are resolved, `patternProperties` become path wildcards
  (`when@<env>` keeps its prefix as `when@*`), and booleans gain
  `true`/`false` values. Facts only; provenance is recorded in
  `metadata/web/manifest.json` (pinned commit
  `039017e8635dbc56d1f4bc930c60e5aa45d218e6`).
- `web_metadata::yaml_cursor_context` scans the document before the
  cursor by indentation: mapping keys stack into the completion path,
  `- ` sequence items contribute a `*` segment, and the position is
  classified as key or value. Flow collections and block scalars are
  out of scope.
- `project_completion::symfony_yaml_schema_items` maps the workspace
  -relative path to a pack and offers schema children (wildcard keys
  are never offered literally) or enum/boolean values.
- The editor chains these items with `show_on_empty` and keeps
  completion alive in these files, same as item 444.

## Limits

- Bundle extension trees (`framework:`, `twig:`, `doctrine:`, ...) are
  not covered: Symfony ships no static schema for them (runtime
  generation, symfony/symfony#59620, was closed unmerged). The plan
  keeps runtime generation opt-in.
- YAML tags (`!service`, `!php/enum`) get no completion - upstream
  limitation noted in the merged PR.
