# Focused input placeholders (item 282)

App single-line settings/search inputs and native Floem text inputs hide their
placeholder while focused, and draw actual content whenever nonempty. Content/
focus changes explicitly request paint in addition to layout. Empty blur shows
the placeholder again. This does not change selection or password handling.
The custom masked-key field retains its explicit focused-entry hint by design.

Host: click empty Search Settings/native endpoint-like input, confirm placeholder
hides before typing, type dummy text and verify no overlap, clear, blur and verify
placeholder returns. Repeat narrow/normal. Existing host capture pipeline showed
one-event presentation lag; compare actual buffer with frames before attributing
remaining stale pixels to placeholder logic. Multiline editor phantom-text
placeholders (chat composer) are a separate path, not changed by this patch.
