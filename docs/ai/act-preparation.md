# Act preparation contract (not enabled)

`agent_act_contract` is pure validation only. It is not connected to the agent
loop, model transport, review UI or executor. `Mode::Act` still rejects before
calling the model. Existing Ask/Plan tools remain read/search only.

## Proposed first Act scope

After 4b pixel/click acceptance, a first integration can generate an edit
proposal, not auto-apply it. The owner starts a fresh scoped run. The host
captures secure existing open-file bases and identities BEFORE a model request.
It supplies content only within the existing explicit project/server read grant.
No new trust comes from selecting Act. No terminal, network tool, source save,
new file, delete, rename, paid provider or unattended edit is in this scope.

`Context::capture` accepts an owner run ID, host-created scope digest and owned
base map (buffer identity, revision and text). It rejects unsafe paths, duplicate
buffers and oversized bases. The host must independently verify Linux secure
read policy, loaded writable buffers, editor autosave disabled and mixed-line-
ending restrictions. The pure contract cannot establish these live facts.

The scope digest must bind canonical project identity, server/provider endpoint
and the current permission generation, not just a display name. Run IDs must be
fresh per invocation and must not reuse the chat session ID. Revocation, Stop,
new run, workspace/endpoint change, window close and timeout invalidate context.
No deserialized or model-supplied object can create a Context.

The future model response is strict JSON only:

```json
{
  "context_digest": "host-issued digest",
  "changes": [
    {
      "path": "app/User.php",
      "base_digest": "host-issued digest for this path and owned base",
      "replacement": "full replacement text\n"
    }
  ]
}
```

Unknown fields, model approvals, commands, caller-supplied bases, code fences,
paths outside the capture and duplicate/no-op/oversized changes are rejected.
Digests are content bindings, not secrets or permission tokens. The model is
expected to echo them; echoing one never authorizes an action.

`review_candidate` requires the host's CURRENT scope, run and owned bases. A
revision, text or buffer-identity mismatch blocks the entire candidate. It uses
the original captured base, never a fresh base read after generation. It returns
only an unreviewed `Proposal`. The review UI must normalize replacement line
endings before displaying and computing the final proposal digest, retain the
original base, show every changed file, and obtain a separate owner Apply click.
The existing digest/base/checkpoint safeguards then run again at Apply.

Generation errors, invalid JSON, Stop or lost permissions yield no proposal or
mutation. Edits made while generation runs must not be silently rebased. A retry
starts a fresh owner-visible run and new captured context. A future executor
must not mark Done merely because a model said "applied". State must distinguish
proposal awaiting review, discarded, applied, restored, conflicted and interrupted.
Only editor/journal outcomes can substantiate application.

## Future integration acceptance gates

- 4b review/apply/restore screenshots and actual click assertions passed first.
- Act disabled now: no new mode button, request, tool registration or execution.
- A loopback model can produce a proposal without changing buffers or files.
- User editing, close/reopen, root/endpoint change, permission revoke, timeout,
  Stop and delayed previous-run replies all prevent candidate installation.
- Valid output preserves original base and owner review boundaries; no model
  wording, tool result, file instruction or echoed digest can authorize Apply.
- Mixed endings and Unicode are displayed accurately; changing normalization
  invalidates any old displayed digest.
- Apply/checkpoint/restore retains 4b conflict and autosave safeguards; source
  files, Git state and unrelated buffers remain unchanged.
- Real-model quality, latency and cancellation acceptance is separate from
  transport mocks and pure tests. Paid models remain unavailable until their
  credential, pricing and spend controls have separate verified integration.

True cross-document Ctrl+Z, terminal execution, browser automation and manager
views are independent later features. This module does not claim any of them.
