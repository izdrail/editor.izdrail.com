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

The selected code must fit in 8,000 UTF-8 bytes and the instruction in 1,000 bytes. Larger selections show a notice without contacting the model or changing source; select less code and reopen. Longer instructions stay in the input so you can shorten them and press Enter again. Neither selected code nor instructions are silently truncated during an edit. Surrounding context alone may be clipped at 4,000 bytes on a character boundary with an explicit marker.

## Keybindings

| Key | Action |
| --- | --- |
| `Ctrl+Shift+I` | Open the inline edit overlay |
| `Enter` | Apply the edit |
| `Esc` | Cancel / close |
