# Dotted names in quoted helper calls complete as one word (item 343)

Accepting a completion after `route('admin.us` used to replace only the
last segment (`us`), producing `admin.admin.users.index`. The same applied
to `config('app.')`, `view('pages.ho')`, `@include('...')`, `@extends`,
`@section`, `@push`/`@stack`, `__`/`trans` keys and Gate abilities.

`completion_word_start` now extends the word start over `.`- and `-`-joined
segments (`project_completion::dotted_word_len`) exactly when
`project_completion::dotted_name_context` - built on the same `context()`
that decides which items are offered - reports a dotted-name context. The
two can never disagree: no items, no extension. The gate also keeps JS
member access (`document.getEl`) and PHP object access (`$this->foo->b`)
on the plain code boundary. Works in any PHP file, not only Blade.

Tests: `dotted_name_context_and_word_len`.
