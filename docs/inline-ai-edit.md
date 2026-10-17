# Inline AI Edit

Rewrite the current selection with a natural-language instruction, without
leaving the editor. Select some code (or just put the caret on a line),
press `Ctrl+Shift+I`, type what you want ("make this async", "extract this
into a function", "translate the comments to English") and press Enter. The
selection is replaced with the model's rewrite.

No function keys are involved, so the shortcut works on 65% keyboards.

## How it works

- `Ctrl+Shift+I` opens a small instruction input anchored under the
  selection. With no selection, the whole current line is the target.
- Enter sends the selection, the instruction, and a bounded excerpt of the
  surrounding code (for style) to the configured AI Chat backend and waits
  for one reply.
- The reply has any surrounding markdown fence stripped and then replaces
  exactly the captured range.
- Esc closes the overlay and cancels an in-flight request.

The overlay targets the selection's buffer revision: if you edit the buffer
while the model is thinking, the reply is discarded instead of stomping on
your changes. Re-open the overlay to retry on the new text.

## Backend

Inline edit uses the AI Chat configuration (`ai_chat` in settings), the
same key-less Ollama lane as AI commit messages and ghost text. The
default endpoint is https://ai.izdrail.com with
`qwen2.5-coder:1.5b-instruct-q4_K_M`; no API key is needed. With the
backend disabled or unreachable, the overlay simply does nothing on
confirm.

## Prompt budget

Every request is bounded: the selection is clipped at 8 KB, the
surrounding context at 4 KB and the instruction at 1 KB, always on a char
boundary with an explicit truncation marker, so a whole-file selection can
never blow up the request.

## Keybindings

| Key | Action |
| --- | --- |
| `Ctrl+Shift+I` | Open the inline edit overlay |
| `Enter` | Apply the edit |
| `Esc` | Cancel / close |
