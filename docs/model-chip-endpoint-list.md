# Model chip: on-demand endpoint model list (item 206, AI parity)

The composer's model chip was display-only for the ollama backend:
clicking it returned early because the dropdown only knew the local
model catalog. The chip now opens an overlay that lists the models the
configured endpoint actually offers - fetched on demand when the chip
opens (GET /v1/models, OpenAI wire shape; the parser also accepts
Ollama's native {"models":[...]} shape), never at startup, with
explicit loading, error and empty rows that are not clickable.

Picking a row writes the model to settings.toml's ai-chat model key
(the config file is watched, so the live config and the chip label
follow), matching how the local lane writes local-model. The URL
builder mirrors completion_url's path rules through a shared
checked_base validation helper, so the list request always targets the
same endpoint shape the chat lane posts to.

Coverage: models_url is unit-tested against the same path cases as
completion_url (bare host, /v1 suffix, trailing slash, rejected
non-http and credentialed URLs); parse_model_names is unit-tested for
both wire shapes, trimming, dedup, empty entries, empty lists, wrong
shapes and invalid JSON. The fetch itself (timeouts, 1 MB cap,
non-success HTTP mapping) mirrors the chat lane's client settings and
is exercised by the host gate's build, not locally.
