# Completion popup placement (item 312)

- Hidden when there are no filtered items. After item 311 gave the popup a border, the empty shell showed as a 1px line under the cursor.
- Flips above the cursor when it would pass the bottom of the editor viewport (previously the bottom of the whole tab, so it opened over the terminal panel). `should_flip_above` is pure and unit-tested. The existing clamp still keeps it inside the tab.

Capture needed: cursor on the last visible editor line with the terminal open; type a prefix with no matches.
