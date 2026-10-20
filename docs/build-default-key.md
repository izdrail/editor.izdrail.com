# Optional distributor gateway key (item 285)

Build with LAPCE_BUILD_LLM_API_KEY in the build environment to embed an optional
fallback. Default builds omit it. The actual value must never be committed,
printed, passed in command-line arguments, or included in patch/test fixtures.
The host supplies it through its private build environment, not chat.

Precedence: runtime LAPCE_LLM_API_KEY, endpoint-bound user key, build fallback.
Fallback applies only to HTTPS llm.izdrail.com on port 443 at the canonical
/v1/chat/completions endpoint. Custom hosts, ports, paths and HTTP get no build
key. Local CPU inference never uses a gateway key and remains default. Remote
backend still requires enabling; embedding a key does not enable networking.

Embedding is distribution of the credential: anyone with the binary can recover
and use it. Git cleanliness does not prevent extraction or quota abuse. Use a
revocable, tightly quota-limited distributor credential and rotate when needed.
Build artifacts/caches may also contain it; protect those and never upload trace
or compiler artifacts containing real values. Removing a user key can reactivate
the embedded fallback. Settings names that source without showing the value.

Host gates must use dummy build values first. Verify no-key build, user/env
precedence, canonical gateway binding and wrong-host rejection. Never route a
real embedded key to a fixture. Capture source label with dummy build value;
real-value build/distribution is a separate explicitly approved host action.
