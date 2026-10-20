# Chat error recovery card (item 264)

Errors now have a titled card, wrapping error body with a 100px vertical scroll
limit, and an Open AI connection settings action. The action opens Settings;
select AI Chat there. It does not test, save, retry, or change provider/backend.
The guidance is conditional (for connection or 401/403 errors), not a claim that
every chat error is an authentication problem. Empty error hides the card.

Host regression matrix, at normal/narrow panel widths:
- Real fixture 401/403: full error is reachable by scrolling, action opens
  Settings, no new network request occurs just from opening it.
- Transport failure and long multiline/non-ASCII error: body wraps, can scroll,
  composer remains reachable. Check a short panel height as well as 1000px.
- Nonconnection error (empty prompt/project/history): retains original message,
  and conditional connection guidance does not replace it.
- Successful send or clear: card hides when error is cleared.
- Keyboard focus and activation of action need runtime checking; declaring
  navigability alone does not prove Enter/Space activation works.

This bounds the error body only. The full chat panel has other fixed sections
and can still require further work at extremely short heights. Trust disclosure
ellipsis and mode-specific empty state remain follow-ups, not solved here.

## Trust disclosure (item 265)

The host 57a0150 screenshot showed the endpoint in the project-read approval
line truncated before the Approve action. The disclosure now wraps at ordinary
foreground contrast, with its action on a separate row below. This changes no
permission condition, endpoint binding, approve/revoke behavior or default.

Host checks: narrow/normal chat panel, long fixture endpoint, unapproved and
approved states. Read the complete endpoint and not-approved phrase without
horizontal scrolling; Approve/Revoke must remain visible and not overlap the
composer. Check short-height layout because the disclosure needs extra height.
Do not approve real project disclosure merely to obtain a screenshot. This
source change does not prove clicks, focus or permission behavior at runtime.
