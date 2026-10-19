# Act model capability probing (spec, item 223)

Problem: the composer model chip lists every model the endpoint
reports (item 206), but Act needs tool support. Models without it
(gemma3:4b) only fail at send time with a provider 400 (softened by
item 222). This spec dims non-tool models in the dropdown before the
user picks them, without a per-open round-trip storm.

## Data source

Ollama native API: POST {base}/api/show with {"model": name}. Newer
Ollama answers a "capabilities" array that includes "tools" when the
model supports tool calling. OpenAI-compatible endpoints have no such
resource - /api/show returns 404/405 there.

Tri-state per (endpoint, model):
- Some(true)  - capabilities present and contain "tools"
- Some(false) - capabilities present, no "tools"
- None        - unknown (not Ollama, field absent, probe failed).
  UNKNOWN NEVER HIDES OR BLOCKS - fail open, the item 222 send-time
  error stays as the backstop.

## chat.rs additions

- show_url(endpoint): checked_base + "/api/show".
- parse_show_capabilities(bytes) -> Option<bool> as above; missing
  "capabilities" field -> None (older Ollama), invalid JSON -> None.
- model_capabilities(endpoint, model) -> Result<Option<bool>, String>:
  POST with 15s connect / 30s total timeout, 256 KiB body cap. A
  non-success status returns Ok(None), not Err - an endpoint without
  /api/show is a capability-unknown endpoint, not a broken one. Err is
  reserved for transport failures, which the caller also folds to
  None.
- Session cache: static OnceLock<Mutex<HashMap<(String, String),
  Option<bool>>>> keyed by (endpoint, model). No TTL: capabilities
  change only when a model is re-pulled, and the key changes with the
  endpoint. In-memory only, nothing persisted.

## Lazy probing in the composer chip

Probing hooks into the existing item-206 flow in chat_view.rs: when
the model list arrives (models_result Ok), for each listed model not
yet cached, spawn one background probe; results land in a signal map
the dropdown rows read. One in-flight probe per key, so reopening the
chip mid-probe never doubles requests, and cached entries make later
opens free. Nothing is probed at startup, nothing is probed for the
"local" backend (same guard as the list fetch).

## UI behaviour

- Rows with Some(false): dimmed, suffixed " (no tools)".
- Mode is Act: a Some(false) row is not selectable (click shows the
  item-222 style hint in the panel error line).
- Modes Chat/Ask/Plan: Some(false) rows stay selectable - those modes
  do not require tools.
- Some(true) and None: unchanged from today.

## Tests

- parse_show_capabilities: with tools / without tools / missing field /
  garbage -> Some(true) / Some(false) / None / None.
- model_capabilities against a loopback server: 200 with capabilities,
  200 without the field, 404 -> Ok(None); assert the request hits
  /api/show with the model name in the body.
- Cache: two calls for the same key produce one server hit.

## Rollout

Patch A: chat.rs transport + parser + cache + tests, no UI.
DONE (item 232). Deviation from the spec text: show_url rebuilds
the endpoint ORIGIN (scheme + host + port) via url::Url::origin
and appends /api/show, instead of appending to checked_base - the
native API does not live under /v1, and string-splitting on "/v1"
breaks hosts like "v1host".
Patch B: dropdown dimming + Act-mode selection block. DONE (item
233): opening the model chip probes each listed model once
(in-flight set + the patch-A cache); rows known to lack tools are
dimmed, suffixed " (no tools)", and in Act mode reject the click
with an error-line hint. Chat/Ask/Plan stay selectable; unknown is
unchanged. One caveat: the in-flight set keys by model name only,
so a same-named model on a freshly switched endpoint probes on the
next open instead of mid-flight.
