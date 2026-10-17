# Walkthroughs

After a chat run, click **Walkthrough** in the AI Chat panel header to
export the session as a markdown document. It is written to
`.lapce/walkthroughs/walkthrough-<yyyymmdd>-<hhmmss>.md` inside the open
folder and opened in the editor, so you can review, trim, and share what
the run covered.

The document starts with the goal (the first line of your first message)
and then lists every turn in order, with code fences preserved.

Honest limits: this is a transcript-style walkthrough of the chat. It does
not include screenshots, recordings, or a diff summary yet - those arrive
with the agent changes UI. Exporting is disabled while a reply is
streaming; the file is capped at 512 KB with an explicit truncation
marker, and a same-second export never overwrites the previous one.

Requires an open folder; scratch windows have nowhere to write.
