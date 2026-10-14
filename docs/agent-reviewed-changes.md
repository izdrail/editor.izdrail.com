# Reviewed changes and checkpoint contract (milestone 4a)

agent_changes is a pure proposal/staging module. It does not edit buffers, save
files, create Git commits or execute tools. The editor review UI and transactional
application are still required before this becomes a usable editing feature.

A proposal contains an exact run ID, path, original buffer revision/content and
proposed replacement. Its digest covers all fields. Only an owner-review UI event
may issue a Reviewed token for the displayed digest. Model approval text is data,
not an owner event. Any later proposal mutation invalidates that review.

Staging checks every current buffer against both original revision and contents,
then returns an owned-buffer snapshot and proposed buffer states. Missing documents,
new files, duplicates, unsafe paths, binary/NUL text, oversized content and revision
overflow are rejected. No earlier file is mutated if a later file conflicts.

Snapshots retain original and actually-applied text/revision. Restore stages only
the owned files if ALL current states still equal the applied states. Later user
edits cause a conflict, not silent overwrite. Restoration increments revisions
instead of rewinding them, and leaves unrelated files, Git staging, databases and
external services untouched.

Limits: 32 existing documents, 256 KiB per original/replacement and 2 MiB total
proposal. Paths reject dot/vendor/node_modules/secrets/credentials and key/certificate
patterns. Scope validation and document ownership remain caller obligations; a
safe-looking path is not access authority.

Next milestone 4b: actual diff review UI, owner-event binding, document ownership,
one undo transaction, checkpoint private persistence, recovery and restore review.
No UI, disk write or end-to-end editing claim is made for this pure contract.

Verification: six tests covering reviewed staging/restore, unrelated-file preservation,
stale base/proposal rejection, later user conflicts, multi-file all-or-none staging,
invalid/duplicate/missing/oversized content and snapshot serialization. Local app
check passes. Host full tests/build gates and pixel verification remain required.
