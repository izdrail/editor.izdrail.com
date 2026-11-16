# Editable diagnostics excerpts (preview)

Open the command palette and run **Diagnostics: Open Editable Excerpts (Preview)**. The existing **Diagnostics: Open Project Diagnostics Document** remains the read-only summary.

The preview gathers two lines of source context around each diagnostic and merges overlapping windows. File headers separate sources in one editor view. Typing, deletion, selection edits and undo/redo update the same source buffers used by ordinary editor tabs and language servers. Clearing a diagnostic does not remove the active excerpt while you type. Headers and edits crossing excerpt boundaries are rejected.

**Save** saves the underlying source buffers, not the projection text. Source tabs stay confirmed so normal workspace recovery and source close warnings retain them. The preview's dirty state follows its source buffers. An unsaved preview offers **Save Source Buffers** or **Keep Editing** when closed. Save As cannot write the aggregate text as a source file.

If a source changes independently, the stale preview refuses further edits and saves. Reopen the preview to refresh it. Closing or reopening a source under the same path does not let the old preview target a different buffer.

## Preview limits

- Excerpts reuse their source file's current lexer colors. Headers remain plain text. The gutter shows absolute source line numbers, with blank headers and separators. Go to Definition maps the caret to the canonical source before using the existing framework/LSP resolvers. Headers and stale sources do not issue definition requests; late responses are ignored after the source or caret changes. Find References, Go to Implementation and Call Hierarchy also use canonical source coordinates and reject stale asynchronous responses. Source diagnostic messages appear through the existing error lens settings and update with source diagnostics. They are display-only, not saved text. Dedicated diagnostic message rows and other source-aware LSP actions are not implemented in this preview. Stale or reopened source buffers are not painted with old source colors.
- Native compound operations compose sequential deltas while checking each intermediate step against protected headers. Disjoint edits retain their source boundaries instead of flattening across headers. Automatic pair insertion, automatic indentation, IME direct edits and modal motion edits do not yet have full source-editor behavior.
- The projection can include at most 128 files, with a 4 MiB per-file snapshot limit, a 16 MiB combined source budget and a 4 MiB projection budget. Each file can supply at most 1,024 diagnostic ranges. Oversized inputs stop with a message instead of opening a partial editable view.
- Source loading times out after 15 seconds. A save that is not confirmed within 30 seconds reports that fact and leaves the buffers open. Do not treat the timeout as a confirmed save or a confirmed failure; check the source buffers and retry when ready.

This is an early slice of the editable diagnostics multibuffer, not full parity with Zed's multibuffer.
