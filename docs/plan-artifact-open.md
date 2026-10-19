# Opening plan/walkthrough artifacts never shows an empty buffer (item 218)

Host QA: clicking a walkthrough row in the chat panel's Plans section
opened the artifact in an editor tab that showed 0 bytes and stayed
empty, while the file on disk had its full content. The normal open
path (jump_to_location -> get_doc -> async proxy buffer load) works
for explorer files but demonstrably left the artifact's doc empty on
host; the exact drop point in the async handoff is not visible from
the app side.

Artifacts are write-once files (epoch-stamped names, never edited
after creation), so the click handler now reads the artifact
synchronously and, when the doc has not loaded content yet, seeds it
directly with init_content before jumping. The regular async proxy
load still runs and delivers the same immutable content, so there is
no clobber risk - unlike create_unsaved_doc (agent proposals), where
the file does not exist on disk and a late response could overwrite
applied text, here both sources agree by construction.

If the file vanished between listing and click, the read fails and
the open falls back to the plain async path.
