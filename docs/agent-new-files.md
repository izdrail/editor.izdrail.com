# Act proposals can create new files (item 189)

An Act proposal may now name a path that does not exist yet: the
review lane validates it as a creation target (inside the project,
under an existing directory, not already on disk, never through a
symlink pointing outside the root, never inside .git) and opens it as
an in-memory unsaved document via MainSplitData::create_unsaved_doc.
Unlike get_doc this makes no proxy new_buffer request, so no async
NewBufferResponse can land after the apply commit and clobber the
applied text; the file reaches disk only if the owner saves. The
proposal's base state is simply the empty buffer, so staging, the
before/after diff (one all-added hunk), the checkpoint journal and
per-hunk accept all work unchanged. Restoring a checkpoint returns the
buffer to empty-unsaved; close the tab to discard the created file
entirely. The Act system contract tells the model it may propose
not-yet-existing paths.

Coverage: creation_target rules (missing-under-existing-dir, existing
file, escapes, absolute, missing directory, .git, symlinked parent)
and empty-base hunk splitting are behavior-tested.
