"""Check smoke output structure and invariants, never machine timing limits."""
import json
import sys

with open(sys.argv[1], encoding="utf-8") as stream:
    report = json.load(stream)
assert report["schema"] == 2
assert report["default_decision"] == "KEEP_LEXILLA"
assert {case["case"] for case in report["results"]} == {"php", "mixed", "blade", "laravel", "symfony"}
for case in report["results"]:
    assert case["bytes"] >= 64 * 1024
    assert case["worker_yields"] > 0
    assert case["worker_cancel_observed_ms"] >= 0
    assert 0 <= case["native_scope_agreement_diagnostic"] <= 1
    assert {edit["position"] for edit in case["edits"]} == {"front", "middle", "tail"}
    for edit in case["edits"]:
        assert 0 < edit["logos_relexed_bytes"] <= case["bytes"] + 1
        assert edit["logos_cache_update_median_ms"] >= 0
print("Logos benchmark schema, coverage and correctness checks passed")
