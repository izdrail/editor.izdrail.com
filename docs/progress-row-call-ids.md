# Progress rows no longer show tool-call ids (item 210, host QA follow-up)

Each tool result row in the progress list showed the protocol's
tool-call id (a long token like "0bGMKEldv5i8KTCqmGPppK9lsWu5hA08")
as its title. The id is wire plumbing for matching calls to results,
not information for the owner. The row now titles itself "Result" and
keeps the outcome detail ("read completed" / "denied or failed"); the
id is still used internally for the approval card's pending-match.
