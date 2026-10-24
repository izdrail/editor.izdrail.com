//! Project-aware completion items: inside `route('...')` offer the app's
//! named routes, inside `config('...')` its config keys, inside `env('...')`
//! the env keys, inside `view('...')` its Blade view names, inside
//! `__('...')` its translation keys, and inside `wire:model="..."` /
//! `wire:click="..."` in a Livewire view the paired component's public
//! properties / methods. In a `.blade.php` view, typing `$` offers the
//! variables the view defines above the cursor, typing `<x-`
//! offers the app's Blade component tags, and `asset('...')` /
//! `secure_asset('...')` / `mix('...')` complete files under
//! `public/`, and `@can` / `->can()` / `Gate::allows('...')`
//! complete Gate abilities from policies and `Gate::define`. Same
//! static
//! scans as the palettes (nothing is executed), cached 30 s per workspace
//! (per component class for Livewire) so completion stays cheap on every
//! keystroke.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use lsp_types::{CompletionItem, CompletionItemKind};

const TTL: Duration = Duration::from_secs(30);
const MAX_ROUTE_FILES: usize = 60;
const MAX_FILE: u64 = 1 << 20;
const MAX_DEPTH: usize = 6;
const MAX_VIEW_FILES: usize = 1000;
const MAX_NAMES: usize = 2000;
const MAX_ITEMS: usize = 500;

/// Which project string is being typed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Route,
    Config,
    Env,
    View,
    /// Blade `@section('...')` / `@yield('...')`.
    Section,
    /// Blade `@push('...')` / `@stack('...')`.
    Stack,
    /// Translation key: `__`, `trans`, `trans_choice`, Blade `@lang` /
    /// `@choice`, `Lang::get` / `Lang::has` / `Lang::choice`.
    Lang,
    /// Livewire `wire:model="..."` (any modifier): a property of the
    /// component class paired with the view.
    LivewireModel,
    /// Livewire `wire:click="..."` and the other event attributes: a
    /// method of the component class paired with the view.
    LivewireAction,
    /// Blade component tag `<x-<partial>` (dots for nested components).
    ComponentTag,
    /// Livewire component tag `<livewire:name` (dots for nesting).
    LivewireTag,
    /// Livewire directive `@livewire('name')`: a component name.
    LivewireName,
    /// Livewire attribute modifier: `wire:model.` / `wire:keydown.en`.
    LivewireModifier,
    /// Alpine event modifier: `x-on:click.` / `@click.` / `@keydown.en`.
    AlpineModifier,
    /// Alpine `$store.<name>`: a store registered with
    /// `Alpine.store('name', ...)` in project JS/TS or inline scripts.
    AlpineStore,
    /// Alpine `$refs.<name>`: an `x-ref="name"` in the current file.
    AlpineRef,
    /// Alpine/Livewire magic property: `$el`, `$store`, `$wire`, ...
    /// typed inside an Alpine expression or `wire:` attribute value.
    AlpineMagic,
    /// Livewire `$wire.<member>`: a property or method of the component
    /// class, inside an Alpine expression or `wire:` attribute value.
    LivewireMember,
    /// Livewire `$wire.$magic`: the `$wire` object's own magics
    /// (`$set`, `$toggle`, `$refresh`, ...).
    LivewireMagic,
    /// Livewire `$wire.$js.<name>`: a function registered with
    /// `$js('name', fn)` in the view's `@script` block.
    LivewireJs,
    /// `$entangle('...')` / `$wire.entangle('...')`: a public property
    /// of the Livewire component paired with the current view.
    LivewireEntangle,
    /// `$wire.$watch('...')`: a public property of the Livewire
    /// component paired with the current view.
    LivewireWatch,
    /// `<x-forms.input ty`: an attribute (prop) name inside a Blade
    /// component tag, after the tag name.
    BladeProp,
    /// `<livewire:user-profile em`: a mount property name inside a
    /// Livewire component tag, after the tag name.
    LivewireProp,
    /// Alpine `x-data="name"`: a component registered with
    /// `Alpine.data('name', ...)` in project JS/TS or inline scripts.
    AlpineData,
    /// Livewire event name: `$dispatch('...')`, `$this->dispatch('...')`,
    /// `Livewire.dispatch('...')`, `->emit('...')` (Livewire 2).
    LivewireEvent,
    /// Alpine event name: `x-on:<partial>` / `@<partial>` inside a tag.
    AlpineEvent,
    /// Livewire attribute name: `wire:<partial>` inside a tag, before any
    /// modifier dot or value.
    LivewireAttribute,
    /// Livewire `wire:target="..."`: an action or property the
    /// loading/dirty state is scoped to.
    LivewireTarget,
    /// `asset('...')` / `secure_asset('...')` / `mix('...')`: a path
    /// under `public/`.
    Asset,
    /// Gate ability: `@can('...')`, `$user->can('...')`,
    /// `Gate::allows('...')` and friends.
    Gate,
    /// Validation rule: the rules string of `->validate([...])`,
    /// `Validator::make(..., [...])` and `#[Validate('...')]`.
    ValidationRule,
    /// Database table: `Schema::table('...')`, `DB::table('...')`,
    /// `->constrained('...')` and friends, and the table parameter of
    /// the `exists:` / `unique:` validation rules (item 388).
    DbTable,
    /// Database column: after the comma in the `exists:` / `unique:`
    /// validation rules (item 390). The table named before the comma
    /// decides which columns are offered.
    DbColumn,
    /// Database column: the first argument of a query-builder call
    /// (`->where('...')`, `->orderBy('...')`, ...) on a line that
    /// names the base table earlier with `DB::table('...')`
    /// (item 392).
    QueryColumn,
    /// Middleware alias: a quoted string in the first argument of
    /// `->middleware('...')` / `Route::middleware('...')` /
    /// `->withoutMiddleware('...')` (item 393).
    MiddlewareAlias,
    /// Filesystem disk: the first argument of `Storage::disk('...')`
    /// / `Storage::drive('...')` (item 397).
    StorageDisk,
    /// Vite entry point: `Vite::asset('...')` / `@vite('...')` /
    /// `@vite(['...'])` (item 398).
    ViteEntry,
    /// Log channel: `Log::channel('...')` / `Log::driver('...')` /
    /// `Log::stack(['...'])` (item 399).
    LogChannel,
    /// Cache store: `Cache::store('...')` / `Cache::driver('...')`
    /// (item 400).
    CacheStore,
    /// Mailer: `Mail::mailer('...')` / `Mail::driver('...')`
    /// (item 401).
    MailMailer,
    /// Auth guard: `Auth::guard('...')` / `Auth::shouldUse('...')`
    /// (item 402).
    AuthGuard,
    /// Database connection: `DB::connection('...')` (item 404).
    DbConnection,
    /// Redis connection: `Redis::connection('...')` (item 405).
    RedisConnection,
    /// Queue connection: `Queue::connection('...')` (item 406).
    QueueConnection,
    /// Broadcast connection: `Broadcast::connection('...')` /
    /// `Broadcast::driver('...')` (item 407).
    BroadcastConnection,
    /// Artisan command: `Artisan::call('...')` /
    /// `Artisan::queue('...')` (item 408).
    ArtisanCommand,
    /// Migration Blueprint method: `$table->str` in a schema closure.
    BlueprintMethod,
}

/// The context at the end of `line` (the text before the cursor): the last
/// `word(` whose argument string is still open. Quote escapes are not read;
/// `route('it\'s')` is rare and simply falls out of context.
pub fn context(line: &str) -> Option<Kind> {
    // Alpine `$store.<name>` in an attribute expression or inline script:
    // the text after the last `$store.` is a partial store name. Checked
    // first so `wire:click="$store.na" is not read as a Livewire method.
    if let Some(i) = line.rfind("$store.") {
        let rest = &line[i + "$store.".len()..];
        if rest
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        {
            return Some(Kind::AlpineStore);
        }
    }
    // Alpine `$refs.<name>`: the text after the last `$refs.` is a
    // partial ref name.
    if let Some(i) = line.rfind("$refs.") {
        let rest = &line[i + "$refs.".len()..];
        if rest
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        {
            return Some(Kind::AlpineRef);
        }
    }
    // Migration Blueprint methods: `$table->` inside a
    // Schema::create / Schema::table closure (item 382). Line-local:
    // the `$table->` prefix is the convention. Arguments
    // (`$table->string('na`) and chains (`$table->string('x')->nul`)
    // are not name positions; `$this->table->` does not contain the
    // `$table->` marker.
    if let Some(i) = line.rfind("$table->") {
        let rest = &line[i + "$table->".len()..];
        if rest
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Some(Kind::BlueprintMethod);
        }
    }
    // Livewire `$wire.$js.<name>`: a function registered with
    // `$js('name', fn)` in the view's `@script` block (item 372). The
    // more specific dotted path runs before the `$wire.$` magic scan.
    if let Some(i) = line.rfind("$wire.$js.") {
        let rest = &line[i + "$wire.$js.".len()..];
        if rest
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        {
            if let Some(attr) = quoted_value_attr(line) {
                if alpine_expression_attr(&attr) || attr.starts_with("wire:") {
                    return Some(Kind::LivewireJs);
                }
            }
        }
    }
    // Livewire `$wire.$magic` (`$set`, `$toggle`, ...): the partial
    // after the last `$wire.$` must start a known magic name, so an
    // unrelated `$wire.$foo` does not raise an irrelevant list. Runs
    // before the `$wire.` member scan, whose partial never starts with
    // `$`.
    if let Some(i) = line.rfind("$wire.$") {
        let rest = &line[i + "$wire.$".len()..];
        if rest
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            && LIVEWIRE_WIRE_MAGICS
                .iter()
                .any(|(name, _, _)| name[1..].starts_with(rest))
        {
            if let Some(attr) = quoted_value_attr(line) {
                if alpine_expression_attr(&attr) || attr.starts_with("wire:") {
                    return Some(Kind::LivewireMagic);
                }
            }
        }
    }
    // Livewire `$wire.<member>`: the text after the last `$wire.` is a
    // partial member name inside an Alpine expression or `wire:`
    // attribute value. Runs before the magic scan and the `wire:` scan
    // so `wire:click="$wire.sa` offers members, not bare method names.
    if let Some(i) = line.rfind("$wire.") {
        let rest = &line[i + "$wire.".len()..];
        if rest
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            if let Some(attr) = quoted_value_attr(line) {
                if alpine_expression_attr(&attr) || attr.starts_with("wire:") {
                    return Some(Kind::LivewireMember);
                }
            }
        }
    }
    // Alpine `x-data="dro"`: a component registered with
    // `Alpine.data('dro', ...)`. The whole quoted value must be a
    // partial name, so `x-data="dropdown({ cou` falls out of context
    // once the call (or any expression) begins.
    if let Some(at) = line.rfind("x-data=") {
        // A name-ish char before means a longer attribute (`data-x-data=`).
        if !line[..at]
            .chars()
            .next_back()
            .is_some_and(|c| {
                c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':' | '.')
            })
        {
            let after = line[at + "x-data=".len()..].trim_start();
            if let Some(q) = after.chars().next() {
                if q == '"' || q == '\'' {
                    let value = &after[q.len_utf8()..];
                    if !value.contains(q)
                        && value.bytes().all(|b| {
                            b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-')
                        })
                    {
                        return Some(Kind::AlpineData);
                    }
                }
            }
        }
    }
    // Alpine/Livewire magic property (`$el`, `$store`, `$wire`, ...):
    // the line ends with `$` plus an optional partial identifier inside
    // an Alpine expression attribute value or a `wire:` attribute value.
    // Runs after `$store.` / `$refs.` so those keep their more specific
    // contexts, and before the `wire:` scan so `wire:click="$di` is not
    // read as a component method.
    {
        let b = line.as_bytes();
        let mut i = b.len();
        while i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_') {
            i -= 1;
        }
        // `i` is a char boundary: the scan only consumed ASCII bytes.
        if let Some(dollar) = line[..i].strip_suffix('$') {
            // `foo$`, `$$` and `$this->` are not magic starts, and a
            // dot (`$wire.$`, `$this.$`) is a member-of-object position,
            // not an Alpine magic one.
            let preceded = dollar
                .chars()
                .next_back()
                .is_some_and(|c| {
                    c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | '.')
                });
            // The partial must be empty or start a known magic name, so
            // an unrelated `$foo` does not raise an irrelevant list.
            let known_prefix = ALPINE_MAGICS
                .iter()
                .any(|(name, _)| name[1..].starts_with(&line[i..]));
            if !preceded && known_prefix {
                if let Some(attr) = quoted_value_attr(line) {
                    if alpine_expression_attr(&attr) || attr.starts_with("wire:") {
                        return Some(Kind::AlpineMagic);
                    }
                }
            }
        }
    }
    // Inside a real open tag (`<` after the last `>`)? Attribute-name
    // contexts (wire: / x-on: names) require it, so a Blade directive or
    // text content is not one.
    let in_tag = line.rfind('<') > line.rfind('>');
    // Livewire `wire:<event>(.modifier)*="..."`: an open attribute value.
    // No parenthesis involved, so this runs before the call-argument scan.
    let mut hay = line;
    while let Some(at) = hay.rfind("wire:") {
        // A name-ish char before `wire:` means it is part of a longer
        // attribute (`data-wire:...` or Alpine `x-on:...wire:...`).
        if hay[..at]
            .chars()
            .next_back()
            .is_some_and(|c| {
                c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '-' | '.')
            })
        {
            hay = &hay[..at];
            continue;
        }
        let after = &hay[at + 5..];
        let attr_len = after
            .find(|c: char| {
                !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
            })
            .unwrap_or(after.len());
        let attr = &after[..attr_len];
        let event = attr.split('.').next().unwrap_or("");
        let rest = &after[attr_len..];
        // Still inside the attribute name after a dot: a modifier follows.
        if rest.is_empty()
            && attr.contains('.')
            && (event == "model"
                || crate::livewire_handlers::EVENTS.contains(&event)
                || crate::livewire_handlers::MODIFIER_DIRECTIVES.contains(&event))
        {
            return Some(Kind::LivewireModifier);
        }
        // Still inside the attribute name with no dot or value: the
        // attribute name itself is being typed (`wire:lo` in a tag).
        if rest.is_empty() && !attr.contains('.') && in_tag {
            return Some(Kind::LivewireAttribute);
        }
        let quoted = rest
            .strip_prefix('=')
            .and_then(|r| r.chars().next().map(|q| (q, &r[q.len_utf8()..])));
        if let Some((q, value)) = quoted {
            if (q == '"' || q == '\'') && !value.contains(q) {
                // A helper call inside the value (`$dispatch('...`) wins
                // over the attribute's own model/action context.
                if let Some(kind) = call_context(line) {
                    return Some(kind);
                }
                if event == "model" {
                    return Some(Kind::LivewireModel);
                }
                // `wire:target="save, qu"`: the actions (and
                // wire:model properties) a wire:loading / wire:dirty
                // state is scoped to; comma-separated.
                if event == "target" {
                    return Some(Kind::LivewireTarget);
                }
                if crate::livewire_handlers::EVENTS.contains(&event) {
                    return Some(Kind::LivewireAction);
                }
            }
        }
        break;
    }
    // Alpine event attribute `x-on:<event>(.modifier)*` or `@<event>...`:
    // the line ends inside the attribute name. After a dot (and a known
    // DOM event) a modifier follows; otherwise the event name itself is
    // being typed. The event-name context needs a real tag (`<` after the
    // last `>`), so a Blade directive like `@foreach` is not one.
    for marker in ["x-on:", "@"] {
        let mut hay = line;
        while let Some(at) = hay.rfind(marker) {
            // A name-ish char before the marker means it is part of a
            // longer attribute; `@` also needs a non-name char before it,
            // which this same check enforces.
            if at > 0
                && hay[..at]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '-' | '.'))
            {
                hay = &hay[..at];
                continue;
            }
            let after = &hay[at + marker.len()..];
            let attr_len = after
                .find(|c: char| {
                    !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
                })
                .unwrap_or(after.len());
            let attr = &after[..attr_len];
            let event = attr.split('.').next().unwrap_or("");
            if attr_len == after.len() {
                if attr.contains('.') {
                    if DOM_EVENTS.contains(&event) {
                        return Some(Kind::AlpineModifier);
                    }
                } else if in_tag {
                    return Some(Kind::AlpineEvent);
                }
            }
            break;
        }
    }
    // Livewire component tag `<livewire:admin.user-profile`: the tail after
    // the marker is name characters only.
    if let Some(at) = line.rfind("<livewire:") {
        let tail = &line[at + "<livewire:".len()..];
        if tail
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        {
            return Some(Kind::LivewireTag);
        }
    }
    // Blade component tag `<x-<partial>`: the tail of the line is tag
    // characters (letters, digits, `.`, `-`, `_`) right after `<x-`.
    // Runs before the call-argument scan so a `(...)` earlier on the
    // line cannot misroute it.
    {
        let b = line.as_bytes();
        let mut i = b.len();
        while i > 0
            && matches!(b[i - 1], b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'-' | b'_')
        {
            i -= 1;
        }
        // The marker's `x-` is itself in the tail class, so the tail
        // starts right after `<` (i is a char boundary: the scan only
        // consumed ASCII bytes, and a non-class byte is never a UTF-8
        // continuation byte that ends mid-tail).
        if i >= 1
            && line.get(..i).is_some_and(|h| h.ends_with('<'))
            && line[i..].starts_with("x-")
            && !line[i..].starts_with("x-slot")
        {
            let ident_end = line[..i - 1]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
            if !ident_end {
                return Some(Kind::ComponentTag);
            }
        }
    }
    // Blade component attribute (prop) name: inside an `<x-...` tag
    // after its name, outside quotes, not at a value position. Runs
    // after the tag-name context, which wins while the caret is on the
    // name itself.
    if blade_prop_position(line).is_some() {
        return Some(Kind::BladeProp);
    }
    // Livewire mount property: `<livewire:user-profile em`.
    if livewire_prop_position(line).is_some() {
        return Some(Kind::LivewireProp);
    }
    call_context(line)
}

/// Completion items for the properties of the nearest enclosing
/// `x-data` object, offered inside Alpine expression attribute values
/// (`x-show="op`, `@click="to`, `:class="op`). `before_cursor` is the
/// buffer up to the cursor (only its last 16k characters are read);
/// `line_before` confirms the cursor sits inside an Alpine expression
/// value. Attribute values that started on an earlier line are not
/// recognised.
pub fn alpine_data_property_items(
    before_cursor: &str,
    line_before: &str,
    root: Option<&Path>,
) -> Vec<CompletionItem> {
    let Some(attr) = quoted_value_attr(line_before) else {
        return Vec::new();
    };
    if !alpine_expression_attr(&attr) {
        return Vec::new();
    }
    let Some(value) = enclosing_x_data_value(before_cursor) else {
        return Vec::new();
    };
    let keys = if value.trim_start().starts_with('{') {
        parse_data_keys(&value)
    } else {
        // A bare component name: resolve its `Alpine.data('name', ...)`
        // factory in the project (item 370).
        let name = value.trim();
        let is_name = !name.is_empty()
            && name.bytes().all(|b| {
                b.is_ascii_alphanumeric() || matches!(b, b'_' | b'$' | b'-')
            });
        if is_name {
            root.map(|r| data_component_keys(r, name))
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    };
    keys.into_iter()
        .take(MAX_ITEMS)
        .map(|n| CompletionItem {
            label: n,
            kind: Some(CompletionItemKind::PROPERTY),
            detail: Some("Alpine x-data property".to_string()),
            ..Default::default()
        })
        .collect()
}

/// The Alpine and Livewire magic properties offered after `$` inside
/// Alpine expression attribute values and `wire:` attribute values
/// (item 367).
const ALPINE_MAGICS: &[(&str, &str)] = &[
    ("$el", "Alpine magic: the element the expression is on"),
    ("$refs", "Alpine magic: elements tagged with x-ref"),
    ("$store", "Alpine magic: a global Alpine.store(...) value"),
    ("$watch", "Alpine magic: run a callback when a property changes"),
    ("$dispatch", "Alpine magic: dispatch a bubbling CustomEvent"),
    ("$nextTick", "Alpine magic: run after the pending DOM updates"),
    ("$root", "Alpine magic: the component root element"),
    ("$id", "Alpine magic: a page-unique id inside an x-id scope"),
    ("$data", "Alpine magic: the component's merged x-data object"),
    (
        "$wire",
        "Livewire magic: the component (properties, methods, $set/$toggle/$refresh/$parent)",
    ),
    (
        "$js",
        "Livewire magic: a function registered with $js('name', fn) in @script",
    ),
    (
        "$entangle",
        "Livewire magic: two-way bind an Alpine and a Livewire property",
    ),
];

/// The `$wire` object's own magics offered after `$wire.$` (item 371):
/// label, detail, and whether it is a method. Aligned with the `$wire`
/// hover (item 350).
const LIVEWIRE_WIRE_MAGICS: &[(&str, &str, bool)] = &[
    ("$set", "Livewire $wire magic: $set('prop', value) writes a property", true),
    ("$get", "Livewire $wire magic: $get('prop') reads a property", true),
    (
        "$toggle",
        "Livewire $wire magic: $toggle('prop') flips a boolean property",
        true,
    ),
    (
        "$refresh",
        "Livewire $wire magic: $refresh() re-renders the component",
        true,
    ),
    ("$parent", "Livewire $wire magic: the parent component", false),
    (
        "$js",
        "Livewire $wire magic: functions registered with $js('name', fn) in @script",
        false,
    ),
];

/// The Alpine and Livewire magic properties, offered after `$` inside
/// Alpine expression attribute values and `wire:` attribute values
/// (item 367). Inline `<script>` bodies are not a magic context.
pub fn alpine_magic_items() -> Vec<CompletionItem> {
    ALPINE_MAGICS
        .iter()
        .map(|(label, detail)| CompletionItem {
            label: label.to_string(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some(detail.to_string()),
            ..Default::default()
        })
        .collect()
}

/// The attribute name when `line` ends inside its quoted value
/// (`<div x-show="op` -> `x-show`). Quotes are not escaped inside HTML
/// attribute values, so a plain state machine is exact.
fn quoted_value_attr(line: &str) -> Option<String> {
    let mut in_quote: Option<u8> = None;
    let mut attr: Option<String> = None;
    for (i, &b) in line.as_bytes().iter().enumerate() {
        match in_quote {
            Some(q) => {
                if b == q {
                    in_quote = None;
                }
            }
            None if b == b'"' || b == b'\'' => {
                in_quote = Some(b);
                attr = attr_name_before(&line[..i]);
            }
            _ => {}
        }
    }
    in_quote?;
    attr
}

/// The `name` in `name="` right before a quote: letters, digits and the
/// attribute punctuation `@ : . - _` (`@click`, `x-on:click`, `:class`).
fn attr_name_before(before_quote: &str) -> Option<String> {
    let b = before_quote.trim_end().strip_suffix('=')?.trim_end();
    let name: String = b
        .chars()
        .rev()
        .take_while(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '@' | ':' | '.' | '-' | '_')
        })
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Attributes whose values are Alpine expressions (as opposed to
/// selectors, transition classes or plain strings).
fn alpine_expression_attr(attr: &str) -> bool {
    attr.starts_with('@')
        || attr.starts_with("x-on:")
        || attr.starts_with("x-bind:")
        || (attr.starts_with(':') && attr.len() > 1)
        || matches!(
            attr,
            "x-show"
                | "x-if"
                | "x-text"
                | "x-html"
                | "x-init"
                | "x-effect"
                | "x-model"
                | "x-modelable"
                | "x-for"
                | "x-id"
                | "x-intersect"
                | "x-trap"
        )
}

/// Keys of the innermost open tag's `x-data` object.
fn x_data_keys_at(before_cursor: &str) -> Vec<String> {
    enclosing_x_data_value(before_cursor)
        .map(|v| parse_data_keys(&v))
        .unwrap_or_default()
}

/// The `x-data` value of the innermost open tag carrying one. A naive
/// tag stack over the last 16k characters before the cursor: quoted
/// attribute values hide `>`, `</name` pops up to and including the
/// matching open tag, void and unclosed tags simply stay on the stack.
fn enclosing_x_data_value(before_cursor: &str) -> Option<String> {
    let tail_start = before_cursor
        .char_indices()
        .rev()
        .nth(16_384)
        .map(|(i, _)| i)
        .unwrap_or(0);
    let text = &before_cursor[tail_start..];
    let b = text.as_bytes();
    let mut stack: Vec<(String, String)> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' || i + 1 >= b.len() {
            i += 1;
            continue;
        }
        if b[i + 1] == b'/' {
            let name_end = text[i + 2..]
                .find(|c: char| {
                    !(c.is_ascii_alphanumeric() || matches!(c, '-' | ':' | '.'))
                })
                .map(|n| i + 2 + n)
                .unwrap_or(text.len());
            let name = &text[i + 2..name_end];
            if let Some(pos) = stack.iter().rposition(|(n, _)| n == name) {
                stack.truncate(pos);
            }
            i = text[i + 2..]
                .find('>')
                .map(|n| i + 2 + n + 1)
                .unwrap_or(text.len());
            continue;
        }
        if !b[i + 1].is_ascii_alphabetic() {
            i += 1;
            continue;
        }
        let name_end = text[i + 1..]
            .find(|c: char| {
                !(c.is_ascii_alphanumeric() || matches!(c, '-' | ':' | '.'))
            })
            .map(|n| i + 1 + n)
            .unwrap_or(text.len());
        let name = text[i + 1..name_end].to_string();
        // Tag text runs to the `>` outside quoted values, or to the
        // cursor when the tag is still open (it is being typed).
        let mut in_quote: Option<u8> = None;
        let mut j = name_end;
        while j < b.len() {
            match in_quote {
                Some(q) => {
                    if b[j] == q {
                        in_quote = None;
                    }
                }
                None if b[j] == b'"' || b[j] == b'\'' => in_quote = Some(b[j]),
                None if b[j] == b'>' => break,
                _ => {}
            }
            j += 1;
        }
        let tag_text = &text[i..j.min(b.len())];
        stack.push((name, tag_text.to_string()));
        i = j;
    }
    for (_, tag_text) in stack.iter().rev() {
        if let Some(value) = x_data_value(tag_text) {
            return Some(value.to_string());
        }
    }
    None
}

/// The quoted value of the first `x-data` attribute in `tag_text`.
fn x_data_value(tag_text: &str) -> Option<&str> {
    let at = tag_text.find("x-data=")? + "x-data=".len();
    let after = tag_text[at..].trim_start();
    let q = after.chars().next()?;
    if q != '"' && q != '\'' {
        return None;
    }
    let rest = &after[q.len_utf8()..];
    let end = rest.find(q)?;
    Some(&rest[..end])
}

/// Top-level keys of an `x-data` object literal: identifiers and quoted
/// keys before a top-level `:`, method names (`toggle() { ... }`), and
/// shorthand idents. Spread entries are skipped. A truncated object
/// (still being typed) yields the keys completed so far.
fn parse_data_keys(value: &str) -> Vec<String> {
    let Some(body) = value.trim().strip_prefix('{') else {
        return Vec::new();
    };
    let mut keys: Vec<String> = Vec::new();
    let mut seg_start = 0;
    let mut end = body.len();
    let mut depth = 0i32;
    let mut in_str: Option<char> = None;
    let mut escaped = false;
    for (idx, c) in body.char_indices() {
        if let Some(q) = in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                in_str = None;
            }
            continue;
        }
        match c {
            '\'' | '"' | '`' => in_str = Some(c),
            '{' | '[' | '(' => depth += 1,
            '}' if depth == 0 => {
                end = idx;
                break;
            }
            '}' | ']' | ')' => depth -= 1,
            ',' if depth == 0 => {
                push_data_key(&body[seg_start..idx], &mut keys);
                seg_start = idx + 1;
            }
            _ => {}
        }
    }
    push_data_key(&body[seg_start..end], &mut keys);
    keys
}

fn push_data_key(segment: &str, keys: &mut Vec<String>) {
    let seg = segment.trim();
    if seg.is_empty() || seg.starts_with("...") {
        return;
    }
    // The key is what precedes the first top-level `:`.
    let mut depth = 0i32;
    let mut in_str: Option<char> = None;
    let mut colon = None;
    for (idx, c) in seg.char_indices() {
        if let Some(q) = in_str {
            if c == q {
                in_str = None;
            }
            continue;
        }
        match c {
            '\'' | '"' | '`' => in_str = Some(c),
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth -= 1,
            ':' if depth == 0 => {
                colon = Some(idx);
                break;
            }
            _ => {}
        }
    }
    let key = match colon {
        Some(idx) => seg[..idx].trim(),
        None => seg,
    };
    let key = if key.len() >= 2
        && ((key.starts_with('\'') && key.ends_with('\''))
            || (key.starts_with('"') && key.ends_with('"')))
    {
        &key[1..key.len() - 1]
    } else if let Some(paren) = key.find('(') {
        key[..paren].trim()
    } else {
        key
    };
    let valid = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '$' | '-'));
    if valid && !keys.iter().any(|k| k == key) {
        keys.push(key.to_string());
    }
}

/// The caret at an attribute-name position inside an `<x-...` tag.
fn blade_prop_position(line: &str) -> Option<(&str, &str)> {
    tag_attr_position(line, "<x-")
}

/// The caret at an attribute-name position inside a `<livewire:...`
/// tag.
fn livewire_prop_position(line: &str) -> Option<(&str, &str)> {
    tag_attr_position(line, "<livewire:")
}

/// The component whose mount properties are being named at the end of
/// `line`: the tag of a `<livewire:...` attribute position, or the
/// first argument of `@livewire('name', ['...'])`.
fn livewire_prop_component(line: &str) -> Option<String> {
    if let Some((tag, _)) = livewire_prop_position(line) {
        return Some(tag.to_string());
    }
    let i = line.rfind("@livewire(")?;
    let after = line[i + "@livewire(".len()..].trim_start();
    let q = after.chars().next()?;
    if q != '\'' && q != '"' {
        return None;
    }
    let body = &after[q.len_utf8()..];
    let end = body.find(q)?;
    let name = &body[..end];
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
    {
        return None;
    }
    Some(name.to_string())
}

/// The component tag and attribute partial at the end of `line` when
/// the caret sits at an attribute-name position inside a component tag
/// opened with `marker` (`<x-` or `<livewire:`): the tag opened on this
/// same line, has not closed, the caret is outside quotes, and not
/// right after `=`, `/` or a closing quote. Attributes on later lines
/// of a multi-line tag are not seen (v1).
fn tag_attr_position<'a>(line: &'a str, marker: &str) -> Option<(&'a str, &'a str)> {
    let open = line.rfind(marker)?;
    // A name-ish char before `<` means a longer word (`foo<x-`).
    if line[..open]
        .chars()
        .next_back()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }
    let after = &line[open + marker.len()..];
    let name_end = after.find(|c: char| {
        !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    })?;
    let tag = &after[..name_end];
    // Caret on the name (the tag-name context wins), and `<x-slot...`
    // takes no props.
    if tag.is_empty()
        || (tag == "slot" && marker == "<x-")
        || after[name_end..].starts_with(':')
    {
        return None;
    }
    let rest = &after[name_end..];
    // The tag must not have closed, and the caret must be outside
    // quotes. A `>` inside a quoted value stays hidden.
    let mut quote: Option<u8> = None;
    for &b in rest.as_bytes() {
        match quote {
            Some(q) => {
                if b == q {
                    quote = None;
                }
            }
            None => match b {
                b'"' | b'\'' => quote = Some(b),
                b'>' => return None,
                _ => {}
            },
        }
    }
    if quote.is_some() {
        return None;
    }
    // The partial attribute name at the end of the line.
    let b = line.as_bytes();
    let mut i = b.len();
    while i > 0 && (b[i - 1].is_ascii_alphanumeric() || matches!(b[i - 1], b'-' | b'_')) {
        i -= 1;
    }
    // A value position (`foo=`, `foo="..."`) or a self-closing `/` is
    // not an attribute-name position, and right after a closing quote
    // (before the separating space) the popup stays shut.
    let before = &line[..i];
    if before.ends_with('"') || before.ends_with('\'') {
        return None;
    }
    let head = before.trim_end();
    if head.ends_with('=') || head.ends_with('/') {
        return None;
    }
    let partial = &line[i..];
    let before = head.as_bytes();
    let prev = |n: usize| before.get(before.len().wrapping_sub(n)).copied();
    // `@click` is an event listener and `x-data` an Alpine attribute,
    // not props; `wire:click` is a directive suffix (a `:` straight
    // after a name), unlike the `:disabled` binding prefix.
    if partial.starts_with("x-")
        || prev(1) == Some(b'@')
        || (prev(1) == Some(b':')
            && prev(2).is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_')))
    {
        return None;
    }
    Some((tag, partial))
}

/// Blueprint methods offered at `$table->` (item 382): the Laravel
/// 12.x migration column types, plus the index and table command
/// methods, cross-checked against `Illuminate\Database\Schema\Blueprint`.
/// Chain modifiers (`->nullable()`, `->default()`, `->change()`)
/// attach after a column call, not at the `$table->` position.
const BLUEPRINT_METHODS: &[(&str, &str)] = &[
    ("bigIncrements", "Blueprint column type"),
    ("bigInteger", "Blueprint column type"),
    ("binary", "Blueprint column type"),
    ("boolean", "Blueprint column type"),
    ("char", "Blueprint column type"),
    ("date", "Blueprint column type"),
    ("dateTime", "Blueprint column type"),
    ("dateTimeTz", "Blueprint column type"),
    ("decimal", "Blueprint column type"),
    ("double", "Blueprint column type"),
    ("dropColumn", "Blueprint command"),
    ("dropForeign", "Blueprint command"),
    ("dropFullText", "Blueprint command"),
    ("dropIndex", "Blueprint command"),
    ("dropMorphs", "Blueprint command"),
    ("dropPrimary", "Blueprint command"),
    ("dropRememberToken", "Blueprint command"),
    ("dropSoftDeletes", "Blueprint command"),
    ("dropSoftDeletesTz", "Blueprint command"),
    ("dropSpatialIndex", "Blueprint command"),
    ("dropTimestamps", "Blueprint command"),
    ("dropTimestampsTz", "Blueprint command"),
    ("dropUnique", "Blueprint command"),
    ("enum", "Blueprint column type"),
    ("float", "Blueprint column type"),
    ("foreign", "Blueprint index"),
    ("foreignId", "Blueprint column type"),
    ("foreignIdFor", "Blueprint column type"),
    ("foreignUlid", "Blueprint column type"),
    ("foreignUuid", "Blueprint column type"),
    ("fullText", "Blueprint index"),
    ("geography", "Blueprint column type"),
    ("geometry", "Blueprint column type"),
    ("id", "Blueprint column type"),
    ("increments", "Blueprint column type"),
    ("index", "Blueprint index"),
    ("integer", "Blueprint column type"),
    ("ipAddress", "Blueprint column type"),
    ("json", "Blueprint column type"),
    ("jsonb", "Blueprint column type"),
    ("longText", "Blueprint column type"),
    ("macAddress", "Blueprint column type"),
    ("mediumIncrements", "Blueprint column type"),
    ("mediumInteger", "Blueprint column type"),
    ("mediumText", "Blueprint column type"),
    ("morphs", "Blueprint column type"),
    ("nullableMorphs", "Blueprint column type"),
    ("nullableTimestamps", "Blueprint column type"),
    ("nullableUlidMorphs", "Blueprint column type"),
    ("nullableUuidMorphs", "Blueprint column type"),
    ("primary", "Blueprint index"),
    ("rememberToken", "Blueprint column type"),
    ("renameColumn", "Blueprint command"),
    ("renameIndex", "Blueprint command"),
    ("set", "Blueprint column type"),
    ("smallIncrements", "Blueprint column type"),
    ("smallInteger", "Blueprint column type"),
    ("softDeletes", "Blueprint column type"),
    ("softDeletesTz", "Blueprint column type"),
    ("spatialIndex", "Blueprint index"),
    ("string", "Blueprint column type"),
    ("text", "Blueprint column type"),
    ("time", "Blueprint column type"),
    ("timeTz", "Blueprint column type"),
    ("timestamp", "Blueprint column type"),
    ("timestampTz", "Blueprint column type"),
    ("timestamps", "Blueprint column type"),
    ("timestampsTz", "Blueprint column type"),
    ("tinyIncrements", "Blueprint column type"),
    ("tinyInteger", "Blueprint column type"),
    ("tinyText", "Blueprint column type"),
    ("ulid", "Blueprint column type"),
    ("ulidMorphs", "Blueprint column type"),
    ("unique", "Blueprint index"),
    ("unsignedBigInteger", "Blueprint column type"),
    ("unsignedInteger", "Blueprint column type"),
    ("unsignedMediumInteger", "Blueprint column type"),
    ("unsignedSmallInteger", "Blueprint column type"),
    ("unsignedTinyInteger", "Blueprint column type"),
    ("uuid", "Blueprint column type"),
    ("uuidMorphs", "Blueprint column type"),
    ("vector", "Blueprint column type"),
    ("year", "Blueprint column type"),
];

/// Some(Kind::ValidationRule) when the caret is at a rule-name
/// position inside the open rules string: the current `|`-separated
/// segment must not contain `:` - a rule parameter (`max:10`) takes
/// no completion. `line` is already known to end inside a string.
fn validation_rule_context(line: &str) -> Option<Kind> {
    let q = line.rfind(|c| c == '\'' || c == '"')?;
    let token = line[q + 1..].rsplit('|').next()?;
    // `exists:` / `unique:` parameters (items 388, 390), grammar from
    // Laravel 12.x ValidatesAttributes: exists takes
    // `table,column,column,value,...` (condition pairs from index 2);
    // unique takes `table,column,except,idColumn,column,value,...`
    // (pairs from index 4). Index 0 is the table; columns are offered
    // at the column positions only - value positions (the unique
    // except, the condition values) and an empty table stay silent.
    let stripped = token
        .strip_prefix("exists:")
        .map(|p| (true, p))
        .or_else(|| token.strip_prefix("unique:").map(|p| (false, p)));
    if let Some((is_exists, param)) = stripped {
        let index = param.matches(',').count();
        if index == 0 {
            return Some(Kind::DbTable);
        }
        if param.starts_with(',') {
            return None;
        }
        let column_position = if is_exists {
            index == 1 || index % 2 == 0
        } else {
            index == 1 || index == 3 || (index >= 4 && index % 2 == 0)
        };
        return if column_position {
            Some(Kind::DbColumn)
        } else {
            None
        };
    }
    if token.contains(':') {
        return None;
    }
    Some(Kind::ValidationRule)
}

/// The table named by the base of the query-builder chain that the
/// call following `head` belongs to (item 392). `head` is the text
/// before the call's method name and ends with `->`. The receiver is
/// resolved by walking backward over whole `->method(...)` segments
/// on comment/string-masked text, so a table named inside a
/// completed subquery (`->whereIn('id', DB::table('posts')...)`)
/// or by an earlier statement on the line is never inherited; only a
/// chain whose base is a literal `DB::table('...')` qualifies.
fn chain_table(head: &str) -> Option<&str> {
    let (masked, _) = mask_comments(head);
    let b = masked.as_bytes();
    let mut i = masked.trim_end().len();
    if !masked[..i].ends_with("->") {
        return None;
    }
    i -= 2;
    loop {
        while i > 0 && b[i - 1].is_ascii_whitespace() {
            i -= 1;
        }
        // Every consumed chain segment is a `method(...)` call; a
        // property, variable or statement boundary ends the chain
        // without a table.
        if i == 0 || b[i - 1] != b')' {
            return None;
        }
        let mut depth = 0usize;
        let mut j = i;
        while j > 0 {
            j -= 1;
            match b[j] {
                b')' => depth += 1,
                b'(' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
        if depth != 0 {
            return None;
        }
        let mut k = j;
        while k > 0 && (b[k - 1].is_ascii_alphanumeric() || b[k - 1] == b'_')
        {
            k -= 1;
        }
        let method = &masked[k..j];
        let mut m = k;
        while m > 0 && b[m - 1].is_ascii_whitespace() {
            m -= 1;
        }
        if masked[..m].ends_with("->") {
            i = m - 2;
            continue;
        }
        if method == "table" && masked[..m].ends_with("::") {
            let pre = masked[..m - 2].trim_end();
            let db_ok = pre.ends_with("DB")
                && (pre.len() == 2 || {
                    let c = pre.as_bytes()[pre.len() - 3];
                    !c.is_ascii_alphanumeric() && c != b'_'
                });
            if db_ok {
                return quoted_word(head[j + 1..].trim_start());
            }
        }
        return None;
    }
}

/// Is `h` (the text before the method name) exactly `Name::` - not
/// a longer identifier like `MyName::`?
fn static_head(h: &str, name: &str) -> bool {
    let suffix = format!("{name}::");
    let Some(pre) = h.trim_end().strip_suffix(suffix.as_str()) else {
        return false;
    };
    !pre.chars()
        .last()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Is `h` (the text before the method name) exactly a `Rule::`
/// receiver - not `MyRule::` or a longer identifier?
fn rule_head(h: &str) -> bool {
    let Some(pre) = h.trim_end().strip_suffix("Rule::") else {
        return false;
    };
    !pre.chars()
        .last()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The opening `(` of the last call on the line, unless it sits
/// inside a comment or a string literal (item 394): the completion
/// string itself is open, so a call `(` is never its own span.
fn code_paren(line: &str) -> Option<usize> {
    let (masked, strings) = mask_comments(line);
    let paren = masked.rfind('(')?;
    let i = strings.partition_point(|&(_, end)| paren > end);
    if i < strings.len() && strings[i].0 <= paren {
        return None;
    }
    Some(paren)
}

/// The table named by the first argument of the line's open
/// `Rule::exists('TABLE', ...)` / `Rule::unique('TABLE', ...)` call
/// (item 394). Extraction is tied to the actual call opening - the
/// last code `(` - so an earlier `Rule::` mention on the line never
/// donates its table to the current call, and a table given as a
/// variable fails closed.
fn fluent_rule_table(line: &str) -> Option<&str> {
    let paren = code_paren(line)?;
    let head = line[..paren].trim_end();
    let word_start = head
        .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .map(|x| x + 1)
        .unwrap_or(0);
    if !matches!(&head[word_start..], "exists" | "unique")
        || !rule_head(&head[..word_start])
    {
        return None;
    }
    quoted_word(line[paren + 1..].trim_start())
}

/// The table named by an open `exists:TABLE,` / `unique:TABLE,` rule
/// segment at the end of `line` (caret after the first comma).
fn rule_param_table(line: &str) -> Option<&str> {
    let q = line.rfind(|c| c == '\'' || c == '"')?;
    let token = line[q + 1..].rsplit('|').next()?;
    let param = token
        .strip_prefix("exists:")
        .or_else(|| token.strip_prefix("unique:"))?;
    let (table, _) = param.split_once(',')?;
    if table.is_empty() {
        None
    } else {
        Some(table)
    }
}

/// The context of an open quoted call argument at the end of `line`
/// (`route('...`, `$dispatch('...`): the last `(` whose argument string
/// is still open, in the argument the helper actually takes the name in
/// (almost always the first). Quote escapes are not read;
/// `route('it\'s')` is rare and simply falls out of context.
fn call_context(line: &str) -> Option<Kind> {
    let paren = line.rfind('(')?;
    let before = &line[..paren];
    // Still inside the argument string: quote state at end of line is
    // open. Track which argument the cursor is in: commas separate
    // arguments only at bracket depth zero, so the array in
    // `@includeFirst(['one', 'two'])` stays inside its first argument.
    let mut in_str: Option<char> = None;
    let mut depth: i32 = 0;
    let mut arg = 0usize;
    // How the currently open string was opened: the non-space character
    // before its quote and the bracket depth there. `@livewire('n',
    // ['prop' => ...])` uses it to tell array keys from values.
    let mut str_opener: Option<(u8, i32)> = None;
    let mut prev_non_space: Option<u8> = None;
    for c in line[paren + 1..].chars() {
        match in_str {
            Some(q) => {
                if c == q {
                    in_str = None;
                    str_opener = None;
                }
            }
            None => match c {
                '\'' | '"' => {
                    in_str = Some(c);
                    str_opener = prev_non_space.map(|b| (b, depth));
                }
                '[' | '(' | '{' => depth += 1,
                ']' | '}' => depth -= 1,
                ')' if depth == 0 => return None,
                ')' => depth -= 1,
                ',' if depth == 0 => arg += 1,
                _ => {}
            },
        }
        if !c.is_whitespace() {
            prev_non_space = if c.is_ascii() {
                Some(c as u8)
            } else {
                None
            };
        }
    }
    in_str?;
    let head = before.trim_end();
    let word_start = head
        .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .map(|x| x + 1)
        .unwrap_or(0);
    let word = &head[word_start..];
    let blade = head[..word_start].ends_with('@');
    Some(match word {
        "route" | "to_route" | "routeIs" | "path" | "generateUrl"
        | "redirectToRoute" | "generate" if arg == 0 => Kind::Route,
        // Route::has('...') - but not $collection->has('...').
        "has" if head[..word_start].ends_with("Route::") && arg == 0 => {
            Kind::Route
        }
        "config" if arg == 0 => Kind::Config,
        "env" | "getenv" if arg == 0 => Kind::Env,
        "view" if arg == 0 => Kind::View,
        // Vite entry points (item 398): `Vite::asset('...')` takes
        // one root-relative source path (Laravel 12.x
        // Foundation\Vite::asset($asset, ...)). Must come before the
        // bare `asset(...)` arm, whose public-dir list is a
        // different set.
        "asset"
            if static_head(&head[..word_start], "Vite")
                && arg == 0
                && code_paren(line) == Some(paren)
                && str_opener.is_none() =>
        {
            Kind::ViteEntry
        }
        // `@vite('resources/js/app.js')` /
        // `@vite(['resources/css/app.css', ...])`: the directive
        // takes one entry or a list (Foundation\Vite::__invoke).
        // `blade` here is only the preceding-@ shape; items() gates
        // the directive on the file being `*.blade.php` (item 398).
        "vite"
            if blade
                && arg == 0
                && (str_opener.is_none()
                    || matches!(str_opener, Some((b'[', 1)))
                    || matches!(str_opener, Some((b',', 1)))) =>
        {
            Kind::ViteEntry
        }
        "asset" | "secure_asset" | "mix" if arg == 0 => Kind::Asset,
        "can" | "cannot" | "canany" if blade && arg == 0 => Kind::Gate,
        "can" | "cannot" | "authorize"
            if head[..word_start].ends_with("->") && arg == 0 =>
        {
            Kind::Gate
        }
        "allows" | "denies" | "any" | "authorize" | "check" | "none"
            if head[..word_start].ends_with("Gate::") && arg == 0 =>
        {
            Kind::Gate
        }
        // Blade directives take view names too - in the first argument,
        // except `@includeWhen($cond, 'view')`, whose view is second.
        "extends" | "include" | "includeIf" | "includeFirst" | "each"
            if blade && arg == 0 => Kind::View,
        "includeWhen" if blade && arg == 1 => Kind::View,
        // Livewire full-page components:
        // `view('livewire.counter')->layout('layouts.app')` names a
        // layout view and `->section('content')` a section of it.
        "layout" if head[..word_start].ends_with("->") && arg == 0 => {
            Kind::View
        }
        "section" | "yield" if blade && arg == 0 => Kind::Section,
        "section" if head[..word_start].ends_with("->") && arg == 0 => {
            Kind::Section
        }
        "push" | "pushOnce" | "stack" if blade && arg == 0 => Kind::Stack,
        "__" | "trans" | "trans_choice" if arg == 0 => Kind::Lang,
        "lang" | "choice" if blade && arg == 0 => Kind::Lang,
        "livewire" if blade && arg == 0 => Kind::LivewireName,
        // `@livewire('name', ['prop' => ...])`: mount property keys in
        // the props array - a string opened by `[` or by a comma inside
        // the array, not a value after `=>`.
        "livewire"
            if blade
                && arg == 1
                && (matches!(str_opener, Some((b'[', _)))
                    || matches!(str_opener, Some((b',', d)) if d >= 1)) =>
        {
            Kind::LivewireProp
        }
        // Validation rules (item 378): a rule-name position inside the
        // rules string of `->validate([...])` - a value after `=>` in
        // the rules array (pipe form), or an entry of a nested
        // `['required', '...']` rule array one level deeper.
        "validate"
            if arg == 0
                && (matches!(str_opener, Some((b'>', d)) if d >= 1)
                    || matches!(str_opener, Some((b'[', d)) if d >= 2)
                    || matches!(str_opener, Some((b',', d)) if d >= 2)) =>
        {
            return validation_rule_context(line);
        }
        // The Livewire `#[Validate('...')]` attribute takes the rules
        // directly: a string right after the paren (no character in
        // between, so no recorded opener), or an entry of a rules
        // array at depth one.
        "Validate"
            if arg == 0
                && (str_opener.is_none()
                    || matches!(str_opener, Some((b'[', 1)))
                    || matches!(str_opener, Some((b',', 1)))) =>
        {
            return validation_rule_context(line);
        }
        // `Validator::make($data, [...])`: the rules array is second.
        "make"
            if head[..word_start].ends_with("Validator::")
                && arg == 1
                && (matches!(str_opener, Some((b'>', d)) if d >= 1)
                    || matches!(str_opener, Some((b'[', d)) if d >= 2)
                    || matches!(str_opener, Some((b',', d)) if d >= 2)) =>
        {
            return validation_rule_context(line);
        }
        // `$validator->sometimes('reason', 'required|...')`: the rules
        // string is the second argument, a bare string.
        "sometimes" if arg == 1 && matches!(str_opener, Some((b',', 0))) => {
            return validation_rule_context(line);
        }
        // Livewire events: `$dispatch('...')`, `$this->dispatch('...')`,
        // `$wire.dispatch('...')`, `Livewire.dispatch('...')` and the
        // Livewire 2 `->emit(...)` family. The event name is the first
        // argument; a payload is not an event name.
        "dispatch" | "emit" | "emitTo" | "dispatchBrowserEvent" if arg == 0 => {
            Kind::LivewireEvent
        }
        // Alpine/Livewire entanglement: `$entangle('...')`,
        // `$wire.entangle('...')`. The property is the first argument;
        // the live flag is not a property.
        "entangle" if arg == 0 => Kind::LivewireEntangle,
        // Livewire property watching: `$wire.$watch('...')`. A bare
        // Alpine `$watch('...')` watches x-data keys, which this
        // line-only context cannot see, so it stays out of context.
        "watch" if head[..word_start].ends_with("$wire.$") && arg == 0 => {
            Kind::LivewireWatch
        }
        "get" | "has" | "choice"
            if head[..word_start].ends_with("Lang::") && arg == 0 =>
        {
            Kind::Lang
        }
        // The View facade (item 396): the static twin of
        // `view('...')` - `View::make('name', ...)` and
        // `View::exists('name')` take the view name first, and
        // `View::first(['a', 'b'])` takes a list of view names
        // (Laravel 12.x Factory: make($view, ...), exists($view),
        // first(array $views, ...)). A scalar `View::first('...')`
        // is not a supported form; `make`/`exists` array strings
        // are data, not names.
        "make" | "exists"
            if static_head(&head[..word_start], "View")
                && arg == 0
                && code_paren(line) == Some(paren)
                && str_opener.is_none() =>
        {
            Kind::View
        }
        "first"
            if static_head(&head[..word_start], "View")
                && arg == 0
                && code_paren(line) == Some(paren)
                && (matches!(str_opener, Some((b'[', 1)))
                    || matches!(str_opener, Some((b',', 1)))) =>
        {
            Kind::View
        }
        // Artisan commands (item 408): `Artisan::call('...')` /
        // `Artisan::queue('...')` take the command name as the scalar
        // first argument (Laravel 12.x Console Kernel contract:
        // call($command, ...), queue($command, ...)).
        "call" | "queue"
            if static_head(&head[..word_start], "Artisan")
                && arg == 0
                && code_paren(line) == Some(paren)
                && str_opener.is_none() =>
        {
            Kind::ArtisanCommand
        }
        // Broadcast connections (item 407):
        // `Broadcast::connection('...')` / `Broadcast::driver('...')`
        // take the connection name as the scalar first argument
        // (Laravel 12.x BroadcastManager: connection($driver =
        // null), driver($name = null) alias).
        "connection" | "driver"
            if static_head(&head[..word_start], "Broadcast")
                && arg == 0
                && code_paren(line) == Some(paren)
                && str_opener.is_none() =>
        {
            Kind::BroadcastConnection
        }
        // Queue connections (item 406): `Queue::connection('...')`
        // takes the connection name as the scalar first argument
        // (Laravel 12.x QueueManager: connection($name = null)).
        "connection"
            if static_head(&head[..word_start], "Queue")
                && arg == 0
                && code_paren(line) == Some(paren)
                && str_opener.is_none() =>
        {
            Kind::QueueConnection
        }
        // Redis connections (item 405): `Redis::connection('...')`
        // takes the connection or cluster name as the scalar first
        // argument (Laravel 12.x RedisManager: connection($name =
        // null), resolve($name) also answering cluster names).
        "connection"
            if static_head(&head[..word_start], "Redis")
                && arg == 0
                && code_paren(line) == Some(paren)
                && str_opener.is_none() =>
        {
            Kind::RedisConnection
        }
        // Database connections (item 404): `DB::connection('...')`
        // takes the connection name as the scalar first argument
        // (Laravel 12.x DatabaseManager: connection($name = null)).
        "connection"
            if static_head(&head[..word_start], "DB")
                && arg == 0
                && code_paren(line) == Some(paren)
                && str_opener.is_none() =>
        {
            Kind::DbConnection
        }
        // Auth guards (item 402): `Auth::guard('...')` and
        // `Auth::shouldUse('...')` take the guard name as the
        // scalar first argument (Laravel 12.x AuthManager:
        // guard($name = null), shouldUse($name)).
        "guard" | "shouldUse"
            if static_head(&head[..word_start], "Auth")
                && arg == 0
                && code_paren(line) == Some(paren)
                && str_opener.is_none() =>
        {
            Kind::AuthGuard
        }
        // Mailers (item 401): `Mail::mailer('...')` /
        // `Mail::driver('...')` take the mailer name as the scalar
        // first argument (Laravel 12.x MailManager:
        // mailer($name = null), driver($driver = null) alias).
        "mailer" | "driver"
            if static_head(&head[..word_start], "Mail")
                && arg == 0
                && code_paren(line) == Some(paren)
                && str_opener.is_none() =>
        {
            Kind::MailMailer
        }
        // Cache stores (item 400): `Cache::store('...')` /
        // `Cache::driver('...')` take the store name as the scalar
        // first argument (Laravel 12.x CacheManager:
        // store($name = null), driver($driver = null) alias). An
        // array argument is an on-demand config, not a name list.
        "store" | "driver"
            if static_head(&head[..word_start], "Cache")
                && arg == 0
                && code_paren(line) == Some(paren)
                && str_opener.is_none() =>
        {
            Kind::CacheStore
        }
        // Log channels (item 399): `Log::channel('...')` /
        // `Log::driver('...')` take the name as the scalar first
        // argument, `Log::stack([...])` takes a list of names
        // (Laravel 12.x LogManager: channel($channel = null),
        // driver($driver = null), stack(array $channels, ...)) - a
        // scalar stack() is not a supported form. The receiver must
        // be exactly `Log::`.
        "channel" | "driver"
            if static_head(&head[..word_start], "Log")
                && arg == 0
                && code_paren(line) == Some(paren)
                && str_opener.is_none() =>
        {
            Kind::LogChannel
        }
        "stack"
            if static_head(&head[..word_start], "Log")
                && arg == 0
                && code_paren(line) == Some(paren)
                && (matches!(str_opener, Some((b'[', 1)))
                    || matches!(str_opener, Some((b',', 1)))) =>
        {
            Kind::LogChannel
        }
        // Filesystem disks (item 397): `Storage::disk('...')` /
        // `Storage::drive('...')` take the disk name as the scalar
        // first argument (Laravel 12.x FilesystemManager:
        // disk($name = null), drive($name = null) alias).
        // `Storage::build(...)` is NOT a disk-name position - its
        // non-array argument is a local root path - so it stays
        // silent. The receiver must be exactly `Storage::`, and a
        // `(` inside a comment or string is not a call.
        "disk" | "drive"
            if static_head(&head[..word_start], "Storage")
                && arg == 0
                && code_paren(line) == Some(paren)
                && str_opener.is_none() =>
        {
            Kind::StorageDisk
        }
        // The Config facade (item 395): the static twin of
        // `config('...')` - `Config::get/has/set('key', ...)` all
        // take the key first (Laravel 12.x Repository: get($key,
        // $default), has($key), set($key, $value)). In the array
        // forms a string is a key only when it opens after `[` or
        // `,` at depth exactly 1, the outer key array (a scalar
        // first-arg string records no opener: the quote is the first
        // thing after the paren, and `prev_non_space` starts unset) -
        // getMany
        // treats assoc keys as keys and assoc values as defaults,
        // and set takes key => value pairs - so a string after `=>`
        // or inside a nested array (depth 2+) never completes. The
        // receiver must be
        // exactly `Config::`, and a `(` inside a comment or string
        // is not a call.
        "get" | "has" | "set"
            if static_head(&head[..word_start], "Config")
                && arg == 0
                && code_paren(line) == Some(paren)
                && (str_opener.is_none()
                    || matches!(str_opener, Some((b'[', 1)))
                    || matches!(str_opener, Some((b',', 1)))) =>
        {
            Kind::Config
        }
        // Database tables (item 381): `Schema::table('...')` and drops,
        // `DB::table('...')`, foreign references
        // (`->constrained('users')`, `->on('users')`) and query joins.
        // `Schema::create('...')` names a NEW table, so it stays out.
        "table" | "drop" | "dropIfExists"
            if head[..word_start].ends_with("Schema::") && arg == 0 =>
        {
            Kind::DbTable
        }
        "table" if head[..word_start].ends_with("DB::") && arg == 0 => {
            Kind::DbTable
        }
        "constrained" | "on" | "join" | "leftJoin" | "rightJoin"
        | "crossJoin"
            if head[..word_start].ends_with("->") && arg == 0 =>
        {
            Kind::DbTable
        }
        // Query-builder columns (item 392): a column-name first
        // argument, only when the call's own chain is based on a
        // literal `DB::table('...')` earlier on the line.
        // `Model::where('...')`, table-less lines and receivers the
        // backward walk cannot prove stay silent - no model-to-table
        // or cross-statement guessing.
        "where" | "orWhere" | "whereIn" | "whereNotIn" | "whereNull"
        | "whereNotNull" | "whereBetween" | "orderBy" | "groupBy"
        | "select" | "pluck" | "value" | "latest" | "oldest"
            if head[..word_start].ends_with("->") && arg == 0 =>
        {
            if chain_table(&head[..word_start]).is_some() {
                Kind::QueryColumn
            } else {
                return None;
            }
        }
        // Middleware aliases (item 393): a quoted string in the
        // first argument of `->middleware(...)` /
        // `Route::middleware(...)` / `->withoutMiddleware(...)`,
        // including `['auth', '...']` array entries (the bracket
        // comma reads as depth, not the next argument). Only inside
        // an open string - class constants stay with normal
        // completion - and the `alias:parameter` suffix is a value,
        // not an alias.
        "middleware" | "withoutMiddleware" if arg == 0 => {
            let h = head[..word_start].trim_end();
            let route_static = h.ends_with("::") && {
                let pre = h[..h.len() - 2].trim_end();
                pre.ends_with("Route")
                    && (pre.len() == 5 || {
                        let c = pre.as_bytes()[pre.len() - 6];
                        !c.is_ascii_alphanumeric() && c != b'_'
                    })
            };
            if !(h.ends_with("->") || route_static) {
                return None;
            }
            let Some(q) = in_str else {
                return None;
            };
            let Some(open) = line.rfind(q) else {
                return None;
            };
            if line[open + 1..].contains(':') {
                return None;
            }
            Kind::MiddlewareAlias
        }
        // Fluent validation rules (item 394): `Rule::exists('users',
        // ...)` / `Rule::unique('users', ...)` take the table first,
        // then the column (Laravel 12.x: `exists($table, $column)` /
        // `unique($table, $column)`). `->exists(...)` on a query
        // builder, `MyRule::exists(...)`, a call `(` inside a comment
        // or string, and a table given as a variable stay silent
        // (the context check mirrors the items() extraction).
        "exists" | "unique"
            if rule_head(&head[..word_start])
                && arg == 0
                && code_paren(line) == Some(paren) =>
        {
            Kind::DbTable
        }
        "exists" | "unique"
            if rule_head(&head[..word_start])
                && arg == 1
                && code_paren(line) == Some(paren) =>
        {
            if fluent_rule_table(line).is_some() {
                Kind::DbColumn
            } else {
                return None;
            }
        }
        _ => return None,
    })
}

#[derive(Clone)]
struct Lists {
    root: PathBuf,
    at: Instant,
    routes: Vec<String>,
    configs: Vec<String>,
    envs: Vec<String>,
    views: Vec<String>,
    sections: Vec<String>,
    stacks: Vec<String>,
    langs: Vec<String>,
    components: Vec<String>,
    assets: Vec<String>,
    gates: Vec<String>,
    tables: Vec<String>,
    columns: Vec<(String, String)>,
    middleware_aliases: Vec<String>,
    disks: Vec<String>,
    vite_entries: Vec<String>,
    log_channels: Vec<String>,
    cache_stores: Vec<String>,
    mail_mailers: Vec<String>,
    auth_guards: Vec<String>,
    db_connections: Vec<String>,
    redis_connections: Vec<String>,
    queue_connections: Vec<String>,
    broadcast_connections: Vec<String>,
    artisan_commands: Vec<String>,
}

static CACHE: OnceLock<Mutex<Option<Lists>>> = OnceLock::new();

/// Table names from `Schema::create('...')` in database/migrations
/// (item 381). Sorted, deduped; `Schema::connection(...)->create(...)`
/// is not seen.
fn table_names(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(root.join("database/migrations"))
    else {
        return out;
    };
    let mut files: Vec<PathBuf> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "php"))
        .collect();
    files.sort();
    files.truncate(MAX_ROUTE_FILES);
    for f in files {
        let Ok(meta) = std::fs::metadata(&f) else { continue };
        if !meta.is_file() || meta.len() > MAX_FILE {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        for (i, _) in text.match_indices("Schema::create(") {
            let rest = text[i + "Schema::create(".len()..].trim_start();
            let Some(q) = rest.chars().next().filter(|c| *c == '\'' || *c == '"')
            else {
                continue;
            };
            let Some(end) = rest[1..].find(q) else { continue };
            let name = &rest[1..1 + end];
            if !name.is_empty()
                && name.len() <= 100
                && !out.iter().any(|n: &String| n == name)
                && out.len() < MAX_NAMES
            {
                out.push(name.to_string());
            }
        }
    }
    out.sort();
    out
}

/// `(table, column)` pairs from `Schema::create/table` closures in
/// database/migrations (item 390): column-type Blueprint methods with
/// an explicit first name argument, plus the implicit columns of
/// `id()`, `timestamps()`, `softDeletes()`, `rememberToken()` and the
/// `morphs` family. Index, foreign, drop and rename commands are not
/// harvested, so a dropped column can linger. Sorted, deduped.
fn table_columns(root: &Path) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let Ok(rd) = std::fs::read_dir(root.join("database/migrations"))
    else {
        return out;
    };
    let mut files: Vec<PathBuf> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "php"))
        .collect();
    files.sort();
    files.truncate(MAX_ROUTE_FILES);
    for f in files {
        let Ok(meta) = std::fs::metadata(&f) else { continue };
        if !meta.is_file() || meta.len() > MAX_FILE {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        if text.contains("<<<") {
            // Heredoc/nowdoc strings are not masked (item 390): fail
            // closed - omit this file's columns rather than risk
            // inventing phantom ones from heredoc content.
            continue;
        }
        harvest_columns(&text, &mut out);
        if out.len() >= MAX_NAMES {
            out.truncate(MAX_NAMES);
            break;
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Middleware alias and group names (item 393): the top-level string
/// keys of the declaration arrays in `bootstrap/app.php` (Laravel 11+
/// `->alias([...])`) and `app/Http/Kernel.php` (`$middlewareAliases`,
/// `$routeMiddleware`, `$middlewareGroups`). Only those arrays are
/// scanned - an unrelated `'name' => ...` config entry elsewhere in
/// the file is not an alias. Sorted, deduped; aliases the framework
/// or a package registers itself are not offered.
fn middleware_aliases(root: &Path) -> Vec<String> {
    const FILES: &[&str] = &["bootstrap/app.php", "app/Http/Kernel.php"];
    const KERNEL_PROPS: &[&str] =
        &["$middlewareAliases", "$routeMiddleware", "$middlewareGroups"];
    let mut out = Vec::new();
    for f in FILES {
        let p = root.join(f);
        let Ok(meta) = std::fs::metadata(&p) else {
            continue;
        };
        if !meta.is_file() || meta.len() > MAX_FILE {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else {
            continue;
        };
        if text.contains("<<<") {
            // Same heredoc/nowdoc fail-closed guard as item 390d.
            continue;
        }
        let (masked, strings) = mask_comments(&text);
        let b = masked.as_bytes();
        let in_string = |pos: usize| {
            let i = strings.partition_point(|&(_, end)| pos > end);
            i < strings.len() && strings[i].0 <= pos
        };
        // The `[` opening each declaration array.
        let mut regions: Vec<usize> = Vec::new();
        for (i, _) in masked.match_indices("->alias(") {
            if in_string(i) {
                continue;
            }
            let mut j = i + "->alias(".len();
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            if b.get(j) == Some(&b'[') {
                regions.push(j);
            }
        }
        for prop in KERNEL_PROPS {
            for (i, _) in masked.match_indices(prop) {
                if in_string(i) {
                    continue;
                }
                let mut j = i + prop.len();
                while j < b.len() && b[j].is_ascii_whitespace() {
                    j += 1;
                }
                // Only the `= [` definition; a `$this->...` usage or
                // a non-array default declares nothing.
                if b.get(j) != Some(&b'=') {
                    continue;
                }
                j += 1;
                while j < b.len() && b[j].is_ascii_whitespace() {
                    j += 1;
                }
                if b.get(j) == Some(&b'[') {
                    regions.push(j);
                }
            }
        }
        for open in regions {
            harvest_alias_keys(&masked, &text, &strings, open, &mut out);
            if out.len() >= MAX_NAMES {
                break;
            }
        }
        if out.len() >= MAX_NAMES {
            break;
        }
    }
    out.sort();
    out.dedup();
    out.truncate(MAX_NAMES);
    out
}

/// The top-level string keys of the `'{key}' => [...]` array in
/// `config/{file}.php` (items 397, 399): each key is a named driver
/// entry (a filesystem disk, a log channel, ...). Only that array is
/// scanned: sibling keys, per-entry settings one level deeper and
/// commented-out entries invent nothing. Sorted, deduped.
fn config_named_keys(root: &Path, file: &str, key: &str) -> Vec<String> {
    let p = root.join("config").join(format!("{file}.php"));
    let Ok(meta) = std::fs::metadata(&p) else {
        return Vec::new();
    };
    if !meta.is_file() || meta.len() > MAX_FILE {
        return Vec::new();
    }
    let Ok(text) = std::fs::read_to_string(&p) else {
        return Vec::new();
    };
    if text.contains("<<<") {
        // Same heredoc/nowdoc fail-closed guard as item 390d.
        return Vec::new();
    }
    let (masked, strings) = mask_comments(&text);
    let mut out = Vec::new();
    if let Some(open) =
        find_region(&masked, &text, &strings, key, 0, masked.len())
    {
        harvest_alias_keys(&masked, &text, &strings, open, &mut out);
    }
    out.sort();
    out.dedup();
    out.truncate(MAX_NAMES);
    out
}

/// Find the open bracket of the first `'key' => [` region whose key
/// string starts in [`from`, `before`). Returns the masked-text offset
/// of the `[`. Key strings are matched on their real text; the `=> [`
/// check runs on the comment-masked text (item 405).
fn find_region(
    masked: &str,
    text: &str,
    strings: &[(usize, usize)],
    key: &str,
    from: usize,
    before: usize,
) -> Option<usize> {
    for (start, end) in strings {
        if *start < from || *start >= before {
            continue;
        }
        if &text[start + 1..*end] != key {
            continue;
        }
        let after = masked[end + 1..].trim_start();
        let Some(rest) = after.strip_prefix("=>") else {
            continue;
        };
        if !rest.trim_start().starts_with('[') {
            continue;
        }
        return Some(masked.len() - rest.trim_start().len());
    }
    None
}

/// The offset just past the `]` closing the array opened at `open`
/// (item 405). Bracket depth walk on the comment-masked text; quoted
/// strings inside are skipped via the recorded spans.
fn region_end(
    masked: &str,
    strings: &[(usize, usize)],
    open: usize,
) -> usize {
    let b = masked.as_bytes();
    let mut i = open + 1;
    let mut depth = 1usize;
    while i < b.len() && depth > 0 {
        match b[i] {
            b'[' | b'(' | b'{' => {
                depth += 1;
                i += 1;
            }
            b']' | b')' | b'}' => {
                depth -= 1;
                i += 1;
            }
            q if q == b'\'' || q == b'"' => {
                if let Ok(si) =
                    strings.binary_search_by_key(&i, |s| s.0)
                {
                    i = strings[si].1 + 1;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    i
}

/// Filesystem disk names (item 397).
fn filesystem_disks(root: &Path) -> Vec<String> {
    config_named_keys(root, "filesystems", "disks")
}

/// Log channel names (item 399).
fn log_channels(root: &Path) -> Vec<String> {
    config_named_keys(root, "logging", "channels")
}

/// Cache store names (item 400).
fn cache_stores(root: &Path) -> Vec<String> {
    config_named_keys(root, "cache", "stores")
}

/// Mailer names (item 401).
fn mail_mailers(root: &Path) -> Vec<String> {
    config_named_keys(root, "mail", "mailers")
}

/// Auth guard names (item 402).
fn auth_guards(root: &Path) -> Vec<String> {
    config_named_keys(root, "auth", "guards")
}

/// Database connection names (item 404).
fn db_connections(root: &Path) -> Vec<String> {
    config_named_keys(root, "database", "connections")
}

/// Queue connection names (item 406).
fn queue_connections(root: &Path) -> Vec<String> {
    config_named_keys(root, "queue", "connections")
}

/// Broadcast connection names (item 407).
fn broadcast_connections(root: &Path) -> Vec<String> {
    config_named_keys(root, "broadcasting", "connections")
}

/// Custom artisan command names (item 408), from the go-to palette's
/// index sources.
fn artisan_commands(root: &Path) -> Vec<String> {
    crate::artisan_index::names(root)
}

/// Redis connection names (item 405). Laravel 12.x
/// `RedisManager::resolve` answers `Redis::connection($name)` from
/// two places: a top-level key of the `'redis'` region of
/// `config/database.php`, or a name inside its `'clusters'`
/// sub-region (`resolveCluster`). The region's metadata keys are not
/// connections: `'client'` (a scalar), `'options'` (an array of
/// defaults) and `'clusters'` itself are excluded, as is the
/// `'options'` metadata inside `'clusters'`.
fn redis_connections(root: &Path) -> Vec<String> {
    let p = root.join("config").join("database.php");
    let Ok(meta) = std::fs::metadata(&p) else {
        return Vec::new();
    };
    if !meta.is_file() || meta.len() > MAX_FILE {
        return Vec::new();
    }
    let Ok(text) = std::fs::read_to_string(&p) else {
        return Vec::new();
    };
    if text.contains("<<<") {
        // Same heredoc/nowdoc fail-closed guard as item 390d.
        return Vec::new();
    }
    let (masked, strings) = mask_comments(&text);
    let mut out = Vec::new();
    let Some(redis_open) =
        find_region(&masked, &text, &strings, "redis", 0, masked.len())
    else {
        return out;
    };
    let redis_close = region_end(&masked, &strings, redis_open);
    harvest_alias_keys(&masked, &text, &strings, redis_open, &mut out);
    if let Some(clusters_open) = find_region(
        &masked,
        &text,
        &strings,
        "clusters",
        redis_open,
        redis_close,
    ) {
        harvest_alias_keys(
            &masked, &text, &strings, clusters_open, &mut out,
        );
    }
    out.retain(|k| {
        !matches!(k.as_str(), "client" | "options" | "clusters")
    });
    out.sort();
    out.dedup();
    out.truncate(MAX_NAMES);
    out
}

/// The top-level `'name' =>` keys of the array opened at `open`
/// (items 393, 397). String spans are jumped whole, so a quote
/// inside a value cannot be misread as a key, and keys of nested
/// arrays sit at depth two and are skipped.
fn harvest_alias_keys(
    masked: &str,
    text: &str,
    strings: &[(usize, usize)],
    open: usize,
    out: &mut Vec<String>,
) {
    let b = masked.as_bytes();
    let mut i = open + 1;
    let mut depth = 1usize;
    while i < b.len() && depth > 0 {
        match b[i] {
            b'[' | b'(' | b'{' => {
                depth += 1;
                i += 1;
            }
            b']' | b')' | b'}' => {
                depth -= 1;
                i += 1;
            }
            q if q == b'\'' || q == b'"' => {
                let Ok(si) = strings.binary_search_by_key(&i, |s| s.0)
                else {
                    i += 1;
                    continue;
                };
                let (start, end) = strings[si];
                if depth == 1 {
                    let name = &text[start + 1..end];
                    if !name.is_empty()
                        && name.len() <= 60
                        && name.chars().all(|c| {
                            c.is_ascii_alphanumeric()
                                || matches!(c, '_' | '-' | '.')
                        })
                        && masked[end + 1..].trim_start().starts_with("=>")
                    {
                        out.push(name.to_string());
                    }
                }
                i = end + 1;
            }
            _ => i += 1,
        }
    }
}

/// Column-harvesting pass over one migration's text: comments are
/// blanked and string literals mapped first, so commented-out or
/// in-string `$table->` / `Schema::` text is ignored (item 390c).
/// Each `$table->` call is attributed to the enclosing
/// `Schema::create/table` closure - block (`function ... { ... }`)
/// or arrow (`fn (...) => ...`) - and another `Schema::` operation
/// (drop / dropIfExists / rename) ends the attribution. Text scan,
/// nothing is executed; a closure parameter named other than `$table`
/// is not seen (the Laravel convention, same as item 382), and files
/// containing heredoc/nowdoc (`<<<`) are skipped entirely - the scan
/// does not mask them, so it fails closed instead.
fn harvest_columns(text: &str, out: &mut Vec<(String, String)>) {
    let (masked, strings) = mask_comments(text);
    let in_string = |pos: usize| {
        let i = strings.partition_point(|&(_, end)| pos > end);
        i < strings.len() && strings[i].0 <= pos
    };
    // 0 = `$table->`, 1 = Schema::create/table (opens a scope),
    // 2 = another Schema:: operation (ends the attribution).
    let mut events: Vec<(usize, u8)> = Vec::new();
    for marker in ["Schema::create(", "Schema::table("] {
        events.extend(masked.match_indices(marker).map(|(i, _)| (i, 1)));
    }
    for marker in ["Schema::dropIfExists(", "Schema::drop(", "Schema::rename("]
    {
        events.extend(masked.match_indices(marker).map(|(i, _)| (i, 2)));
    }
    events.extend(masked.match_indices("$table->").map(|(i, _)| (i, 0)));
    events.sort_unstable();
    let mut table: Option<(String, usize)> = None; // (name, scope end)
    for (pos, kind) in events {
        if in_string(pos) {
            continue;
        }
        let rest = &masked[pos..];
        if kind == 2 {
            table = None;
            continue;
        }
        if kind == 1 {
            // Both markers end at their '('.
            let Some(open) = rest.find('(') else { continue };
            let Some(name) = quoted_word(rest[open + 1..].trim_start())
            else {
                continue;
            };
            table = closure_scope(&masked, pos + open + 1, &strings)
                .map(|end| (name.to_string(), end));
            continue;
        }
        let Some((table, end)) = table.as_ref() else { continue };
        if pos > *end {
            continue;
        }
        let after = &rest["$table->".len()..];
        let end_word = after
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(after.len());
        let method = &after[..end_word];
        let after = after[end_word..].trim_start();
        if !after.starts_with('(') {
            continue;
        }
        let name = quoted_word(after[1..].trim_start());
        let mut push = |col: &str| out.push((table.clone(), col.to_string()));
        match method {
            "id" => push(name.unwrap_or("id")),
            "timestamps" | "timestampsTz" | "nullableTimestamps" => {
                push("created_at");
                push("updated_at");
            }
            "softDeletes" | "softDeletesTz" => {
                push(name.unwrap_or("deleted_at"))
            }
            "rememberToken" => push("remember_token"),
            "morphs" | "nullableMorphs" | "uuidMorphs"
            | "nullableUuidMorphs" | "ulidMorphs" | "nullableUlidMorphs" => {
                if let Some(n) = name {
                    push(&format!("{n}_id"));
                    push(&format!("{n}_type"));
                }
            }
            m if BLUEPRINT_METHODS
                .iter()
                .any(|(b, d)| *b == m && *d == "Blueprint column type") =>
            {
                if let Some(n) = name {
                    push(n);
                }
            }
            _ => {}
        }
        if out.len() >= MAX_NAMES {
            return;
        }
    }
}

/// Comments blanked to spaces (offsets preserved), string literals
/// kept verbatim with their (start, end) byte spans returned, so
/// commented text can never produce events and events inside a string
/// literal can be skipped. `//`, `#` (but not the `#[` attribute
/// opener) and `/* ... */` are comments; `'` / `"` strings honor
/// backslash escapes. Heredoc/nowdoc is not special-cased.
fn mask_comments(text: &str) -> (String, Vec<(usize, usize)>) {
    let bytes = text.as_bytes();
    let mut out = bytes.to_vec();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    out[i] = b' ';
                    i += 1;
                }
            }
            b'#' if bytes.get(i + 1) != Some(&b'[') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    out[i] = b' ';
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                out[i] = b' ';
                out[i + 1] = b' ';
                i += 2;
                while i < bytes.len()
                    && !(bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/'))
                {
                    if bytes[i] != b'\n' {
                        out[i] = b' ';
                    }
                    i += 1;
                }
                if i < bytes.len() {
                    out[i] = b' ';
                    out[i + 1] = b' ';
                    i += 2;
                }
            }
            q @ (b'\'' | b'"') => {
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i] != q {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
                spans.push((start, i.min(bytes.len())));
                i = (i + 1).min(bytes.len());
            }
            _ => i += 1,
        }
    }
    (String::from_utf8(out).unwrap_or_default(), spans)
}

/// The end offset of the body a `Schema::create/table` call passes to
/// its closure: the matching `}` of the first block closure, or the
/// call's own closing `)` when no block appears (arrow functions).
/// String spans are skipped so braces and parens inside string
/// literals do not count. `args_start` is just past the call's `(`.
fn closure_scope(
    masked: &str,
    args_start: usize,
    strings: &[(usize, usize)],
) -> Option<usize> {
    let bytes = masked.as_bytes();
    let mut span = strings.partition_point(|&(start, _)| start < args_start);
    let mut i = args_start;
    let mut paren = 1usize; // the call's own '('
    while i < bytes.len() {
        if span < strings.len() && i == strings[span].0 {
            i = strings[span].1 + 1;
            span += 1;
            continue;
        }
        match bytes[i] {
            b'(' => paren += 1,
            b')' => {
                paren -= 1;
                if paren == 0 {
                    return Some(i);
                }
            }
            b'{' => {
                let mut depth = 1usize;
                let mut j = i + 1;
                while j < bytes.len() {
                    if span < strings.len() && j == strings[span].0 {
                        j = strings[span].1 + 1;
                        span += 1;
                        continue;
                    }
                    match bytes[j] {
                        b'{' => depth += 1,
                        b'}' => {
                            depth -= 1;
                            if depth == 0 {
                                return Some(j);
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }
                return None;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// A `'name'` or `"name"` at the start of `text`: nonempty, at most
/// 100 chars, ascii alphanumeric or underscore (table/column shape).
fn quoted_word(text: &str) -> Option<&str> {
    let q = text.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    let end = text[1..].find(q)?;
    let word = &text[1..1 + end];
    if !word.is_empty()
        && word.len() <= 100
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        Some(word)
    } else {
        None
    }
}

/// `->name('...')` values in routes/*.php (Laravel).
fn route_names(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(root.join("routes")) else { return out };
    let mut files: Vec<PathBuf> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "php"))
        .collect();
    files.sort();
    files.truncate(MAX_ROUTE_FILES);
    for f in files {
        let Ok(meta) = std::fs::metadata(&f) else { continue };
        if !meta.is_file() || meta.len() > MAX_FILE {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        for (i, _) in text.match_indices("->name(") {
            let rest = text[i + 7..].trim_start();
            let Some(q) = rest.chars().next().filter(|c| *c == '\'' || *c == '"')
            else {
                continue;
            };
            let Some(end) = rest[1..].find(q) else { continue };
            let name = &rest[1..1 + end];
            if !name.is_empty()
                && name.len() <= 100
                && !out.iter().any(|n: &String| n == name)
                && out.len() < MAX_NAMES
            {
                out.push(name.to_string());
            }
        }
    }
    out
}

/// Blade view names (`pages/home.blade.php` -> `pages.home`).
fn view_names(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, depth: usize, base: &Path, out: &mut Vec<String>) {
        if depth > MAX_DEPTH || out.len() >= MAX_VIEW_FILES {
            return;
        }
        if depth == 0
            && std::fs::symlink_metadata(dir).map_or(true, |m| m.file_type().is_symlink())
        {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.filter_map(Result::ok) {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let p = e.path();
            if ft.is_dir() {
                walk(&p, depth + 1, base, out);
            } else if ft.is_file() {
                let Some(name) = p.file_name().and_then(|n| n.to_str()) else { continue };
                let stem = name
                    .strip_suffix(".blade.php")
                    .or_else(|| name.strip_suffix(".php"));
                let Some(stem) = stem else { continue };
                let Ok(rel) = p.strip_prefix(base) else { continue };
                let mut dotted = rel
                    .parent()
                    .map(|pp| {
                        pp.to_string_lossy()
                            .replace(['/', '\\'], ".")
                    })
                    .filter(|s| !s.is_empty())
                    .map(|s| format!("{s}."))
                    .unwrap_or_default();
                dotted.push_str(stem);
                if out.len() < MAX_VIEW_FILES && !out.contains(&dotted) {
                    out.push(dotted);
                }
            }
        }
    }
    let base = root.join("resources/views");
    let mut out = Vec::new();
    walk(&base, 0, &base, &mut out);
    out.sort();
    out
}

/// `@yield('x')` / `@section('x')` / `@stack('x')` names declared in
/// `resources/views/layouts/**` (the conventional home of layouts; capped).
fn layout_names(root: &Path) -> (Vec<String>, Vec<String>) {
    const MARKERS: &[&str] = &["@yield(", "@section(", "@stack("];
    let base = root.join("resources/views/layouts");
    let mut files = Vec::new();
    walk_php_blade(&base, 0, &mut files);
    let mut sections = Vec::new();
    let mut stacks = Vec::new();
    for f in files.into_iter().take(100) {
        let Ok(md) = std::fs::symlink_metadata(&f) else { continue };
        if !md.is_file() || md.len() > 128 * 1024 {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        for marker in MARKERS {
            for (i, _) in text.match_indices(marker) {
                let rest = text[i + marker.len()..].trim_start();
                let Some(q) = rest.chars().next().filter(|c| *c == '\'' || *c == '"')
                else {
                    continue;
                };
                let Some(end) = rest[1..].find(q) else { continue };
                let name = &rest[1..1 + end];
                if name.is_empty() || name.len() > 60 {
                    continue;
                }
                let target = if *marker == "@stack(" {
                    &mut stacks
                } else {
                    &mut sections
                };
                if !target.iter().any(|n: &String| n == name) {
                    target.push(name.to_string());
                }
            }
        }
    }
    (sections, stacks)
}

/// Blade files under `dir` (depth bounded, symlinks skipped).
fn walk_php_blade(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || out.len() >= MAX_VIEW_FILES {
        return;
    }
    if depth == 0
        && std::fs::symlink_metadata(dir).map_or(true, |m| m.file_type().is_symlink())
    {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.filter_map(Result::ok) {
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let p = e.path();
        if ft.is_dir() {
            walk_php_blade(&p, depth + 1, out);
        } else if ft.is_file()
            && p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".blade.php"))
        {
            out.push(p);
        }
        if out.len() >= MAX_VIEW_FILES {
            return;
        }
    }
}

/// Config keys and env keys from the env/config palette index labels
/// (`"env  KEY  (.env)"`, `"config  app.name  (config/app.php)"`).
fn env_config_keys(root: &Path) -> (Vec<String>, Vec<String>) {
    let mut envs = Vec::new();
    let mut configs = Vec::new();
    for t in crate::env_index::discover(root) {
        if let Some(rest) = t.label.strip_prefix("env  ") {
            if let Some((key, _)) = rest.rsplit_once("  (") {
                envs.push(key.to_string());
            }
        } else if let Some(rest) = t.label.strip_prefix("config  ") {
            if let Some((key, _)) = rest.rsplit_once("  (") {
                configs.push(key.to_string());
            }
        }
    }
    (envs, configs)
}

/// Translation keys from the Go to Translation index labels
/// (`"lang  auth.failed  (resources/lang/en/auth.php)"`).
fn lang_keys(root: &Path) -> Vec<String> {
    crate::lang_index::discover(root)
        .into_iter()
        .filter_map(|t| {
            let rest = t.label.strip_prefix("lang  ")?;
            rest.rsplit_once("  (").map(|(k, _)| k.to_string())
        })
        .collect()
}

/// File-cap and name-cap for the Alpine store scan.
const MAX_STORE_FILES: usize = 400;
const MAX_STORES: usize = 200;

/// Collect JS/TS/PHP files that may register Alpine stores.
fn walk_store_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 8 || out.len() >= MAX_STORE_FILES {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        if out.len() >= MAX_STORE_FILES {
            return;
        }
        let path = e.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        if meta.is_symlink() {
            continue;
        }
        if meta.is_dir() {
            walk_store_files(&path, depth + 1, out);
        } else if meta.is_file()
            && meta.len() <= 1 << 20
            && path
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| matches!(x, "js" | "ts" | "php"))
        {
            out.push(path);
        }
    }
}

/// Store names from `Alpine.store('name', ...)` calls in `text`.
fn stores_in(text: &str, out: &mut Vec<String>) {
    let mut hay = text;
    while let Some(i) = hay.find("Alpine.store(") {
        hay = &hay[i + "Alpine.store(".len()..];
        let t = hay.trim_start();
        let Some(q) = t.chars().next() else { break };
        if q != '\'' && q != '"' {
            continue;
        }
        let body = &t[q.len_utf8()..];
        let Some(end) = body.find(q) else { break };
        let name = &body[..end];
        if !name.is_empty() && name.len() <= 80 && !name.contains('\\') {
            out.push(name.to_string());
        }
        hay = &body[end..];
    }
}

/// Alpine store names registered with `Alpine.store('name', ...)` in the
/// project's JS/TS files and inline `<script>` in Blade views. Textual
/// scan; nothing is executed.
fn store_names(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    for base in ["resources/js", "resources/ts", "resources/views", "public/js"] {
        walk_store_files(&root.join(base), 0, &mut files);
    }
    let mut names = Vec::new();
    for f in files {
        if names.len() >= MAX_STORES {
            break;
        }
        if let Ok(text) = std::fs::read_to_string(&f) {
            stores_in(&text, &mut names);
        }
    }
    names.sort();
    names.dedup();
    names.truncate(MAX_STORES);
    names
}

/// Component names from `Alpine.data('name', ...)` calls in `text`.
fn data_names_in(text: &str, out: &mut Vec<String>) {
    let mut hay = text;
    while let Some(i) = hay.find("Alpine.data(") {
        hay = &hay[i + "Alpine.data(".len()..];
        let t = hay.trim_start();
        let Some(q) = t.chars().next() else { break };
        if q != '\'' && q != '"' {
            continue;
        }
        let body = &t[q.len_utf8()..];
        let Some(end) = body.find(q) else { break };
        let name = &body[..end];
        if !name.is_empty() && name.len() <= 80 && !name.contains('\\') {
            out.push(name.to_string());
        }
        hay = &body[end..];
    }
}

/// Alpine component names registered with `Alpine.data('name', ...)` in
/// the project's JS/TS files and inline `<script>` in Blade views.
/// Textual scan, like the store scan; nothing is executed.
fn data_names(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    for base in ["resources/js", "resources/ts", "resources/views", "public/js"] {
        walk_store_files(&root.join(base), 0, &mut files);
    }
    let mut names = Vec::new();
    for f in files {
        if names.len() >= MAX_STORES {
            break;
        }
        if let Ok(text) = std::fs::read_to_string(&f) {
            data_names_in(&text, &mut names);
        }
    }
    names.sort();
    names.dedup();
    names.truncate(MAX_STORES);
    names
}

/// Keys of the object returned by the `Alpine.data('name', ...)`
/// factory registered in the project's JS/TS files or inline Blade
/// scripts (item 370). Only the `=> ({ ... })` factory shape is read;
/// a block body or a shared function reference is not resolved. The
/// first registration found wins.
fn data_component_keys(root: &Path, name: &str) -> Vec<String> {
    let mut files = Vec::new();
    for base in ["resources/js", "resources/ts", "resources/views", "public/js"] {
        walk_store_files(&root.join(base), 0, &mut files);
    }
    for f in files {
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        if let Some(keys) = data_factory_keys(&text, name) {
            return keys;
        }
    }
    Vec::new()
}

/// The returned object's keys when `text` registers `name` with
/// `Alpine.data('name', () => ({ ... }))`.
fn data_factory_keys(text: &str, name: &str) -> Option<Vec<String>> {
    let mut hay = text;
    while let Some(i) = hay.find("Alpine.data(") {
        hay = &hay[i + "Alpine.data(".len()..];
        let t = hay.trim_start();
        let Some(q) = t.chars().next() else { break };
        if q != '\'' && q != '"' {
            continue;
        }
        let body = &t[q.len_utf8()..];
        let Some(end) = body.find(q) else { break };
        let found = &body[..end];
        let after_name = &body[end + q.len_utf8()..];
        hay = after_name;
        if found != name {
            continue;
        }
        let arrow = after_name.find("=>")?;
        let expr = after_name[arrow + 2..].trim_start();
        let object = expr
            .strip_prefix('(')
            .map(str::trim_start)
            .unwrap_or(expr);
        return Some(parse_data_keys(object));
    }
    None
}

/// `x-ref="name"` values in `text` (Alpine refs, read via `$refs.name`).
fn refs_in(text: &str, out: &mut Vec<String>) {
    let mut hay = text;
    while let Some(i) = hay.find("x-ref=") {
        hay = &hay[i + "x-ref=".len()..];
        let Some(q) = hay.chars().next() else { break };
        if q != '\'' && q != '"' {
            continue;
        }
        let body = &hay[q.len_utf8()..];
        let Some(end) = body.find(q) else { break };
        let name = &body[..end];
        if !name.is_empty()
            && name.len() <= 80
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        {
            out.push(name.to_string());
        }
        hay = &body[end..];
    }
}

/// Alpine `x-ref` names in the current file, for `$refs.` completion.
/// Reads the saved file, like the Livewire property list.
fn ref_names(file: Option<&Path>) -> Vec<String> {
    let mut names = Vec::new();
    let Some(f) = file else { return names };
    let readable = std::fs::symlink_metadata(f)
        .ok()
        .is_some_and(|m| m.is_file() && m.len() <= 1 << 20);
    if readable {
        if let Ok(text) = std::fs::read_to_string(f) {
            refs_in(&text, &mut names);
        }
    }
    names.sort();
    names.dedup();
    names.truncate(100);
    names
}

/// `$js('name', fn)` registrations in `text` (Livewire 3 `@script`).
fn js_names_in(text: &str, out: &mut Vec<String>) {
    let mut hay = text;
    while let Some(i) = hay.find("$js(") {
        hay = &hay[i + "$js(".len()..];
        let t = hay.trim_start();
        let Some(q) = t.chars().next() else { break };
        if q != '\'' && q != '"' {
            continue;
        }
        let body = &t[q.len_utf8()..];
        let Some(end) = body.find(q) else { break };
        let name = &body[..end];
        if !name.is_empty()
            && name.len() <= 80
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        {
            out.push(name.to_string());
        }
        hay = &body[end..];
    }
}

/// `$js` function names registered in the current file (read from the
/// saved file, like the x-ref list), for `$wire.$js.` completion.
fn js_names(file: Option<&Path>) -> Vec<String> {
    let mut names = Vec::new();
    let Some(f) = file else { return names };
    let readable = std::fs::symlink_metadata(f)
        .ok()
        .is_some_and(|m| m.is_file() && m.len() <= 1 << 20);
    if readable {
        if let Ok(text) = std::fs::read_to_string(f) {
            js_names_in(&text, &mut names);
        }
    }
    names.sort();
    names.dedup();
    names.truncate(100);
    names
}

/// Caps for the Livewire event scan.
const MAX_EVENT_FILES: usize = 400;
const MAX_EVENTS: usize = 300;

/// Collect `.php` files (plain and `.blade.php`) that may dispatch or
/// listen for Livewire events.
fn walk_event_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 8 || out.len() >= MAX_EVENT_FILES {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        if out.len() >= MAX_EVENT_FILES {
            return;
        }
        let path = e.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        if meta.is_symlink() {
            continue;
        }
        if meta.is_dir() {
            walk_event_files(&path, depth + 1, out);
        } else if meta.is_file()
            && meta.len() <= 1 << 20
            && path
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x == "php")
        {
            out.push(path);
        }
    }
}

/// Livewire event names dispatched or listened for in `text`: the first
/// quoted argument of `dispatch(`, `dispatchBrowserEvent(`, `emit(`,
/// `emitTo(` and `#[On('...')]`.
fn events_in(text: &str, out: &mut Vec<String>) {
    for pat in ["dispatch(", "dispatchBrowserEvent(", "emitTo(", "emit(", "#[On("] {
        let mut hay = text;
        while let Some(i) = hay.find(pat) {
            // Not the tail of a longer identifier (`xdispatch(`).
            if pat != "#[On(" && i > 0 {
                let prev = hay.as_bytes()[i - 1];
                if prev.is_ascii_alphanumeric() || prev == b'_' {
                    hay = &hay[i + pat.len()..];
                    continue;
                }
            }
            hay = &hay[i + pat.len()..];
            let t = hay.trim_start();
            let Some(q) = t.chars().next() else { break };
            if q != '\'' && q != '"' {
                continue;
            }
            let body = &t[q.len_utf8()..];
            let Some(end) = body.find(q) else { break };
            let name = &body[..end];
            if !name.is_empty()
                && name.len() <= 100
                && name.bytes().all(|b| {
                    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':')
                })
            {
                out.push(name.to_string());
            }
            hay = &body[end..];
        }
    }
}

/// Livewire event names from `app/` and `resources/views/`: dispatches
/// and `#[On]` listeners. Textual scan; nothing is executed.
fn event_names(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    walk_event_files(&root.join("app"), 0, &mut files);
    walk_event_files(&root.join("resources/views"), 0, &mut files);
    let mut names = Vec::new();
    for f in files {
        if names.len() >= MAX_EVENTS {
            break;
        }
        if let Ok(text) = std::fs::read_to_string(&f) {
            events_in(&text, &mut names);
        }
    }
    names.sort();
    names.dedup();
    names.truncate(MAX_EVENTS);
    names
}

fn lists(root: &Path) -> Lists {
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    {
        let guard = cache.lock().unwrap();
        if let Some(l) = guard.as_ref() {
            if l.root == root && l.at.elapsed() < TTL {
                return l.clone();
            }
        }
    }
    let routes = route_names(root);
    let views = view_names(root);
    let (envs, configs) = env_config_keys(root);
    let (sections, stacks) = layout_names(root);
    let langs = lang_keys(root);
    let components = component_names(root);
    let assets = asset_paths(root);
    let gates = ability_names(root);
    let tables = table_names(root);
    let columns = table_columns(root);
    let middleware_aliases = middleware_aliases(root);
    let disks = filesystem_disks(root);
    let vite_entries = vite_entries(root);
    let log_channels = log_channels(root);
    let cache_stores = cache_stores(root);
    let mail_mailers = mail_mailers(root);
    let auth_guards = auth_guards(root);
    let db_connections = db_connections(root);
    let redis_connections = redis_connections(root);
    let queue_connections = queue_connections(root);
    let broadcast_connections = broadcast_connections(root);
    let artisan_commands = artisan_commands(root);
    let fresh = Lists {
        root: root.to_path_buf(),
        at: Instant::now(),
        routes,
        configs,
        envs,
        views,
        sections,
        stacks,
        langs,
        components,
        assets,
        gates,
        tables,
        columns,
        middleware_aliases,
        disks,
        vite_entries,
        log_channels,
        cache_stores,
        mail_mailers,
        auth_guards,
        db_connections,
        redis_connections,
        queue_connections,
        broadcast_connections,
        artisan_commands,
    };
    *cache.lock().unwrap() = Some(fresh.clone());
    fresh
}

static PROP_CACHE: OnceLock<
    Mutex<Option<(PathBuf, Option<SystemTime>, Vec<String>)>>,
> = OnceLock::new();

fn prop_item(n: String) -> CompletionItem {
    CompletionItem {
        label: n,
        kind: Some(CompletionItemKind::PROPERTY),
        detail: Some("Livewire property".to_string()),
        ..Default::default()
    }
}

/// Public properties of the Livewire component paired with `file`,
/// re-parsed only when the class file changes.
fn livewire_props(root: &Path, file: Option<&Path>) -> Vec<CompletionItem> {
    let Some(file) = file else { return Vec::new() };
    let Some(class) = crate::livewire_handlers::class_for_view(root, file)
    else {
        return Vec::new();
    };
    let Ok(md) = std::fs::metadata(&class) else { return Vec::new() };
    let mtime = md.modified().ok();
    let cache = PROP_CACHE.get_or_init(|| Mutex::new(None));
    {
        let guard = cache.lock().unwrap();
        if let Some((p, m, props)) = guard.as_ref() {
            if *p == class && *m == mtime {
                return props
                    .iter()
                    .take(MAX_ITEMS)
                    .cloned()
                    .map(prop_item)
                    .collect();
            }
        }
    }
    let Ok(text) = std::fs::read_to_string(&class) else { return Vec::new() };
    let props = crate::livewire_handlers::public_props(&text);
    *cache.lock().unwrap() = Some((class, mtime, props.clone()));
    props.into_iter().take(MAX_ITEMS).map(prop_item).collect()
}

/// Variables a Blade view defines above the cursor: `$name = ...`
/// assignments, `@foreach` / `@forelse` loop variables, `@props([...])`
/// entries and `@inject('name', ...)` names. The compiled view is one PHP
/// function scope, so anything assigned above the cursor is in scope.
/// Static text scan only; at most the last 256 KiB before the cursor.
pub fn blade_variables(
    text_before: &str,
    line_before: &str,
) -> Vec<CompletionItem> {
    // Only when the line ends inside a `$<partial>` token.
    let bytes = line_before.as_bytes();
    let mut i = bytes.len();
    while i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_')
    {
        i -= 1;
    }
    if i == 0 || bytes[i - 1] != b'$' {
        return Vec::new();
    }
    if i >= 2 {
        let p = bytes[i - 2];
        if p.is_ascii_alphanumeric() || p == b'_' || p == b'$' {
            return Vec::new();
        }
    }

    // At most the last 256 KiB.
    let mut start = text_before.len().saturating_sub(262_144);
    while !text_before.is_char_boundary(start) {
        start += 1;
    }
    // Strip Blade comments so they cannot define variables.
    let mut cleaned = String::with_capacity(text_before.len() - start);
    let mut rest = &text_before[start..];
    while let Some(o) = rest.find("{{--") {
        cleaned.push_str(&rest[..o]);
        rest = match rest[o..].find("--}}") {
            Some(c) => &rest[o + c + 4..],
            None => "",
        };
    }
    cleaned.push_str(rest);
    let text = cleaned.as_str();

    let mut vars: Vec<String> = Vec::new();
    let mut push = |name: &str, vars: &mut Vec<String>| {
        if !name.is_empty()
            && name.len() <= 60
            && !vars.iter().any(|v| v == name)
        {
            vars.push(name.to_string());
        }
    };

    // `$name` occurrences that are definitions: followed by a single `=`,
    // or preceded by `as` (loop variables) or `=>` (foreach key => value).
    let tb = text.as_bytes();
    let mut at = 0;
    while at < tb.len() {
        if tb[at] != b'$' {
            at += 1;
            continue;
        }
        let ns = at + 1;
        let mut ne = ns;
        while ne < tb.len()
            && (tb[ne].is_ascii_alphanumeric() || tb[ne] == b'_')
        {
            ne += 1;
        }
        let name = &text[ns..ne];
        at = ne.max(ns);
        if name.is_empty() || name.as_bytes()[0].is_ascii_digit() {
            continue;
        }
        let mut fw = ne;
        while fw < tb.len() && (tb[fw] == b' ' || tb[fw] == b'\t') {
            fw += 1;
        }
        let assigns = fw < tb.len()
            && tb[fw] == b'='
            && tb.get(fw + 1) != Some(&b'=')
            && tb.get(fw + 1) != Some(&b'>');
        // Look back from `$` over spaces, then at the last three chars
        // (char-aware: the bytes before may be multibyte).
        let mut bw = ns - 1;
        while bw > 0 && (tb[bw - 1] == b' ' || tb[bw - 1] == b'\t') {
            bw -= 1;
        }
        let tail: Vec<char> = text[..bw].chars().rev().take(3).collect();
        let after_as = tail.len() >= 2
            && tail[0].eq_ignore_ascii_case(&'s')
            && tail[1].eq_ignore_ascii_case(&'a')
            && tail
                .get(2)
                .map_or(true, |c| !(c.is_ascii_alphanumeric() || *c == '_'));
        let after_fat =
            tail.len() >= 2 && tail[0] == '>' && tail[1] == '=';
        if assigns || after_as || after_fat {
            push(name, &mut vars);
        }
    }

    // @props(['disabled' => ..., 'type']) keys and bare entries. A
    // quoted string is a prop name only when followed by `=>` (key) or
    // by `,` / `]` / `)` (bare entry) — and never right after `=>`
    // (that is a default value, not a name).
    let mut p = 0;
    while let Some(o) = text[p..].find("@props") {
        let o = p + o;
        let after = &text[o + 6..];
        let Some(op) = after.find('(') else { break };
        let Some(cp) = after[op..].find(')') else { break };
        let inner = &after[op + 1..op + cp];
        let ib = inner.as_bytes();
        let mut q = 0;
        while q < ib.len() {
            let quote = ib[q];
            if quote != b'\'' && quote != b'"' {
                q += 1;
                continue;
            }
            let Some(e) = inner[q + 1..].find(quote as char).map(|e| q + 1 + e)
            else {
                break;
            };
            // Skip whitespace after the closing quote.
            let mut n = e + 1;
            while n < ib.len() && (ib[n] == b' ' || ib[n] == b'\t') {
                n += 1;
            }
            // Non-whitespace before the opening quote.
            let mut b = q;
            while b > 0 && (ib[b - 1] == b' ' || ib[b - 1] == b'\t') {
                b -= 1;
            }
            let is_value = b >= 2 && ib[b - 2] == b'=' && ib[b - 1] == b'>';
            let is_key = ib.get(n) == Some(&b'=')
                && ib.get(n + 1) == Some(&b'>');
            let is_bare = matches!(ib.get(n), Some(b',') | Some(b']') | Some(b')'));
            if !is_value && (is_key || is_bare) {
                push(&inner[q + 1..e], &mut vars);
            }
            q = e + 1;
        }
        p = o + 6 + op + cp;
    }

    // @inject('name', 'Service') first argument.
    let mut p = 0;
    while let Some(o) = text[p..].find("@inject") {
        let o = p + o;
        let after = &text[o + 7..];
        let Some(q1) = after.find(['\'', '"']) else { break };
        let quote = after.as_bytes()[q1];
        let Some(q2) = after[q1 + 1..].find(quote as char).map(|e| q1 + 1 + e)
        else {
            break;
        };
        push(&after[q1 + 1..q2], &mut vars);
        p = o + 7 + q2;
    }

    let builtins = builtin_blade_variables(text, &vars);
    vars.into_iter()
        .take(100)
        .map(|n| CompletionItem {
            label: n,
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some("Blade variable".to_string()),
            ..Default::default()
        })
        .chain(builtins)
        .collect()
}

/// Blade's own variables that are in scope without any assignment the
/// scanner could see: `$errors` is shared with every view, `$loop` exists
/// inside an unclosed `@foreach`/`@forelse`, `$message` inside `@error`,
/// and `$slot` / `$attributes` in a component view (one with `@props`).
fn builtin_blade_variables(
    text: &str,
    vars: &[String],
) -> Vec<CompletionItem> {
    let mut out: Vec<CompletionItem> = Vec::new();
    let mut push = |name: &str, detail: &str| {
        if !vars.iter().any(|v| v == name)
            && !out.iter().any(|i| i.label == name)
        {
            out.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some(detail.to_string()),
                ..Default::default()
            });
        }
    };
    push("errors", "Blade: shared ViewErrorBag of validation errors");
    let open_loops = (text.matches("@foreach").count()
        + text.matches("@forelse").count())
    .saturating_sub(
        text.matches("@endforeach").count() + text.matches("@endforelse").count(),
    );
    if open_loops > 0 {
        push(
            "loop",
            "Blade: current iteration ($loop->index, ->iteration, ->first, ->last, ->remaining, ->count, ->depth)",
        );
    }
    let open_error = text
        .matches("@error")
        .count()
        .saturating_sub(text.matches("@enderror").count());
    if open_error > 0 {
        push("message", "Blade: the field's validation message inside @error");
    }
    if text.contains("@props") {
        push("slot", "Blade component: default slot content");
        push(
            "attributes",
            "Blade component: attribute bag ($attributes->merge([...]))",
        );
    }
    out
}

static METHOD_CACHE: std::sync::OnceLock<
    Mutex<Option<(PathBuf, Option<SystemTime>, Vec<String>)>>,
> = std::sync::OnceLock::new();

fn method_item(n: String) -> CompletionItem {
    CompletionItem {
        label: n,
        kind: Some(CompletionItemKind::METHOD),
        detail: Some("Livewire method".to_string()),
        ..Default::default()
    }
}

/// Public methods of the Livewire component paired with `file`,
/// re-parsed only when the class file changes.
fn livewire_methods(root: &Path, file: Option<&Path>) -> Vec<CompletionItem> {
    let Some(file) = file else { return Vec::new() };
    let Some(class) = crate::livewire_handlers::class_for_view(root, file)
    else {
        return Vec::new();
    };
    let Ok(md) = std::fs::metadata(&class) else { return Vec::new() };
    let mtime = md.modified().ok();
    let cache = METHOD_CACHE.get_or_init(|| Mutex::new(None));
    {
        let guard = cache.lock().unwrap();
        if let Some((p, m, methods)) = guard.as_ref() {
            if *p == class && *m == mtime {
                return methods
                    .iter()
                    .take(MAX_ITEMS)
                    .cloned()
                    .map(method_item)
                    .collect();
            }
        }
    }
    let Ok(text) = std::fs::read_to_string(&class) else { return Vec::new() };
    let methods = crate::livewire_handlers::public_methods(&text);
    *cache.lock().unwrap() = Some((class, mtime, methods.clone()));
    methods.into_iter().take(MAX_ITEMS).map(method_item).collect()
}

/// Completion items for the project context at the end of `line_before`.
/// `file` is the edited file; only Livewire completion uses it.
/// Gate abilities: public policy methods under `app/Policies`
/// (reusing the policy index parser) plus `Gate::define('name')` names
/// from `app/Providers`. Static text scan, bounded like the index.
fn ability_names(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        if depth > 3 || out.len() >= 500 {
            return;
        }
        if depth == 0
            && std::fs::symlink_metadata(dir)
                .map_or(true, |m| m.file_type().is_symlink())
        {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.filter_map(Result::ok) {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let p = e.path();
            if ft.is_dir() {
                walk(&p, depth + 1, out);
            } else if ft.is_file()
                && p.extension().is_some_and(|x| x == "php")
            {
                out.push(p);
            }
            if out.len() >= 500 {
                return;
            }
        }
    }
    fn read(p: &Path) -> Option<String> {
        let m = std::fs::metadata(p).ok()?;
        if m.len() > (1 << 20) {
            return None;
        }
        std::fs::read_to_string(p).ok()
    }
    let mut names: Vec<String> = Vec::new();
    let mut push = |n: &str, names: &mut Vec<String>| {
        if !n.is_empty() && n.len() <= 100 && !names.iter().any(|x| x == n) {
            names.push(n.to_string());
        }
    };
    let mut files = Vec::new();
    walk(&root.join("app/Policies"), 0, &mut files);
    for f in files.drain(..) {
        let Some(text) = read(&f) else { continue };
        let Some(p) = crate::policy_index::parse_policy(&text) else {
            continue;
        };
        for (a, _, _) in p.abilities {
            push(&a, &mut names);
        }
    }
    walk(&root.join("app/Providers"), 0, &mut files);
    for f in files {
        let Some(text) = read(&f) else { continue };
        for (n, _) in crate::policy_index::parse_gates(&text) {
            push(&n, &mut names);
        }
    }
    names.truncate(2000);
    names
}

/// Files under `public/` as `asset()`-ready paths (`css/app.css`).
/// Symlinks (common for `public/storage`) and hidden directories are
/// skipped; the walk is bounded like the other static scans.
/// All non-hidden files under `dir`, as `/`-joined paths relative
/// to `base`; symlinks skipped, capped at MAX_VIEW_FILES.
fn walk_files(dir: &Path, depth: usize, base: &Path, out: &mut Vec<String>) {
    if depth > MAX_DEPTH || out.len() >= MAX_VIEW_FILES {
        return;
    }
    if depth == 0
        && std::fs::symlink_metadata(dir)
            .map_or(true, |m| m.file_type().is_symlink())
    {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.filter_map(Result::ok) {
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let p = e.path();
        let hidden =
            e.file_name().to_str().is_some_and(|n| n.starts_with('.'));
        if hidden {
            continue;
        }
        if ft.is_dir() {
            walk_files(&p, depth + 1, base, out);
        } else if ft.is_file() {
            let Ok(rel) = p.strip_prefix(base) else { continue };
            let path = rel.to_string_lossy().replace('\\', "/");
            if out.len() < MAX_VIEW_FILES && !out.contains(&path) {
                out.push(path);
            }
        }
    }
}

fn asset_paths(root: &Path) -> Vec<String> {
    let base = root.join("public");
    let mut out = Vec::new();
    walk_files(&base, 0, &base, &mut out);
    out
}

/// Vite entry points (item 398): files under `resources/js` and
/// `resources/css`, as root-relative paths (`resources/js/app.js`) -
/// the strings `Vite::asset('...')` and `@vite(...)` take. Sorted,
/// deduped; the public-dir assets of `asset()` are a different set.
fn vite_entries(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for sub in ["resources/js", "resources/css"] {
        walk_files(&root.join(sub), 0, root, &mut out);
    }
    out.sort();
    out.dedup();
    out
}

/// Blade component tag names from `resources/views/components`:
/// `forms/input.blade.php` -> `forms.input`, and an `index.blade.php`
/// names its directory, matching the component resolver in
/// `props_draft.rs`. Symlinks are not followed; bounded walk.
fn component_names(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, depth: usize, base: &Path, out: &mut Vec<String>) {
        if depth > MAX_DEPTH || out.len() >= MAX_VIEW_FILES {
            return;
        }
        if depth == 0
            && std::fs::symlink_metadata(dir)
                .map_or(true, |m| m.file_type().is_symlink())
        {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.filter_map(Result::ok) {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let p = e.path();
            if ft.is_dir() {
                walk(&p, depth + 1, base, out);
            } else if ft.is_file() {
                let Some(name) = p.file_name().and_then(|n| n.to_str())
                else {
                    continue;
                };
                let Some(stem) = name.strip_suffix(".blade.php") else {
                    continue;
                };
                let Ok(rel) = p.strip_prefix(base) else { continue };
                let parent = rel
                    .parent()
                    .map(|pp| pp.to_string_lossy().replace(['/', '\\'], "."))
                    .filter(|s| !s.is_empty());
                let dotted = if stem == "index" {
                    match parent {
                        Some(d) => d,
                        None => continue,
                    }
                } else {
                    match parent {
                        Some(d) => format!("{d}.{stem}"),
                        None => stem.to_string(),
                    }
                };
                if out.len() < MAX_VIEW_FILES && !out.contains(&dotted) {
                    out.push(dotted);
                }
            }
        }
    }
    let base = root.join("resources").join("views").join("components");
    let mut out = Vec::new();
    walk(&base, 0, &base, &mut out);
    out
}

/// `<x-...>` tag completion: only inside a `.blade.php` view.
fn component_tags(root: &Path, file: Option<&Path>) -> Vec<CompletionItem> {
    let is_blade = file
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".blade.php"));
    if !is_blade {
        return Vec::new();
    }
    lists(root)
        .components
        .into_iter()
        .take(MAX_ITEMS)
        .map(|n| CompletionItem {
            label: n,
            kind: Some(CompletionItemKind::CLASS),
            detail: Some("Blade component".to_string()),
            ..Default::default()
        })
        .collect()
}

/// kebab-case of a StudlyCaps class name: `UserProfile` -> `user-profile`.
fn kebab(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Livewire component names: view files under `resources/views/livewire`
/// and classes under `app/Livewire` / `app/Http/Livewire`, dotted and
/// kebab-cased (`Admin/UserProfile.php` -> `admin.user-profile`). Symlinks
/// are not followed; bounded walk.
fn livewire_names(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, depth: usize, base: &Path, ext: &str, out: &mut Vec<String>) {
        if depth > MAX_DEPTH || out.len() >= MAX_VIEW_FILES {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.filter_map(Result::ok) {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let p = e.path();
            if ft.is_dir() {
                walk(&p, depth + 1, base, ext, out);
            } else if ft.is_file() {
                let Some(fname) = p.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                let Some(stem) = fname.strip_suffix(ext) else { continue };
                let Ok(rel) = p.strip_prefix(base) else { continue };
                let mut parts: Vec<String> = rel
                    .parent()
                    .map(|pp| {
                        pp.components()
                            .map(|c| c.as_os_str().to_string_lossy().to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                parts.push(stem.to_string());
                let name = parts
                    .iter()
                    .map(|x| kebab(x))
                    .collect::<Vec<_>>()
                    .join(".");
                if !out.contains(&name) && out.len() < MAX_VIEW_FILES {
                    out.push(name);
                }
            }
        }
    }
    let mut out = Vec::new();
    let views = root.join("resources").join("views").join("livewire");
    walk(&views, 0, &views, ".blade.php", &mut out);
    for base in [root.join("app").join("Livewire"), root.join("app").join("Http").join("Livewire")] {
        walk(&base, 0, &base, ".php", &mut out);
    }
    out.sort();
    out
}

/// `<livewire:...>` tag completion: only inside a `.blade.php` view.
fn livewire_tags(root: &Path, file: Option<&Path>) -> Vec<CompletionItem> {
    let is_blade = file
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".blade.php"));
    if !is_blade {
        return Vec::new();
    }
    livewire_names(root)
        .into_iter()
        .take(MAX_ITEMS)
        .map(|n| CompletionItem {
            label: n,
            kind: Some(CompletionItemKind::CLASS),
            detail: Some("Livewire component".to_string()),
            ..Default::default()
        })
        .collect()
}

/// Does the completion context at the end of `line_before_cursor` offer
/// dotted names (routes, config keys, views, sections, stacks, translation
/// keys, Gate abilities, middleware aliases)? Used to extend the
/// completion word start over
/// `.`-joined segments so accepting replaces the whole name.
pub fn dotted_name_context(line_before_cursor: &str) -> bool {
    matches!(
        context(line_before_cursor),
        Some(
            Kind::Route
                | Kind::Config
                | Kind::View
                | Kind::Section
                | Kind::Stack
                | Kind::Lang
                | Kind::Gate
                | Kind::Lang
                | Kind::Gate
                | Kind::LivewireName
                | Kind::LivewireEvent
                | Kind::MiddlewareAlias
                | Kind::StorageDisk
                | Kind::LogChannel
                | Kind::CacheStore
                | Kind::MailMailer
                | Kind::AuthGuard
                | Kind::DbConnection
                | Kind::RedisConnection
                | Kind::QueueConnection
                | Kind::BroadcastConnection
                | Kind::ArtisanCommand
        )
    )
}

/// Bytes to extend a completion word start backwards over `.`- and
/// `-`-joined segments of a quoted name (`admin.us`,
/// `messages.welcome-back`). Stops at anything else, such as the opening
/// quote. Returns 0 when `before` does not end in a joined segment.
pub fn dotted_word_len(before: &str) -> usize {
    let mut rest = before;
    let mut total = 0;
    while let Some(sep) =
        rest.chars().next_back().filter(|c| matches!(c, '.' | '-'))
    {
        let r = &rest[..rest.len() - sep.len_utf8()];
        let word: usize = r
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .map(char::len_utf8)
            .sum();
        if word == 0 {
            break;
        }
        total += sep.len_utf8() + word;
        rest = &r[..r.len() - word];
    }
    total
}

/// DOM events Alpine listens to with `x-on:` / `@`, for the modifier
/// context check. Not every DOM event, only the ones worth completing.
const DOM_EVENTS: &[&str] = &[
    "click", "dblclick", "submit", "keydown", "keyup", "change", "input",
    "blur", "focus", "mouseenter", "mouseleave", "mousedown", "mouseup",
    "scroll", "paste", "resize",
];

/// `x-on:<event>.` / `@<event>.` modifier completion: Alpine v3 modifiers.
pub fn alpine_modifiers_for(line: &str) -> Vec<CompletionItem> {
    let at = if let Some(at) = line.rfind("x-on:") {
        at + 5
    } else if let Some(at) = line.rfind('@') {
        at + 1
    } else {
        return Vec::new();
    };
    let after = &line[at..];
    let attr_len = after
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
        .unwrap_or(after.len());
    let attr = &after[..attr_len];
    if attr_len != after.len() || !attr.contains('.') {
        return Vec::new();
    }
    let event = attr.split('.').next().unwrap_or("");
    if !DOM_EVENTS.contains(&event) {
        return Vec::new();
    }
    let mut names: Vec<&str> = vec![
        "prevent", "stop", "once", "self", "capture", "passive", "window",
        "document", "debounce.250ms", "throttle.250ms",
    ];
    if matches!(event, "click" | "dblclick" | "mousedown" | "mouseup") {
        names.push("outside");
    }
    if matches!(event, "keydown" | "keyup") {
        names.extend([
            "enter", "escape", "space", "tab", "backspace", "arrow-up",
            "arrow-down", "arrow-left", "arrow-right", "shift", "ctrl", "alt",
            "meta",
        ]);
    }
    names
        .into_iter()
        .map(|n| CompletionItem {
            label: n.to_string(),
            kind: Some(CompletionItemKind::ENUM_MEMBER),
            detail: Some("Alpine modifier".to_string()),
            ..Default::default()
        })
        .collect()
}

/// `wire:<event>.` modifier completion: Livewire v3 modifier tables for
/// the events that take them; other events have none.
pub fn wire_modifiers_for(line: &str) -> Vec<CompletionItem> {
    let Some(at) = line.rfind("wire:") else { return Vec::new() };
    let after = &line[at + 5..];
    let attr_len = after
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
        .unwrap_or(after.len());
    let attr = &after[..attr_len];
    if !attr.contains('.') || attr_len != after.len() {
        return Vec::new();
    }
    let event = attr.split('.').next().unwrap_or("");
    let names: &[&str] = match event {
        "model" => &[
            "live", "blur", "change", "lazy", "fill", "debounce.150ms",
            "throttle.500ms",
        ],
        "keydown" | "keyup" => &[
            "enter", "escape", "space", "tab", "backspace", "arrow-up",
            "arrow-down", "arrow-left", "arrow-right", "shift", "ctrl", "alt",
            "meta",
        ],
        "click" | "dblclick" | "submit" | "mouseenter" | "mouseleave" => {
            &["prevent", "stop"]
        }
        "loading" => &[
            "class", "class.remove", "attr", "remove", "delay",
            "delay.shortest", "delay.longer", "delay.longest",
        ],
        "dirty" | "offline" => &["class", "class.remove", "attr", "remove"],
        "navigate" => &["hover"],
        "ignore" => &["self"],
        _ => &[],
    };
    names
        .iter()
        .map(|n| CompletionItem {
            label: n.to_string(),
            kind: Some(CompletionItemKind::ENUM_MEMBER),
            detail: Some("Livewire modifier".to_string()),
            ..Default::default()
        })
        .collect()
}

/// Is the current call the `@vite(` directive shape - method word
/// `vite` right after a `@`?
fn vite_directive(line: &str) -> bool {
    let Some(paren) = line.rfind('(') else {
        return false;
    };
    let head = line[..paren].trim_end();
    let word_start = head
        .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .map(|x| x + 1)
        .unwrap_or(0);
    &head[word_start..] == "vite" && head[..word_start].ends_with('@')
}

/// A Blade template file (`*.blade.php`).
fn is_blade_file(file: &Path) -> bool {
    file.file_name()
        .is_some_and(|n| n.to_string_lossy().ends_with(".blade.php"))
}

/// The harvested columns of `table` (items 390, 392).
fn column_items(lists: &Lists, table: &str) -> Vec<CompletionItem> {
    lists
        .columns
        .iter()
        .filter(|(t, _)| t == table)
        .take(MAX_ITEMS)
        .map(|(_, c)| CompletionItem {
            label: c.clone(),
            kind: Some(CompletionItemKind::PROPERTY),
            detail: Some(format!("{table} column")),
            ..Default::default()
        })
        .collect()
}

pub fn items(
    root: &Path,
    file: Option<&Path>,
    line_before: &str,
) -> Vec<CompletionItem> {
    if line_before.len() > 4096 {
        return Vec::new();
    }
    let Some(kind) = context(line_before) else { return Vec::new() };
    if kind == Kind::LivewireModel {
        return livewire_props(root, file);
    }
    if kind == Kind::LivewireAction {
        return livewire_methods(root, file);
    }
    if kind == Kind::LivewireTarget {
        // Actions first, then the wire:model-able properties.
        let mut targets = livewire_methods(root, file);
        targets.extend(livewire_props(root, file));
        targets.truncate(MAX_ITEMS);
        return targets;
    }
    if kind == Kind::ComponentTag {
        return component_tags(root, file);
    }
    if kind == Kind::LivewireProp {
        let Some(name) = livewire_prop_component(line_before) else {
            return Vec::new();
        };
        let Some(class) = crate::template_refs::livewire_component(root, &name)
        else {
            return Vec::new();
        };
        // Anonymous components resolve to a view and take no props.
        if class.to_string_lossy().ends_with(".blade.php") {
            return Vec::new();
        }
        let readable = std::fs::symlink_metadata(&class)
            .ok()
            .is_some_and(|m| m.is_file() && m.len() <= 1 << 20);
        if !readable {
            return Vec::new();
        }
        let Ok(text) = std::fs::read_to_string(&class) else {
            return Vec::new();
        };
        // Tags are HTML: attribute names are kebab-case. The
        // `@livewire('name', [...])` array is PHP: keys read as the
        // property names as declared (Livewire accepts both - mount
        // params with `-` are camel-cased for matching).
        let kebab_keys = livewire_prop_position(line_before).is_some();
        return crate::livewire_handlers::public_props(&text)
            .into_iter()
            .take(MAX_ITEMS)
            .map(|n| CompletionItem {
                label: if kebab_keys {
                    crate::blade_props::kebab(&n)
                } else {
                    n
                },
                kind: Some(CompletionItemKind::PROPERTY),
                detail: Some("Livewire mount property".to_string()),
                ..Default::default()
            })
            .collect();
    }
    if kind == Kind::BladeProp {
        let Some((tag, _)) = blade_prop_position(line_before) else {
            return Vec::new();
        };
        return crate::blade_props::component_prop_names(root, tag)
            .into_iter()
            .take(MAX_ITEMS)
            .map(|n| CompletionItem {
                label: n,
                kind: Some(CompletionItemKind::PROPERTY),
                detail: Some("Blade component prop".to_string()),
                ..Default::default()
            })
            .collect();
    }
    if kind == Kind::LivewireTag {
        return livewire_tags(root, file);
    }
    if kind == Kind::LivewireName {
        return livewire_tags(root, file);
    }
    if kind == Kind::LivewireModifier {
        return wire_modifiers_for(line_before);
    }
    if kind == Kind::AlpineModifier {
        return alpine_modifiers_for(line_before);
    }
    if kind == Kind::AlpineStore {
        return store_names(root)
            .into_iter()
            .take(MAX_ITEMS)
            .map(|n| CompletionItem {
                label: n,
                kind: Some(CompletionItemKind::VALUE),
                detail: Some("Alpine store".to_string()),
                ..Default::default()
            })
            .collect();
    }
    if kind == Kind::AlpineRef {
        return ref_names(file)
            .into_iter()
            .take(MAX_ITEMS)
            .map(|n| CompletionItem {
                label: n,
                kind: Some(CompletionItemKind::VALUE),
                detail: Some("Alpine x-ref".to_string()),
                ..Default::default()
            })
            .collect();
    }
    if kind == Kind::AlpineMagic {
        return alpine_magic_items();
    }
    if kind == Kind::AlpineData {
        return data_names(root)
            .into_iter()
            .take(MAX_ITEMS)
            .map(|n| CompletionItem {
                label: n,
                kind: Some(CompletionItemKind::VALUE),
                detail: Some("Alpine.data component".to_string()),
                ..Default::default()
            })
            .collect();
    }
    if kind == Kind::LivewireJs {
        return js_names(file)
            .into_iter()
            .take(MAX_ITEMS)
            .map(|n| CompletionItem {
                label: n,
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some("Livewire $js function".to_string()),
                ..Default::default()
            })
            .collect();
    }
    if kind == Kind::LivewireMagic {
        return LIVEWIRE_WIRE_MAGICS
            .iter()
            .map(|(label, detail, method)| CompletionItem {
                label: label.to_string(),
                kind: Some(if *method {
                    CompletionItemKind::METHOD
                } else {
                    CompletionItemKind::PROPERTY
                }),
                detail: Some(detail.to_string()),
                ..Default::default()
            })
            .collect();
    }
    if kind == Kind::LivewireEntangle {
        // Only properties entangle; methods are not valid arguments.
        return livewire_props(root, file);
    }
    if kind == Kind::LivewireWatch {
        // Only properties can be watched.
        return livewire_props(root, file);
    }
    if kind == Kind::LivewireMember {
        let props = livewire_props(root, file);
        let seen: std::collections::HashSet<String> =
            props.iter().map(|i| i.label.clone()).collect();
        let mut out = props;
        out.extend(
            livewire_methods(root, file)
                .into_iter()
                .filter(|i| !seen.contains(&i.label)),
        );
        return out;
    }
    if kind == Kind::LivewireEvent {
        return event_names(root)
            .into_iter()
            .take(MAX_ITEMS)
            .map(|n| CompletionItem {
                label: n,
                kind: Some(CompletionItemKind::VALUE),
                detail: Some("Livewire event".to_string()),
                ..Default::default()
            })
            .collect();
    }
    if kind == Kind::AlpineEvent {
        return DOM_EVENTS
            .iter()
            .take(MAX_ITEMS)
            .map(|n| CompletionItem {
                label: n.to_string(),
                kind: Some(CompletionItemKind::VALUE),
                detail: Some("Alpine event".to_string()),
                ..Default::default()
            })
            .collect();
    }
    if kind == Kind::LivewireAttribute {
        let mut names: Vec<String> = crate::livewire_handlers::EVENTS
            .iter()
            .map(|n| n.to_string())
            .collect();
        for (n, _, _) in crate::hover_help::ATTRIBUTES {
            if let Some(rest) = n.strip_prefix("wire:") {
                if !names.iter().any(|x| x.as_str() == rest) {
                    names.push(rest.to_string());
                }
            }
        }
        return names
            .into_iter()
            .take(MAX_ITEMS)
            .map(|n| CompletionItem {
                label: n,
                kind: Some(CompletionItemKind::VALUE),
                detail: Some("Livewire attribute".to_string()),
                ..Default::default()
            })
            .collect();
    }
    if kind == Kind::ValidationRule {
        return crate::validation_rules::items();
    }
    if kind == Kind::BlueprintMethod {
        // `$table->` is a convention, not a type: only migration
        // files get Blueprint methods, so an unrelated `$table`
        // variable elsewhere sees nothing.
        let in_migration = file
            .and_then(|f| f.strip_prefix(root).ok())
            .is_some_and(|rel| rel.starts_with("database/migrations/"));
        if !in_migration {
            return Vec::new();
        }
        return BLUEPRINT_METHODS
            .iter()
            .map(|(n, d)| CompletionItem {
                label: (*n).to_string(),
                kind: Some(CompletionItemKind::VALUE),
                detail: Some((*d).to_string()),
                ..Default::default()
            })
            .collect();
    }
    if kind == Kind::DbColumn {
        let Some(table) = rule_param_table(line_before)
            .or_else(|| fluent_rule_table(line_before))
        else {
            return Vec::new();
        };
        return column_items(&lists(root), table);
    }
    if kind == Kind::QueryColumn {
        // The receiver head: text before the open call's method
        // name, exactly as call_context sliced it.
        let Some(paren) = line_before.rfind('(') else {
            return Vec::new();
        };
        let word_start = paren
            - line_before[..paren]
                .bytes()
                .rev()
                .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                .count();
        let Some(table) = chain_table(&line_before[..word_start]) else {
            return Vec::new();
        };
        return column_items(&lists(root), table);
    }
    let lists = lists(root);
    let (names, detail) = match kind {
        Kind::Route => (lists.routes, "route name"),
        Kind::Config => (lists.configs, "config key"),
        Kind::Env => (lists.envs, "env key"),
        Kind::View => (lists.views, "Blade view"),
        Kind::Section => (lists.sections, "Blade section"),
        Kind::Stack => (lists.stacks, "Blade stack"),
        Kind::Lang => (lists.langs, "translation key"),
        // Returned early above; the match must stay exhaustive.
        Kind::LivewireModel => (Vec::new(), "Livewire property"),
        Kind::LivewireAction => (Vec::new(), "Livewire method"),
        // Returned early above; the match must stay exhaustive.
        Kind::ComponentTag => (Vec::new(), "Blade component"),
        // Returned early above; the match must stay exhaustive.
        Kind::BladeProp => (Vec::new(), "Blade component prop"),
        // Returned early above; the match must stay exhaustive.
        Kind::LivewireProp => (Vec::new(), "Livewire mount property"),
        // Returned early above; the match must stay exhaustive.
        Kind::LivewireName => (Vec::new(), "Livewire component"),
        // Returned early above; the match must stay exhaustive.
        Kind::LivewireModifier => (Vec::new(), "Livewire modifier"),
        // Returned early above; the match must stay exhaustive.
        Kind::AlpineModifier => (Vec::new(), "Alpine modifier"),
        // Returned early above; the match must stay exhaustive.
        Kind::AlpineStore => (Vec::new(), "Alpine store"),
        // Returned early above; the match must stay exhaustive.
        Kind::AlpineRef => (Vec::new(), "Alpine x-ref"),
        // Returned early above; the match must stay exhaustive.
        Kind::AlpineMagic => (Vec::new(), "Alpine magic"),
        // Returned early above; the match must stay exhaustive.
        Kind::LivewireMember => (Vec::new(), "Livewire member"),
        // Returned early above; the match must stay exhaustive.
        Kind::LivewireMagic => (Vec::new(), "Livewire $wire magic"),
        // Returned early above; the match must stay exhaustive.
        Kind::LivewireJs => (Vec::new(), "Livewire $js function"),
        // Returned early above; the match must stay exhaustive.
        Kind::LivewireEntangle => (Vec::new(), "Livewire property"),
        // Returned early above; the match must stay exhaustive.
        Kind::LivewireWatch => (Vec::new(), "Livewire property"),
        // Returned early above; the match must stay exhaustive.
        Kind::AlpineData => (Vec::new(), "Alpine.data component"),
        // Returned early above; the match must stay exhaustive.
        Kind::LivewireEvent => (Vec::new(), "Livewire event"),
        // Returned early above; the match must stay exhaustive.
        Kind::AlpineEvent => (Vec::new(), "Alpine event"),
        // Returned early above; the match must stay exhaustive.
        Kind::LivewireAttribute => (Vec::new(), "Livewire attribute"),
        // Returned early above; the match must stay exhaustive.
        Kind::LivewireTarget => (Vec::new(), "Livewire target"),
        Kind::LivewireTag => (Vec::new(), "Livewire component"),
        Kind::Asset => (lists.assets, "asset path"),
        Kind::Gate => (lists.gates, "Gate ability"),
        Kind::DbTable => (lists.tables, "database table"),
        // Returned early above; the match must stay exhaustive.
        Kind::DbColumn => (Vec::new(), "database column"),
        // Returned early above; the match must stay exhaustive.
        Kind::QueryColumn => (Vec::new(), "database column"),
        Kind::MiddlewareAlias => {
            (lists.middleware_aliases, "middleware alias")
        }
        Kind::StorageDisk => (lists.disks, "filesystem disk"),
        Kind::LogChannel => (lists.log_channels, "log channel"),
        Kind::CacheStore => (lists.cache_stores, "cache store"),
        Kind::MailMailer => (lists.mail_mailers, "mail mailer"),
        Kind::AuthGuard => (lists.auth_guards, "auth guard"),
        Kind::DbConnection => {
            (lists.db_connections, "database connection")
        }
        Kind::RedisConnection => {
            (lists.redis_connections, "redis connection")
        }
        Kind::QueueConnection => {
            (lists.queue_connections, "queue connection")
        }
        Kind::BroadcastConnection => {
            (lists.broadcast_connections, "broadcast connection")
        }
        Kind::ArtisanCommand => {
            (lists.artisan_commands, "artisan command")
        }
        // The `@vite` directive exists only in Blade files; the
        // `blade` flag in context() is just the preceding-@ shape,
        // so gate the directive form on the file here (item 398).
        // In a plain PHP file `@vite(...)` is the error-suppression
        // operator and offers nothing. `Vite::asset('...')` is plain
        // PHP and ungated.
        Kind::ViteEntry => {
            if vite_directive(line_before)
                && !file.is_some_and(is_blade_file)
            {
                return Vec::new();
            }
            (lists.vite_entries, "Vite entry point")
        }
        // Returned early above; the match must stay exhaustive.
        Kind::BlueprintMethod => (Vec::new(), "Blueprint method"),
        // Returned early above; the match must stay exhaustive.
        Kind::ValidationRule => (Vec::new(), "validation rule"),
    };
    names
        .into_iter()
        .take(MAX_ITEMS)
        .map(|n| CompletionItem {
            label: n,
            kind: Some(CompletionItemKind::VALUE),
            detail: Some(detail.to_string()),
            ..Default::default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
    }

    #[test]
    fn detects_contexts() {
        assert_eq!(context("route('us"), Some(Kind::Route));
        assert_eq!(context("to_route(\"us"), Some(Kind::Route));
        assert_eq!(context("Route::has('adm"), Some(Kind::Route));
        assert_eq!(context("config('app.na"), Some(Kind::Config));
        assert_eq!(context("env('DB_"), Some(Kind::Env));
        assert_eq!(context("view('pages."), Some(Kind::View));
        // Closed string, after the call, unknown function, Collection::has.
        assert_eq!(context("route('users.index')"), None);
        assert_eq!(context("route('users.index')->x"), None);
        assert_eq!(context("foo('x"), None);
        assert_eq!(context("$c->has('x"), None);
        assert_eq!(context(""), None);
        // Blade directives.
        assert_eq!(context("@extends('lay"), Some(Kind::View));
        assert_eq!(context("@include('partials."), Some(Kind::View));
        assert_eq!(context("@section('con"), Some(Kind::Section));
        assert_eq!(context("@yield('con"), Some(Kind::Section));
        // Livewire full-page chains: `->layout('...')` is a view,
        // `->section('...')` a section; the bare helper is not one.
        assert_eq!(
            context("return view('livewire.counter')->layout('lay"),
            Some(Kind::View)
        );
        assert_eq!(
            context("return view('livewire.counter')->section('con"),
            Some(Kind::Section)
        );
        assert_eq!(context("layout('lay"), None);
        // Database tables (item 381): schema/query contexts name an
        // existing table; `Schema::create` names a new one, so it and
        // a bare `table('...')` stay silent.
        assert_eq!(context("Schema::table('us"), Some(Kind::DbTable));
        assert_eq!(context("Schema::dropIfExists('us"), Some(Kind::DbTable));
        assert_eq!(context("DB::table('us"), Some(Kind::DbTable));
        assert_eq!(
            context("$table->foreignId('user_id')->constrained('us"),
            Some(Kind::DbTable)
        );
        assert_eq!(context("->on('us"), Some(Kind::DbTable));
        assert_eq!(context("->leftJoin('us"), Some(Kind::DbTable));
        assert_eq!(context("Schema::create('us"), None);
        assert_eq!(context("table('us"), None);
        // Blueprint methods (item 382): the `$table->` name position,
        // with argument, chain and `$this->table->` negatives.
        assert_eq!(context("$table->str"), Some(Kind::BlueprintMethod));
        assert_eq!(context("$table->"), Some(Kind::BlueprintMethod));
        assert_eq!(
            context("Schema::table('users', function (Blueprint $table) { $table->str"),
            Some(Kind::BlueprintMethod)
        );
        assert_eq!(context("$table->string('na"), None);
        assert_eq!(context("$table->string('x')->nul"), None);
        assert_eq!(context("$this->table->str"), None);
        assert_eq!(context("@push('scr"), Some(Kind::Stack));
        assert_eq!(context("@stack('mod"), Some(Kind::Stack));
        // Without the @, these are ordinary calls.
        assert_eq!(context("yield('x"), None);
        assert_eq!(context("section('x"), None);
        // Translation keys.
        assert_eq!(context("__('wel"), Some(Kind::Lang));
        assert_eq!(context("__(\"wel"), Some(Kind::Lang));
        assert_eq!(context("trans('mess"), Some(Kind::Lang));
        assert_eq!(context("trans_choice('apple"), Some(Kind::Lang));
        assert_eq!(context("@lang('mess"), Some(Kind::Lang));
        assert_eq!(context("@choice('apple"), Some(Kind::Lang));
        assert_eq!(context("Lang::get('mess"), Some(Kind::Lang));
        assert_eq!(context("Lang::has('mess"), Some(Kind::Lang));
        // Without the @ or Lang::, these are ordinary calls.
        assert_eq!(context("lang('x"), None);
        assert_eq!(context("$c->choice('x"), None);
        assert_eq!(context("Str::get('x"), None);
        // Livewire wire:model, any quote style or modifier chain.
        assert_eq!(
            context("<input wire:model=\"em"),
            Some(Kind::LivewireModel)
        );
        assert_eq!(
            context("<input wire:model.live=\"em"),
            Some(Kind::LivewireModel)
        );
        assert_eq!(
            context("<input wire:model.debounce.300ms='em"),
            Some(Kind::LivewireModel)
        );
        // Closed value, other attributes and Alpine are not model contexts.
        assert_eq!(context("<input wire:model=\"em\""), None);
        assert_eq!(context("<div x-data=\"{ m: 'x' }\">"), None);
        // Livewire event attributes complete methods.
        assert_eq!(
            context("<button wire:click=\"sa"),
            Some(Kind::LivewireAction)
        );
        assert_eq!(
            context("<form wire:submit.prevent=\"sa"),
            Some(Kind::LivewireAction)
        );
        assert_eq!(
            context("<input wire:keydown.enter='sa"),
            Some(Kind::LivewireAction)
        );
        // Non-event directive attributes complete modifiers after the
        // dot (item 362); their values and closed values are not method
        // contexts.
        assert_eq!(
            context("<div wire:loading.cla"),
            Some(Kind::LivewireModifier)
        );
        assert_eq!(context("<div wire:loading=\"cl"), None);
        // wire:target values are actions or properties (item 379).
        assert_eq!(context("<div wire:target=\"sa"), Some(Kind::LivewireTarget));
        assert_eq!(
            context("<div wire:loading wire:target=\"save, de"),
            Some(Kind::LivewireTarget)
        );
        assert_eq!(context("<div wire:target=\"save\""), None);
        assert_eq!(context("<div data-wire:target=\"sa"), None);
        assert_eq!(context("<div wire:key=\"it"), None);
        assert_eq!(context("<button wire:click=\"sa\""), None);
        assert_eq!(context("<button data-wire:click=\"sa"), None);
        assert_eq!(context("<button x-on:wire:click=\"sa"), None);
    }

    #[test]
    fn offers_project_names() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "routes/web.php", "<?php\nRoute::get('/users', [UserController::class, 'index'])->name('users.index');\nRoute::get('/', fn () => view('home'))->name(\"home\");\n");
        put(r, "config/app.php", "<?php\nreturn [\n    'name' => env('APP_NAME', 'Laravel'),\n];\n");
        put(r, ".env", "APP_NAME=Acme\nDB_HOST=db\n");
        put(r, "resources/views/pages/home.blade.php", "<h1>hi</h1>\n");
        put(r, "resources/views/emails/welcome.php", "<p>w</p>\n");
        put(r, "resources/views/layouts/app.blade.php", "<html>\n@yield('content')\n@yield(\"scripts\")\n@stack('modals')\n@section('sidebar')\n@show\n</html>\n");
        put(r, "resources/lang/en/auth.php", "<?php\nreturn [\n    'failed' => 'These credentials do not match.',\n    'throttle' => [\n        'seconds' => 'Too many.',\n    ],\n];\n");
        put(r, "resources/lang/fr/auth.php", "<?php\nreturn [\n    'failed' => 'Ces identifiants ne correspondent pas.',\n    'bonjour' => 'Bonjour',\n];\n");
        put(r, "resources/lang/en.json", "{\n  \"Welcome back\": \"Welcome back\"\n}\n");

        let labels = |kind_line: &str| {
            items(r, None, kind_line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(labels("route('"), ["users.index", "home"]);
        assert!(labels("config('").contains(&"app.name".to_string()));
        assert_eq!(labels("env('"), ["APP_NAME", "DB_HOST"]);
        assert_eq!(
            labels("view('"),
            ["emails.welcome", "layouts.app", "pages.home"]
        );
        assert!(labels("foo('").is_empty());
        assert_eq!(labels("@section('"), ["content", "scripts", "sidebar"]);
        assert_eq!(labels("@yield('"), ["content", "scripts", "sidebar"]);
        assert_eq!(labels("@stack('"), ["modals"]);
        assert!(labels("@extends('").contains(&"layouts.app".to_string()));
        // Translation keys: nested PHP keys dot-flattened, JSON keys as-is,
        // a key in several locales listed once, fr-only keys included.
        let langs = labels("__('");
        assert!(langs.contains(&"auth.failed".to_string()));
        assert!(langs.contains(&"auth.throttle.seconds".to_string()));
        assert!(langs.contains(&"Welcome back".to_string()));
        assert!(langs.contains(&"auth.bonjour".to_string()));
        assert_eq!(
            langs.iter().filter(|l| *l == "auth.failed").count(),
            1
        );
        assert_eq!(labels("trans('"), langs);
        assert_eq!(labels("@lang('"), langs);
    }

    #[test]
    fn detects_alpine_store_context() {
        assert_eq!(context("x-on:click=\"$store."), Some(Kind::AlpineStore));
        assert_eq!(context("wire:click=\"$store.na"), Some(Kind::AlpineStore));
        assert_eq!(context("$store"), None);
        assert_eq!(context("$store.nav.open"), None);
        assert_eq!(context("x-data=\"{ s: $store.na }\""), None);
    }

    #[test]
    fn offers_alpine_stores() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "resources/js/app.js",
            "Alpine.store('nav', { open: false });\nAlpine.store(\"dark-mode\", {});\n",
        );
        put(
            r,
            "resources/views/layouts/app.blade.php",
            "<script>Alpine.store('theme', {});</script>\n",
        );
        let got = labels(items(r, None, "x-on:click=\"$store."));
        assert!(got.contains(&"nav".to_string()), "{got:?}");
        assert!(got.contains(&"dark-mode".to_string()), "{got:?}");
        assert!(got.contains(&"theme".to_string()), "{got:?}");
    }

    #[test]
    fn detects_alpine_ref_context() {
        assert_eq!(context("x-on:click=\"$refs."), Some(Kind::AlpineRef));
        assert_eq!(context("$refs.in"), Some(Kind::AlpineRef));
        assert_eq!(context("$refs"), None);
        assert_eq!(context("$refs.a.b"), None);
    }

    #[test]
    fn offers_alpine_refs() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "resources/views/p.blade.php",
            "<input x-ref=\"email\" />\n<span x-ref='feedback'></span>\n",
        );
        let f = r.join("resources/views/p.blade.php");
        let got = labels(items(r, Some(&f), "x-on:click=\"$refs."));
        assert!(got.contains(&"email".to_string()), "{got:?}");
        assert!(got.contains(&"feedback".to_string()), "{got:?}");
        assert!(items(r, None, "x-on:click=\"$refs.").is_empty());
    }

    #[test]
    fn detects_alpine_magic_context() {
        let magic = Some(Kind::AlpineMagic);
        // A bare `$` and partial names in Alpine expression values.
        assert_eq!(context(r#"x-show="$"#), magic);
        assert_eq!(context(r#"x-show="$ne"#), magic);
        assert_eq!(context(r#"<button @click="$"#), magic);
        assert_eq!(context(r#"<span :class="$ro"#), magic);
        assert_eq!(context(r#"x-on:click="$st"#), magic);
        // Mid-expression, after another operator.
        assert_eq!(context(r#"x-show="open && $"#), magic);
        // `wire:` attribute values too: `$dispatch`/`$wire` belong there.
        assert_eq!(context(r#"wire:click="$di"#), magic);
        assert_eq!(context(r#"wire:model="$"#), magic);
        // Plain attributes and x-data are not magic contexts.
        assert_eq!(context(r#"class="$"#), None);
        assert_eq!(context(r#"x-data="$"#), None);
        assert_eq!(context(r#"href="$"#), None);
        // A partial that starts no known magic is not a magic context:
        // the line falls back to whatever it otherwise is.
        assert_eq!(context(r#"x-show="$foo"#), None);
        assert_eq!(context(r#"x-on:click="$xyz"#), None);
        assert_eq!(
            context(r#"wire:click="$foo"#),
            Some(Kind::LivewireAction)
        );
        // `foo$`, `$$` and `$this->` are not magic starts.
        assert_eq!(context(r#"x-show="count$"#), None);
        assert_eq!(context(r#"x-show="$$"#), None);
        assert_eq!(context(r#"x-show="$this->di"#), None);
        // The more specific `$store.` / `$refs.` contexts still win.
        assert_eq!(
            context(r#"x-on:click="$store."#),
            Some(Kind::AlpineStore)
        );
        assert_eq!(context(r#"x-on:click="$refs."#), Some(Kind::AlpineRef));
        // A helper call keeps its own context.
        assert_eq!(
            context(r#"@click="$dispatch('"#),
            Some(Kind::LivewireEvent)
        );
    }

    #[test]
    fn offers_alpine_magics() {
        let r = std::path::Path::new("/nonexistent-project-root-367");
        let labels: Vec<_> = items(r, None, r#"x-show="$"#)
            .into_iter()
            .map(|i| i.label)
            .collect();
        assert_eq!(
            labels,
            [
                "$el", "$refs", "$store", "$watch", "$dispatch", "$nextTick",
                "$root", "$id", "$data", "$wire", "$js", "$entangle",
            ]
        );
        // The same list in a `wire:` value; nothing outside the context.
        assert_eq!(items(r, None, r#"wire:click="$"#).len(), 12);
        assert!(items(r, None, r#"class="$"#).is_empty());
    }

    #[test]
    fn detects_livewire_wire_member_context() {
        let member = Some(Kind::LivewireMember);
        // A bare `$wire.` and partial member names in Alpine expression
        // and `wire:` attribute values.
        assert_eq!(context(r#"x-on:click="$wire."#), member);
        assert_eq!(context(r#"<button @click="$wire.sa"#), member);
        assert_eq!(context(r#"x-show="$wire.cou"#), member);
        assert_eq!(context(r#"wire:click="$wire.sa"#), member);
        assert_eq!(context(r#"wire:model="$wire."#), member);
        // Plain attributes and x-data are not member contexts.
        assert_eq!(context(r#"class="$wire."#), None);
        assert_eq!(context(r#"x-data="$wire."#), None);
        // A partial magic name still wins before the dot.
        assert_eq!(context(r#"x-on:click="$wi"#), Some(Kind::AlpineMagic));
        // `$wire.$` is not an Alpine magic; since item 371 it is the
        // `$wire` object's own magic context.
        assert_eq!(context(r#"x-on:click="$wire.$"#), Some(Kind::LivewireMagic));
        // Helper calls on `$wire` keep their own contexts.
        assert_eq!(
            context(r#"x-on:click="$wire.dispatch('"#),
            Some(Kind::LivewireEvent)
        );
        assert_eq!(context(r#"x-on:click="$wire.save('"#), None);
    }

    #[test]
    fn offers_livewire_wire_members() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "resources/views/livewire/user-profile.blade.php",
            "<button>\n",
        );
        put(
            r,
            "app/Livewire/UserProfile.php",
            "<?php\nclass UserProfile extends \\Livewire\\Component {\n    public $email = '';\n    public function save() {}\n    private function helper() {}\n}\n",
        );
        let view = r.join("resources/views/livewire/user-profile.blade.php");
        let labels = |line: &str| {
            items(r, Some(view.as_path()), line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        // Properties first, then methods, private members left out.
        assert_eq!(labels("<button @click=\"$wire."), ["email", "save"]);
        assert_eq!(labels("<span x-show=\"$wire.em"), ["email", "save"]);
        assert_eq!(labels("<input wire:click=\"$wire."), ["email", "save"]);
        assert!(labels("<span class=\"$wire.").is_empty());
        // Without a paired view file there is nothing to offer.
        assert!(items(r, None, "<button @click=\"$wire.").is_empty());
    }

    #[test]
    fn detects_alpine_data_context() {
        let data = Some(Kind::AlpineData);
        // A bare `x-data="` and partial component names, either quote.
        assert_eq!(context(r#"<div x-data=""#), data);
        assert_eq!(context(r#"<div x-data="dro"#), data);
        assert_eq!(context(r#"<div x-data='sea"#), data);
        assert_eq!(context(r#"<div x-data="search-fo"#), data);
        // The whole value must be a partial name: a call or expression
        // falls out of context.
        assert_eq!(context(r#"<div x-data="dropdown({ cou"#), None);
        assert_eq!(context(r#"<div x-data="{ open: fa"#), None);
        // A closed x-data earlier on the line does not count.
        assert_eq!(context(r#"<div x-data="dropdown" x-show="op"#), None);
        // A longer attribute name containing `x-data=` is not x-data.
        assert_eq!(context(r#"<div data-x-data="dro"#), None);
        // Other attributes and expressions are untouched.
        assert_eq!(context(r#"<div class="dro"#), None);
        assert_eq!(context(r#"x-show="dro"#), None);
        assert_eq!(context(r#"x-data="$"#), None);
    }

    #[test]
    fn offers_alpine_data_names() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "resources/js/components.js",
            "Alpine.data('dropdown', () => ({ open: false }));\nAlpine.data(\"search-form\", () => ({}));\n",
        );
        put(
            r,
            "resources/views/layouts/app.blade.php",
            "<script>Alpine.data('tabs', () => ({}));</script>\n",
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        let got = labels("<div x-data=\"");
        assert_eq!(got, ["dropdown", "search-form", "tabs"]);
        assert_eq!(labels("<div x-data=\"dro"), ["dropdown", "search-form", "tabs"]);
        assert!(labels("<div class=\"dro").is_empty());
    }

    #[test]
    fn detects_livewire_wire_magic_context() {
        let magic = Some(Kind::LivewireMagic);
        // A bare `$wire.$` and partial known magic names.
        assert_eq!(context(r#"x-on:click="$wire.$"#), magic);
        assert_eq!(context(r#"<button @click="$wire.$se"#), magic);
        assert_eq!(context(r#"x-show="$wire.$tog"#), magic);
        assert_eq!(context(r#"wire:click="$wire.$re"#), magic);
        // Unknown partials and deeper paths are not magic contexts.
        assert_eq!(context(r#"x-on:click="$wire.$foo"#), None);
        // Since item 372 this is the `$js` function-name context.
        assert_eq!(context(r#"x-on:click="$wire.$js."#), Some(Kind::LivewireJs));
        // Plain attributes and x-data are not magic contexts.
        assert_eq!(context(r#"class="$wire.$"#), None);
        assert_eq!(context(r#"x-data="$wire.$"#), None);
        // `$wire.<member>` still wins without the second `$`.
        assert_eq!(
            context(r#"x-on:click="$wire.sa"#),
            Some(Kind::LivewireMember)
        );
        // A call into a magic falls out of context.
        assert_eq!(context(r#"x-on:click="$wire.$set('"#), None);
    }

    #[test]
    fn offers_livewire_wire_magics() {
        let r = std::path::Path::new("/nonexistent-project-root-371");
        let labels: Vec<_> = items(r, None, r#"x-on:click="$wire.$"#)
            .into_iter()
            .map(|i| i.label)
            .collect();
        assert_eq!(
            labels,
            ["$set", "$get", "$toggle", "$refresh", "$parent", "$js"]
        );
        assert_eq!(items(r, None, r#"wire:click="$wire.$to"#).len(), 6);
        assert!(items(r, None, r#"class="$wire.$"#).is_empty());
    }

    #[test]
    fn detects_livewire_js_context() {
        let js = Some(Kind::LivewireJs);
        // A bare `$wire.$js.` and partial function names.
        assert_eq!(context(r#"x-on:click="$wire.$js."#), js);
        assert_eq!(context(r#"<button @click="$wire.$js.co"#), js);
        assert_eq!(context(r#"wire:click="$wire.$js.save"#), js);
        // Plain attributes and x-data are not contexts.
        assert_eq!(context(r#"class="$wire.$js."#), None);
        assert_eq!(context(r#"x-data="$wire.$js."#), None);
        // `$wire.$j`/`$wire.$js` (no dot yet) stay `$wire` magics.
        assert_eq!(context(r#"x-on:click="$wire.$j"#), Some(Kind::LivewireMagic));
        assert_eq!(context(r#"x-on:click="$wire.$js"#), Some(Kind::LivewireMagic));
        // A lookalike (`$jsx`) and a deeper path are not contexts.
        assert_eq!(context(r#"x-on:click="$wire.$jsx."#), None);
        assert_eq!(context(r#"x-on:click="$wire.$js.copy()"#), None);
    }

    #[test]
    fn offers_livewire_js_names() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "resources/views/livewire/clipboard.blade.php",
            "<div>\n@script\n<script>\n    $js('copy', () => { $wire.save() });\n    $js(\"confirmSave\", () => {});\n</script>\n@endscript\n</div>\n",
        );
        let view = r.join("resources/views/livewire/clipboard.blade.php");
        let labels = |line: &str| {
            items(r, Some(view.as_path()), line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(labels("<button @click=\"$wire.$js."), ["confirmSave", "copy"]);
        assert_eq!(labels("<button @click=\"$wire.$js.co"), ["confirmSave", "copy"]);
        assert!(labels("<button class=\"$wire.$js.").is_empty());
        // Without a file there is nothing to scan.
        assert!(items(r, None, "<button @click=\"$wire.$js.").is_empty());
    }

    #[test]
    fn detects_livewire_entangle_context() {
        let ent = Some(Kind::LivewireEntangle);
        // Bare, `$`-magic and `$wire`-member call shapes.
        assert_eq!(context("$entangle('"), ent);
        assert_eq!(context("$entangle('sh"), ent);
        assert_eq!(context("$wire.entangle(\""), ent);
        // The common home: an x-data object value.
        assert_eq!(context("x-data=\"{ open: $entangle('sh"), ent);
        assert_eq!(context("<button @click=\"$entangle('o"), ent);
        // No open argument string: no context.
        assert_eq!(context("$entangle("), None);
        assert_eq!(context("$entangle('show'"), None);
        assert_eq!(context("$entangle('show', "), None);
        assert_eq!(context("$entangle('show')"), None);
    }

    #[test]
    fn offers_livewire_entangle_props() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "resources/views/livewire/user-profile.blade.php",
            "<button>\n",
        );
        put(
            r,
            "app/Livewire/UserProfile.php",
            "<?php\nclass UserProfile extends \\Livewire\\Component {\n    public $email = '';\n    public function save() {}\n}\n",
        );
        let view = r.join("resources/views/livewire/user-profile.blade.php");
        let labels = |line: &str| {
            items(r, Some(view.as_path()), line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        // Properties entangle; methods do not.
        assert_eq!(labels("x-data=\"{ open: $entangle('"), ["email"]);
        assert_eq!(labels("<button @click=\"$entangle('em"), ["email"]);
        // Without a paired view file there is nothing to offer.
        assert!(items(r, None, "$entangle('").is_empty());
    }

    #[test]
    fn detects_livewire_watch_context() {
        let watch = Some(Kind::LivewireWatch);
        // `$wire.$watch('...')` in any call position.
        assert_eq!(context("$wire.$watch('"), watch);
        assert_eq!(context("$wire.$watch('em"), watch);
        assert_eq!(context("x-on:click=\"$wire.$watch('o"), watch);
        assert_eq!(context("@script $wire.$watch(\"open"), watch);
        // No open argument string: no context.
        assert_eq!(context("$wire.$watch("), None);
        assert_eq!(context("$wire.$watch('show', "), None);
        // The property is the first argument; a later string is not one.
        assert_eq!(context("$wire.$watch('open', '"), None);
        // A lookalike and a deeper member path are not contexts.
        assert_eq!(context("$wire.$watcher('"), None);
        assert_eq!(context("$this->watch('"), None);
    }

    #[test]
    fn offers_livewire_watch_props() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "resources/views/livewire/user-profile.blade.php",
            "<button>\n",
        );
        put(
            r,
            "app/Livewire/UserProfile.php",
            "<?php\nclass UserProfile extends \\Livewire\\Component {\n    public $email = '';\n    public function save() {}\n}\n",
        );
        let view = r.join("resources/views/livewire/user-profile.blade.php");
        let labels = |line: &str| {
            items(r, Some(view.as_path()), line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        // Only properties can be watched; methods stay out.
        assert_eq!(labels("<button @click=\"$wire.$watch('"), ["email"]);
        assert_eq!(labels("@script $wire.$watch('em"), ["email"]);
        // Without a paired view file there is nothing to offer.
        assert!(items(r, None, "$wire.$watch('").is_empty());
    }

    #[test]
    fn detects_blade_prop_context() {
        let prop = Some(Kind::BladeProp);
        // After the tag name: empty and partial attribute names.
        assert_eq!(context("<x-input "), prop);
        assert_eq!(context(" <x-forms.input ty"), prop);
        assert_eq!(context(r#"<x-input type="email" di"#), prop);
        // The tag-name position stays the tag-name context.
        assert_eq!(context("<x-forms.in"), Some(Kind::ComponentTag));
        // Inside a quoted value, at a value position, after `/>` or the
        // self-closing slash: no context.
        assert_eq!(context(r#"<x-input type="em"#), None);
        assert_eq!(context("<x-input type="), None);
        // Right after a closing quote, before the space: no popup.
        assert_eq!(context(r#"<x-input type="email""#), None);
        assert_eq!(context(r#"<x-input type="email" />"#), None);
        assert_eq!(context("<x-input /"), None);
        // Slots take no props; a name-ish char before `<x-` is a word.
        assert_eq!(context("<x-slot:name ti"), None);
        assert_eq!(context("<x-slot "), None);
        assert_eq!(context("foo<x-input "), None);
        // Framework attribute positions keep their PRE-EXISTING
        // contexts (checked earlier): props defer to them.
        assert_eq!(context("<x-input @"), Some(Kind::AlpineEvent));
        assert_eq!(context("<x-input @cl"), Some(Kind::AlpineEvent));
        assert_eq!(context("<x-input wire:cl"), Some(Kind::LivewireAttribute));
        // `x-data` has no name context of its own; the `:disabled`
        // binding prefix completes props.
        assert_eq!(context("<x-input x-da"), None);
        assert_eq!(context("<x-input :di"), Some(Kind::BladeProp));
    }

    #[test]
    fn offers_blade_component_props() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "resources/views/components/forms/input.blade.php",
            "@props([\n    'type' => 'text',\n    'label',\n    'maxLength' => 20,\n])\n<input />\n",
        );
        put(
            r,
            "app/View/Components/Alert.php",
            "<?php\nclass Alert {\n    public function __construct(\n        public string $type,\n        public bool $dismissible = false,\n    ) {}\n}\n",
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        // Declaration order; camelCase keys are offered as kebab-case.
        assert_eq!(labels("<x-forms.input "), ["type", "label", "max-length"]);
        assert_eq!(labels("<x-forms.input la"), ["type", "label", "max-length"]);
        // A class component offers its constructor parameters.
        assert_eq!(labels("<x-alert "), ["type", "dismissible"]);
        // Unknown components and value positions offer nothing.
        assert!(labels("<x-nope ").is_empty());
        assert!(labels("<x-forms.input type=").is_empty());
    }

    #[test]
    fn detects_livewire_prop_context() {
        let prop = Some(Kind::LivewireProp);
        // After the tag name: empty and partial mount-property names.
        assert_eq!(context("<livewire:counter "), prop);
        assert_eq!(context(" <livewire:admin.user-profile em"), prop);
        assert_eq!(context(r#"<livewire:counter count="5" st"#), prop);
        // The `:prop` binding prefix completes too (Livewire 3).
        assert_eq!(context("<livewire:counter :cou"), prop);
        // The tag-name position stays the tag-name context.
        assert_eq!(context("<livewire:admin.us"), Some(Kind::LivewireTag));
        // Inside a quoted value, at a value position, after `/>`, the
        // self-closing slash or a closing quote: no context.
        assert_eq!(context(r#"<livewire:counter count="5"#), None);
        assert_eq!(context("<livewire:counter count="), None);
        assert_eq!(context(r#"<livewire:counter count="5" />"#), None);
        assert_eq!(context(r#"<livewire:counter :n="1""#), None);
        // wire: attribute names and @listeners keep their PRE-EXISTING
        // contexts (checked earlier): mount props defer to them.
        assert_eq!(
            context("<livewire:counter wire:cl"),
            Some(Kind::LivewireAttribute)
        );
        assert_eq!(context("<livewire:counter @"), Some(Kind::AlpineEvent));
    }

    #[test]
    fn offers_livewire_mount_props() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "app/Livewire/Counter.php",
            "<?php\nclass Counter extends \\Livewire\\Component {\n    public $count = 0;\n    public $maxCount = 10;\n    private $secret = 1;\n    public function save() {}\n}\n",
        );
        put(r, "resources/views/livewire/counter.blade.php", "<div>\n");
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        // Public properties only, camelCase offered as kebab-case.
        assert_eq!(labels("<livewire:counter "), ["count", "max-count"]);
        assert_eq!(labels("<livewire:counter ma"), ["count", "max-count"]);
        // Unknown components and value positions offer nothing.
        assert!(labels("<livewire:nope ").is_empty());
        assert!(labels("<livewire:counter count=").is_empty());
    }

    #[test]
    fn detects_livewire_directive_prop_context() {
        let prop = Some(Kind::LivewireProp);
        // `@livewire('name', ['key' => ...])`: mount property keys.
        assert_eq!(context("@livewire('counter', ['"), prop);
        assert_eq!(context("@livewire('counter', ['cou"), prop);
        assert_eq!(context("@livewire('counter', ['count' => 1, 'st"), prop);
        assert_eq!(context("@livewire(\"counter\", [\"ma"), prop);
        // Array values and a bare second argument are not keys.
        assert_eq!(context("@livewire('counter', ['count' => 'va"), None);
        assert_eq!(context("@livewire('counter', 'st"), None);
        // The first argument keeps the component-name context.
        assert_eq!(context("@livewire('cou"), Some(Kind::LivewireName));
    }

    #[test]
    fn offers_livewire_directive_mount_props() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "app/Livewire/Counter.php",
            "<?php\nclass Counter extends \\Livewire\\Component {\n    public $count = 0;\n    public $maxCount = 10;\n}\n",
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(labels("@livewire('counter', ['"), ["count", "maxCount"]);
        assert_eq!(labels("@livewire('counter', ['cou"), ["count", "maxCount"]);
        assert!(labels("@livewire('nope', ['").is_empty());
        assert!(labels("@livewire('counter', ['count' => '").is_empty());
    }

    #[test]
    fn detects_validation_rule_context() {
        let rules = Some(Kind::ValidationRule);
        // `->validate([...])`: pipe form and nested array form.
        assert_eq!(context("$this->validate(['name' => 'req|mai"), rules);
        assert_eq!(context("$request->validate(['email' => 'ema"), rules);
        assert_eq!(context("$this->validate(['name' => ['required', 'ma"), rules);
        // `Validator::make($data, [...])`: rules are the second argument.
        assert_eq!(context("Validator::make($data, ['name' => 'mi"), rules);
        // Livewire attribute and conditional rules.
        assert_eq!(context("#[Validate('req|mi"), rules);
        assert_eq!(context("#[Validate(['required', 'mi"), rules);
        assert_eq!(
            context("$validator->sometimes('reason', 'required|ma"),
            rules
        );
        // Field names, custom messages, rule parameters, a bare string
        // argument and unrelated calls stay silent.
        assert_eq!(context("$this->validate(['na"), None);
        assert_eq!(context("$this->validate(['name' => 'required'], ['na"), None);
        assert_eq!(context("$this->validate(['name' => 'max:1"), None);
        assert_eq!(context("$this->validate(['name' => 'min:2|max:5"), None);
        // `exists:` / `unique:` parameters (items 388, 389): the
        // table position offers tables, the position after the comma
        // offers that table's columns, an empty table stays silent.
        let table = Some(Kind::DbTable);
        let column = Some(Kind::DbColumn);
        assert_eq!(context("$this->validate(['email' => 'exists:us"), table);
        assert_eq!(
            context("$this->validate(['email' => 'required|unique:us"),
            table
        );
        assert_eq!(context("#[Validate('exists:us"), table);
        assert_eq!(context("Validator::make($data, ['email' => 'unique:us"), table);
        assert_eq!(context("$this->validate(['email' => 'exists:users,em"), column);
        assert_eq!(context("$this->validate(['email' => 'unique:users,"), column);
        assert_eq!(context("$this->validate(['email' => 'exists:,em"), None);
        // Grounded parameter grammar (item 390b): value positions -
        // the unique except value and both rules' condition values -
        // stay silent; the unique idColumn and the condition columns
        // of both rules offer columns.
        assert_eq!(context("$this->validate(['e' => 'unique:users,email,1"), None);
        assert_eq!(
            context("$this->validate(['e' => 'unique:users,email,1,id"),
            column
        );
        assert_eq!(
            context("$this->validate(['e' => 'unique:users,email,1,id,acc"),
            column
        );
        assert_eq!(
            context("$this->validate(['e' => 'unique:users,email,1,id,account_id,1"),
            None
        );
        assert_eq!(
            context("$this->validate(['e' => 'exists:users,email,acc"),
            column
        );
        assert_eq!(
            context("$this->validate(['e' => 'exists:users,email,account_id,1"),
            None
        );
        assert_eq!(context("$this->validate('req|ma"), None);
        assert_eq!(context("$this->check(['name' => 'req"), None);
    }

    #[test]
    fn detects_query_column_context() {
        let col = Some(Kind::QueryColumn);
        assert_eq!(context("DB::table('users')->where('em"), col);
        assert_eq!(context("DB::table('users')->orderBy('na"), col);
        assert_eq!(
            context("DB::table('users')->where('email', 1)->orWhere('na"),
            col
        );
        assert_eq!(context("DB::table('users')->select('na"), col);
        // Nested chain: the nearest table wins.
        assert_eq!(
            context("DB::table('u')->whereIn('id', DB::table('p')->select('i"),
            col
        );
        // A completed nested subquery is skipped: the outer call's
        // receiver is the outer table.
        assert_eq!(
            context(
                "DB::table('u')->whereIn('id', DB::table('p')->select('i'))->where('"
            ),
            col
        );
        // No base table on the line, a model static call, an
        // unrelated receiver after a statement boundary, and a
        // non-column argument position stay silent.
        assert_eq!(context("->where('em"), None);
        assert_eq!(context("User::where('em"), None);
        assert_eq!(
            context("$x = DB::table('users')->count(); $q->where('em"),
            None
        );
        assert_eq!(context("DB::table('users')->where('email', '"), None);
        assert_eq!(context("DB::table('users')->insert(['"), None);
    }

    #[test]
    fn detects_artisan_command_context() {
        let cs = Some(Kind::ArtisanCommand);
        assert_eq!(context("Artisan::call('re"), cs);
        assert_eq!(context("Artisan::queue('re"), cs);
        // A scalar name only; look-alike receivers, object calls
        // ($schedule->command is a different, ungrounded receiver),
        // a second argument and comment or string-contained calls
        // stay silent.
        assert_eq!(context("Artisan::call(['re"), None);
        assert_eq!(context("MyArtisan::call('re"), None);
        assert_eq!(context("$artisan->call('re"), None);
        assert_eq!(context("Artisan::call('reports:send', ['x"), None);
        assert_eq!(context("// Artisan::call('re"), None);
        assert_eq!(context("$h = \"use Artisan::call('re"), None);
    }

    #[test]
    fn detects_broadcast_connection_context() {
        let cs = Some(Kind::BroadcastConnection);
        assert_eq!(context("Broadcast::connection('pu"), cs);
        assert_eq!(context("Broadcast::driver('pu"), cs);
        // A scalar name only; the other connection and driver arms
        // do not steal the Broadcast receiver; look-alike
        // receivers, object calls, a second argument and comment
        // or string-contained calls stay silent.
        assert_eq!(context("Broadcast::connection(['pu"), None);
        assert_eq!(context("MyBroadcast::connection('pu"), None);
        assert_eq!(context("$broadcast->connection('pu"), None);
        assert_eq!(context("Broadcast::driver('pusher', 'x"), None);
        assert_eq!(context("// Broadcast::connection('pu"), None);
        assert_eq!(
            context("$h = \"use Broadcast::connection('pu"),
            None
        );
    }

    #[test]
    fn detects_queue_connection_context() {
        let cs = Some(Kind::QueueConnection);
        assert_eq!(context("Queue::connection('re"), cs);
        // A scalar name only; the DB and Redis arms do not steal
        // the Queue receiver; look-alike receivers, object calls, a
        // second argument and comment or string-contained calls
        // stay silent.
        assert_eq!(context("Queue::connection(['re"), None);
        assert_eq!(context("MyQueue::connection('re"), None);
        assert_eq!(context("$queue->connection('re"), None);
        assert_eq!(context("Queue::connection('redis', 'x"), None);
        assert_eq!(context("// Queue::connection('re"), None);
        assert_eq!(context("$h = \"use Queue::connection('re"), None);
    }

    #[test]
    fn detects_redis_connection_context() {
        let cs = Some(Kind::RedisConnection);
        assert_eq!(context("Redis::connection('de"), cs);
        // A scalar name only; the DB arm does not steal the Redis
        // receiver; look-alike receivers, object calls, a second
        // argument and comment or string-contained calls stay
        // silent.
        assert_eq!(context("Redis::connection(['de"), None);
        assert_eq!(context("MyRedis::connection('de"), None);
        assert_eq!(context("$redis->connection('de"), None);
        assert_eq!(context("Redis::connection('default', 'x"), None);
        assert_eq!(context("// Redis::connection('de"), None);
        assert_eq!(context("$h = \"use Redis::connection('de"), None);
        assert_ne!(
            context("Redis::connection('de"),
            Some(Kind::DbConnection)
        );
    }

    #[test]
    fn detects_db_connection_context() {
        let cs = Some(Kind::DbConnection);
        assert_eq!(context("DB::connection('my"), cs);
        // A scalar name only; look-alike receivers, object calls
        // (including Schema::connection's chain), a second argument
        // and comment or string-contained calls stay silent.
        assert_eq!(context("DB::connection(['my"), None);
        assert_eq!(context("MyDB::connection('my"), None);
        assert_eq!(context("$db->connection('my"), None);
        assert_eq!(context("Schema::connection('my"), None);
        assert_eq!(context("DB::connection('mysql', 'x"), None);
        assert_eq!(context("// DB::connection('my"), None);
        assert_eq!(context("$h = \"use DB::connection('my"), None);
    }

    #[test]
    fn detects_auth_guard_context() {
        let cs = Some(Kind::AuthGuard);
        assert_eq!(context("Auth::guard('we"), cs);
        assert_eq!(context("Auth::shouldUse('we"), cs);
        // A scalar name only; look-alike receivers, object calls, a
        // second argument and comment or string-contained calls
        // stay silent.
        assert_eq!(context("Auth::guard(['we"), None);
        assert_eq!(context("MyAuth::guard('we"), None);
        assert_eq!(context("$auth->guard('we"), None);
        assert_eq!(context("Auth::guard('web', 'x"), None);
        assert_eq!(context("// Auth::guard('we"), None);
        assert_eq!(context("$h = \"use Auth::guard('we"), None);
    }

    #[test]
    fn detects_mailer_context() {
        let cs = Some(Kind::MailMailer);
        assert_eq!(context("Mail::mailer('sm"), cs);
        assert_eq!(context("Mail::driver('sm"), cs);
        // A scalar name only; look-alike receivers, object calls, a
        // second argument and comment or string-contained calls
        // stay silent.
        assert_eq!(context("Mail::mailer(['sm"), None);
        assert_eq!(context("MyMail::mailer('sm"), None);
        assert_eq!(context("$mail->mailer('sm"), None);
        assert_eq!(context("Mail::mailer('smtp', 'x"), None);
        assert_eq!(context("// Mail::mailer('sm"), None);
        assert_eq!(context("$h = \"use Mail::mailer('sm"), None);
    }

    #[test]
    fn detects_cache_store_context() {
        let cs = Some(Kind::CacheStore);
        assert_eq!(context("Cache::store('re"), cs);
        assert_eq!(context("Cache::driver('re"), cs);
        // An array argument is an on-demand config, not a name;
        // look-alike receivers, object calls, a second argument and
        // comment or string-contained calls stay silent.
        assert_eq!(context("Cache::store(['dri"), None);
        assert_eq!(context("MyCache::store('re"), None);
        assert_eq!(context("$cache->store('re"), None);
        assert_eq!(context("Cache::store('redis', 'x"), None);
        assert_eq!(context("// Cache::store('re"), None);
        assert_eq!(context("$h = \"use Cache::store('re"), None);
    }

    #[test]
    fn detects_log_channel_context() {
        let ch = Some(Kind::LogChannel);
        assert_eq!(context("Log::channel('sl"), ch);
        assert_eq!(context("Log::driver('sl"), ch);
        assert_eq!(context("Log::stack(['single', 'sl"), ch);
        assert_eq!(context("Log::stack(['sl"), ch);
        // A scalar stack() is not a supported form; look-alike
        // receivers, object calls, a second argument and comment or
        // string-contained calls stay silent.
        assert_eq!(context("Log::stack('sl"), None);
        assert_eq!(context("MyLog::channel('sl"), None);
        assert_eq!(context("$log->channel('sl"), None);
        assert_eq!(context("Log::channel('slack', 'x"), None);
        assert_eq!(context("// Log::channel('sl"), None);
        assert_eq!(context("$h = \"use Log::channel('sl"), None);
    }

    #[test]
    fn detects_vite_entry_context() {
        let ve = Some(Kind::ViteEntry);
        assert_eq!(context("Vite::asset('resources/js/ap"), ve);
        assert_eq!(context("@vite('resources/js/ap"), ve);
        assert_eq!(context("@vite(['resources/css/ap"), ve);
        assert_eq!(context("@vite(['resources/css/a.css', 're"), ve);
        // The bare helper keeps the public-dir list.
        assert_eq!(context("asset('logo.pn"), Some(Kind::Asset));
        // The build-directory argument stays silent (both arms need
        // arg 0).
        assert_eq!(
            context("Vite::asset('resources/js/app.js', 'bu"),
            None
        );
        // The landed bare `asset(...)` arm is head-agnostic, so
        // look-alike, commented and string-contained `...::asset('`
        // lines still get the public-dir list - pre-existing
        // behavior this item does not change; the Vite list claims
        // only the exact `Vite::` receiver.
        assert_eq!(context("MyVite::asset('resources/js/ap"), Some(Kind::Asset));
        assert_eq!(context("// Vite::asset('resources/js/ap"), Some(Kind::Asset));
        assert_eq!(
            context("$h = \"Vite::asset('resources/js/ap"),
            Some(Kind::Asset)
        );
    }

    #[test]
    fn detects_storage_disk_context() {
        let dsk = Some(Kind::StorageDisk);
        assert_eq!(context("Storage::disk('s"), dsk);
        assert_eq!(context("Storage::drive('s"), dsk);
        // build() is not a disk-name position (its string argument
        // is a local root path), and look-alike receivers, object
        // calls, a second argument and comment/string-contained
        // calls stay silent.
        assert_eq!(context("Storage::build('s"), None);
        assert_eq!(context("MyStorage::disk('s"), None);
        assert_eq!(context("$storage->disk('s"), None);
        assert_eq!(context("Storage::disk('s3', 'x"), None);
        assert_eq!(context("// Storage::disk('s"), None);
        assert_eq!(context("$h = \"use Storage::disk('s"), None);
    }

    #[test]
    fn detects_view_facade_context() {
        let vw = Some(Kind::View);
        assert_eq!(context("View::make('pages.ho"), vw);
        assert_eq!(context("View::exists('pages.ho"), vw);
        assert_eq!(context("View::first(['emails.welcome', 'fall"), vw);
        // A scalar first(), a make()/exists() array, a look-alike
        // receiver, the data argument, and comment/string-contained
        // calls stay silent.
        assert_eq!(context("View::first('pages.ho"), None);
        assert_eq!(context("View::make(['pages.ho"), None);
        assert_eq!(context("MyView::make('pages.ho"), None);
        assert_eq!(context("$factory->make('pages.ho"), None);
        assert_eq!(context("View::make('pages.home', ['ti"), None);
        assert_eq!(context("// View::make('pages.ho"), None);
        assert_eq!(context("$h = \"use View::make('pages.ho"), None);
    }

    #[test]
    fn detects_config_facade_context() {
        let cfg = Some(Kind::Config);
        assert_eq!(context("Config::get('app.na"), cfg);
        assert_eq!(context("Config::has('app.de"), cfg);
        assert_eq!(context("Config::set('app.de"), cfg);
        assert_eq!(context("Config::get(['app.a', 'app.b"), cfg);
        assert_eq!(context("Config::set(['app.na"), cfg);
        // Array values are not keys: getMany treats assoc values as
        // defaults and set takes key => value pairs.
        assert_eq!(context("Config::set(['app.name' => 'arbi"), None);
        assert_eq!(context("Config::get(['app.name' => 'de"), None);
        // Nested array data (depth 2+) is value/default data, not
        // keys.
        assert_eq!(
            context("Config::set(['app.options' => ['nes"),
            None
        );
        assert_eq!(
            context("Config::get(['app.key' => ['def"),
            None
        );
        // A look-alike receiver, an object call, a value position
        // and a comment- or string-contained call stay silent.
        assert_eq!(context("MyConfig::get('app.na"), None);
        assert_eq!(context("$config->get('app.na"), None);
        assert_eq!(context("Config::get('app.name', 'de"), None);
        assert_eq!(context("// Config::get('app.na"), None);
        assert_eq!(context("$h = \"use Config::get('app.na"), None);
    }

    #[test]
    fn detects_fluent_rule_context() {
        assert_eq!(context("Rule::exists('us"), Some(Kind::DbTable));
        assert_eq!(context("Rule::unique('us"), Some(Kind::DbTable));
        assert_eq!(
            context("Rule::exists('users', 'em"),
            Some(Kind::DbColumn)
        );
        assert_eq!(
            context("Rule::unique('users', 'em"),
            Some(Kind::DbColumn)
        );
        // A query-builder exists(), a look-alike static receiver, a
        // variable table, a third argument, a comment-contained call
        // and a string-contained call stay silent.
        assert_eq!(context("$q->exists('us"), None);
        assert_eq!(context("MyRule::exists('us"), None);
        assert_eq!(context("Rule::exists($table, 'em"), None);
        assert_eq!(context("Rule::exists('users', 'email', 'x"), None);
        assert_eq!(context("// Rule::exists('us"), None);
        assert_eq!(context("// Rule::exists('users', 'em"), None);
        assert_eq!(context("$hint = \"use Rule::exists('us"), None);
        // A prior call's literal never donates its table to the
        // current call.
        assert_eq!(
            context("Rule::exists('posts', 'a'), Rule::unique($t, 'em"),
            None
        );
    }

    #[test]
    fn detects_middleware_alias_context() {
        let mw = Some(Kind::MiddlewareAlias);
        assert_eq!(context("Route::get('/a', $c)->middleware('au"), mw);
        assert_eq!(context("Route::middleware('au"), mw);
        assert_eq!(context("->withoutMiddleware('ve"), mw);
        assert_eq!(
            context("Route::middleware(['auth', 've"),
            mw
        );
        // Alias parameters, class constants, closed strings and
        // non-middleware receivers stay silent.
        assert_eq!(context("->middleware('throttle:6"), None);
        assert_eq!(context("->middleware(['auth'], 've"), None);
        assert_eq!(context("->middleware(Authenticate::class"), None);
        assert_eq!(context("MyRoute::middleware('au"), None);
        assert_eq!(context("->name('au"), None);
    }

    #[test]
    fn offers_validation_rules() {
        let d = tempfile::tempdir().unwrap();
        let labels = |line: &str| {
            items(d.path(), None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        let got = labels("$this->validate(['name' => 'req|mai");
        assert_eq!(got.len(), crate::validation_rules::RULES.len());
        assert!(got.contains(&"required".to_string()));
        assert!(got.contains(&"email".to_string()));
        assert!(got.contains(&"sometimes".to_string()));
        assert!(labels("$this->validate(['name' => 'max:1").is_empty());
        assert!(labels("$this->validate(['na").is_empty());
    }

    #[test]
    fn offers_livewire_layout_chain() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "resources/views/layouts/app.blade.php", "<html>@yield('content')\n@section('sidebar')\n");
        put(r, "resources/views/livewire/counter.blade.php", "<div></div>\n");
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels("return view('livewire.counter')->layout('lay"),
            ["layouts.app", "livewire.counter"]
        );
        assert_eq!(
            labels("return view('livewire.counter')->section('"),
            ["content", "sidebar"]
        );
        // The bare (non-chain) helper is not a Livewire layout context.
        assert!(labels("layout('lay").is_empty());
    }

    #[test]
    fn offers_database_tables() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "database/migrations/2024_01_01_000000_create_users_table.php",
            "<?php\nSchema::create('users', function (Blueprint $t) {});\nSchema::create('password_resets', function (Blueprint $t) {});\n",
        );
        put(
            r,
            "database/migrations/2024_02_01_000000_create_posts_table.php",
            "<?php\nSchema::create(\"posts\", function (Blueprint $t) {});\n",
        );
        // A `create` outside database/migrations is not harvested.
        put(r, "database/schema/tables.php", "<?php\nSchema::create('ghost', function (Blueprint $t) {});\n");
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels("Schema::table('"),
            ["password_resets", "posts", "users"]
        );
        assert_eq!(
            labels("->constrained('us"),
            ["password_resets", "posts", "users"]
        );
        assert!(labels("Schema::create('").is_empty());
    }

    #[test]
    fn offers_validation_rule_tables() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "database/migrations/2024_01_01_000000_create_users_table.php",
            "<?php\nSchema::create('users', function (Blueprint $t) {});\nSchema::create('password_resets', function (Blueprint $t) {});\n",
        );
        put(
            r,
            "database/migrations/2024_02_01_000000_create_posts_table.php",
            "<?php\nSchema::create(\"posts\", function (Blueprint $t) {});\n",
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        let tables = ["password_resets", "posts", "users"];
        assert_eq!(labels("$this->validate(['email' => 'exists:"), tables);
        assert_eq!(
            labels("$this->validate(['email' => 'required|unique:us"),
            tables
        );
        assert_eq!(labels("#[Validate('exists:u"), tables);
        assert_eq!(labels("Validator::make($data, ['email' => 'unique:"), tables);
    }

    #[test]
    fn offers_validation_rule_columns() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "database/migrations/2024_01_01_000000_create_users_table.php",
            "<?php\nSchema::create('users', function (Blueprint $table) {\n    $table->id();\n    $table->string('name');\n    $table->string('email')->unique();\n    $table->timestamp('email_verified_at')->nullable();\n    $table->rememberToken();\n    $table->timestamps();\n});\n",
        );
        put(
            r,
            "database/migrations/2024_02_01_000000_create_posts_table.php",
            "<?php\nSchema::create(\"posts\", function (Blueprint $table) {\n    $table->id();\n    $table->foreignId('user_id');\n    $table->string('title');\n    $table->morphs('imageable');\n    $table->softDeletes();\n});\n",
        );
        // A later `Schema::table` adds columns to the same table.
        put(
            r,
            "database/migrations/2024_03_01_000000_add_bio_to_users.php",
            "<?php\nSchema::table('users', function (Blueprint $table) {\n    $table->text('bio')->nullable();\n});\n",
        );
        // A `$table->` call after another Schema:: operation is not
        // attributed anywhere: `stray` must not appear for any table.
        put(
            r,
            "database/migrations/2024_04_01_000000_drop_legacy.php",
            "<?php\nSchema::dropIfExists('legacy');\n$table->string('stray');\n",
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        let users = labels("$this->validate(['email' => 'exists:users,");
        assert_eq!(
            users,
            [
                "bio",
                "created_at",
                "email",
                "email_verified_at",
                "id",
                "name",
                "remember_token",
                "updated_at"
            ]
        );
        // `unique:` behaves the same, in every rules-string form.
        assert_eq!(labels("#[Validate('unique:users,"), users);
        assert_eq!(
            labels("Validator::make($data, ['e' => 'exists:users,a|unique:users,"),
            users
        );
        assert_eq!(
            labels("$this->validate(['key' => 'unique:posts,"),
            [
                "deleted_at",
                "id",
                "imageable_id",
                "imageable_type",
                "title",
                "user_id"
            ]
        );
        // The unique idColumn and the condition columns of both rules
        // offer columns; the except and condition values stay silent.
        assert_eq!(labels("$this->validate(['e' => 'unique:users,email,1,"), users);
        assert_eq!(labels("$this->validate(['e' => 'exists:users,email,"), users);
        assert!(labels("$this->validate(['e' => 'unique:users,email,").is_empty());
        assert!(
            labels("$this->validate(['e' => 'exists:users,email,bio,").is_empty()
        );
        // An unknown or empty table offers no columns; the table
        // position still offers tables.
        assert!(labels("$this->validate(['email' => 'exists:nope,").is_empty());
        assert!(labels("$this->validate(['email' => 'exists:,em").is_empty());
        assert!(!labels("$this->validate(['email' => 'exists:us").is_empty());
    }

    #[test]
    fn harvest_ignores_comments_strings_and_stray_calls() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "database/migrations/2024_01_01_000000_create_users_table.php",
            "<?php\nSchema::create('users', function (Blueprint $table) {\n    $table->id();\n    // $table->string('ghost_line');\n    /* $table->string('ghost_block'); */\n    # $table->string('ghost_hash');\n    $table->string('name');\n    $table->string('note')->default('$table->string(\\'ghost_str\\')');\n});\nSchema::table('users', fn (Blueprint $table) => $table->string('arrow_col'));\n$fn = function () { $table->string('stray_closure'); };\n// $table->string('ghost_outside');\n",
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        // Only real closure-body columns: commented-out, in-string and
        // out-of-scope `$table->` calls contribute nothing.
        assert_eq!(
            labels("$this->validate(['e' => 'exists:users,"),
            ["arrow_col", "id", "name", "note"]
        );
    }

    #[test]
    fn harvest_fails_closed_on_heredoc() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "database/migrations/2024_01_01_000000_create_users_table.php",
            "<?php\nSchema::create('users', function (Blueprint $table) {\n    $table->string('name');\n});\n$sql = <<<SQL\n$table->string('phantom');\nSQL;\n",
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        // The file is skipped whole: neither the phantom nor the real
        // column is offered.
        assert!(labels("$this->validate(['e' => 'exists:users,").is_empty());
    }

    #[test]
    fn offers_artisan_commands() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "app/Console/Commands/SendReports.php",
            concat!(
                "<?php\nclass SendReports {\n",
                "    protected $signature = 'reports:send {user}';\n",
                "}\n",
                "// protected $signature = 'reports:legacy';\n",
            ),
        );
        put(
            r,
            "routes/console.php",
            "<?php\nArtisan::command('inspire:me {name}', fn () => 0);\n",
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        // The $signature first word, the closure command name and
        // the built-in commands (item 409): not the signature
        // argument, not the commented-out legacy command.
        let got = labels("Artisan::call('");
        assert!(got.iter().any(|l| l == "reports:send"));
        assert!(got.iter().any(|l| l == "inspire:me"));
        assert!(got.iter().any(|l| l == "migrate"));
        assert!(got.iter().any(|l| l == "queue:work"));
        // Real non-hidden hyphenated command (host respin catch).
        assert!(got.iter().any(|l| l == "clear-compiled"));
        // Hidden commands are not offered.
        assert!(!got.iter().any(|l| l == "invoke-serialized-closure"));
        assert!(!got.iter().any(|l| l == "reports:legacy"));
        assert!(!got.iter().any(|l| l == "{user}"));
        let queued = labels("Artisan::queue('");
        assert!(queued.iter().any(|l| l == "reports:send"));
        assert!(queued.iter().any(|l| l == "migrate"));
        assert!(labels("Artisan::call(['").is_empty());
    }

    #[test]
    fn offers_broadcast_connections() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "config/broadcasting.php",
            concat!(
                "<?php\nreturn [\n",
                "    'default' => env('BROADCAST_CONNECTION'),\n",
                "    'connections' => [\n",
                "        'pusher' => ['driver' => 'pusher'],\n",
                "        'reverb' => ['driver' => 'reverb'],\n",
                "        'log' => ['driver' => 'log'],\n",
                "        // 'ably' => ['driver' => 'ably'],\n",
                "    ],\n",
                "];\n",
            ),
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        // Only the connections array's top-level keys: not the
        // 'default' key, per-connection 'driver' settings one
        // level deeper, or the commented 'ably'.
        assert_eq!(
            labels("Broadcast::connection('"),
            ["log", "pusher", "reverb"]
        );
        assert_eq!(
            labels("Broadcast::driver('"),
            ["log", "pusher", "reverb"]
        );
        assert!(labels("Broadcast::connection(['").is_empty());
    }

    #[test]
    fn offers_queue_connections() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "config/queue.php",
            concat!(
                "<?php\nreturn [\n",
                "    'default' => env('QUEUE_CONNECTION', 'sync'),\n",
                "    'connections' => [\n",
                "        'sync' => ['driver' => 'sync'],\n",
                "        'database' => ['driver' => 'database'],\n",
                "        'redis' => ['driver' => 'redis'],\n",
                "        // 'sqs' => ['driver' => 'sqs'],\n",
                "    ],\n",
                "    'failed' => ['driver' => 'database-uuids'],\n",
                "];\n",
            ),
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        // Only the connections array's top-level keys: not the
        // 'default' key, the sibling 'failed' region, per-connection
        // 'driver' settings one level deeper, or the commented
        // 'sqs'.
        assert_eq!(
            labels("Queue::connection('"),
            ["database", "redis", "sync"]
        );
        assert!(labels("Queue::connection(['").is_empty());
    }

    #[test]
    fn offers_redis_connections() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "config/database.php",
            concat!(
                "<?php\nreturn [\n",
                "    'connections' => [\n",
                "        'mysql' => ['clusters' => ['x' => []]],\n",
                "    ],\n",
                "    'redis' => [\n",
                "        'client' => env('REDIS_CLIENT', 'phpredis'),\n",
                "        'options' => [\n",
                "            'cluster' => env('REDIS_CLUSTER'),\n",
                "        ],\n",
                "        'default' => ['host' => '127.0.0.1'],\n",
                "        'cache' => ['host' => '127.0.0.1'],\n",
                "        // 'sessions' => ['host' => '127.0.0.1'],\n",
                "        'clusters' => [\n",
                "            'options' => ['prefix' => 'x'],\n",
                "            'nodes' => [['host' => '10.0.0.1']],\n",
                "        ],\n",
                "    ],\n",
                "];\n",
            ),
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        // Region-verified 12.x shape: direct connections 'default'
        // and 'cache' plus the 'nodes' cluster, excluding the
        // 'client' scalar, both 'options' metadata regions, the
        // 'clusters' key itself, the commented 'sessions', and the
        // unrelated 'clusters' key inside the mysql connection.
        assert_eq!(labels("Redis::connection('"), ["cache", "default", "nodes"]);
        assert!(labels("Redis::connection(['").is_empty());
    }

    #[test]
    fn offers_db_connections() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "config/database.php",
            concat!(
                "<?php\nreturn [\n",
                "    'default' => env('DB_CONNECTION', 'sqlite'),\n",
                "    'connections' => [\n",
                "        'sqlite' => ['driver' => 'sqlite'],\n",
                "        'mysql' => ['driver' => 'mysql'],\n",
                "        // 'pgsql' => ['driver' => 'pgsql'],\n",
                "    ],\n",
                "    'redis' => ['client' => 'phpredis'],\n",
                "];\n",
            ),
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        // Only the connections array's top-level keys: not the
        // 'default' key, the per-connection 'driver' settings one
        // level deeper, the sibling 'redis' region, or the
        // commented 'pgsql'.
        assert_eq!(labels("DB::connection('"), ["mysql", "sqlite"]);
        assert!(labels("DB::connection(['").is_empty());
        assert!(labels("Schema::connection('").is_empty());
    }

    #[test]
    fn offers_auth_guards() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "config/auth.php",
            concat!(
                "<?php\nreturn [\n",
                "    'defaults' => ['guard' => 'web'],\n",
                "    'guards' => [\n",
                "        'web' => ['driver' => 'session'],\n",
                "        'api' => ['driver' => 'token'],\n",
                "        // 'admin' => ['driver' => 'session'],\n",
                "    ],\n",
                "];\n",
            ),
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        // Only the guards array's top-level keys: not the nested
        // 'defaults' -> 'guard' key, the per-guard 'driver'
        // settings one level deeper, or the commented 'admin'.
        assert_eq!(labels("Auth::guard('"), ["api", "web"]);
        assert_eq!(labels("Auth::shouldUse('"), ["api", "web"]);
        assert!(labels("Auth::guard(['").is_empty());
    }

    #[test]
    fn offers_mail_mailers() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "config/mail.php",
            concat!(
                "<?php\nreturn [\n",
                "    'default' => env('MAIL_MAILER', 'log'),\n",
                "    'mailers' => [\n",
                "        'smtp' => ['transport' => 'smtp'],\n",
                "        'log' => ['transport' => 'log'],\n",
                "        // 'ses' => ['transport' => 'ses'],\n",
                "    ],\n",
                "];\n",
            ),
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        // Only the mailers array's top-level keys: not the
        // 'default' key, the per-mailer 'transport' settings one
        // level deeper, or the commented-out 'ses'.
        assert_eq!(labels("Mail::mailer('"), ["log", "smtp"]);
        assert_eq!(labels("Mail::driver('"), ["log", "smtp"]);
        assert!(labels("Mail::mailer(['").is_empty());
    }

    #[test]
    fn offers_cache_stores() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "config/cache.php",
            concat!(
                "<?php\nreturn [\n",
                "    'default' => env('CACHE_STORE', 'file'),\n",
                "    'stores' => [\n",
                "        'file' => ['driver' => 'file'],\n",
                "        'redis' => ['driver' => 'redis'],\n",
                "        // 'memcached' => ['driver' => 'memcached'],\n",
                "    ],\n",
                "];\n",
            ),
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        // Only the stores array's top-level keys: not the 'default'
        // key, the per-store 'driver' settings one level deeper, or
        // the commented-out 'memcached'.
        assert_eq!(labels("Cache::store('"), ["file", "redis"]);
        assert_eq!(labels("Cache::driver('"), ["file", "redis"]);
        assert!(labels("Cache::store(['").is_empty());
    }

    #[test]
    fn offers_log_channels() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "config/logging.php",
            concat!(
                "<?php\nreturn [\n",
                "    'default' => env('LOG_CHANNEL', 'stack'),\n",
                "    'channels' => [\n",
                "        'single' => ['driver' => 'single'],\n",
                "        'slack' => ['driver' => 'slack'],\n",
                "        // 'flare' => ['driver' => 'flare'],\n",
                "    ],\n",
                "];\n",
            ),
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        // Only the channels array's top-level keys: not the
        // 'default' key, the per-channel 'driver' settings one
        // level deeper, or the commented-out 'flare'.
        assert_eq!(labels("Log::channel('"), ["single", "slack"]);
        assert_eq!(labels("Log::stack(['"), ["single", "slack"]);
        assert!(labels("Log::stack('").is_empty());
    }

    #[test]
    fn offers_vite_entries() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "resources/js/app.js", "import './bootstrap';\n");
        put(r, "resources/js/Pages/Home.vue", "<template/>\n");
        put(r, "resources/css/app.css", "@tailwind base;\n");
        put(r, "public/logo.png", "png");
        let blade = std::path::Path::new("probe.blade.php");
        let php = std::path::Path::new("probe.php");
        let labels = |file: &std::path::Path, line: &str| {
            items(r, Some(file), line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        let entries = [
            "resources/css/app.css",
            "resources/js/Pages/Home.vue",
            "resources/js/app.js",
        ];
        // Source files only - the public-dir asset stays with
        // `asset()`.
        assert_eq!(labels(php, "Vite::asset('"), entries);
        assert_eq!(labels(blade, "@vite('"), entries);
        assert_eq!(
            labels(blade, "@vite(['resources/css/app.css', '"),
            entries
        );
        assert_eq!(labels(php, "asset('"), ["logo.png"]);
        assert!(
            labels(php, "Vite::asset('resources/js/app.js', '").is_empty()
        );
        // The directive is Blade-file-only: in a plain PHP file
        // `@vite(...)` is the error-suppression operator, and an
        // unknown file type fails closed. `Vite::asset` is plain
        // PHP and ungated.
        assert!(labels(php, "@vite('").is_empty());
        assert!(items(r, None, "@vite('").is_empty());
        assert_eq!(
            items(r, None, "Vite::asset('")
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>(),
            entries
        );
    }

    #[test]
    fn offers_storage_disks() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "config/filesystems.php",
            concat!(
                "<?php\nreturn [\n",
                "    'default' => env('FILESYSTEM_DISK', 'local'),\n",
                "    'disks' => [\n",
                "        'local' => ['driver' => 'local'],\n",
                "        's3' => ['driver' => 's3', 'bucket' => 'b'],\n",
                "        // 'backup' => ['driver' => 'local'],\n",
                "    ],\n",
                "];\n",
            ),
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        // Only the disks array's top-level keys: not the 'default'
        // key, the per-disk 'driver'/'bucket' settings one level
        // deeper, or the commented-out 'backup'.
        assert_eq!(labels("Storage::disk('"), ["local", "s3"]);
        assert_eq!(labels("Storage::drive('"), ["local", "s3"]);
        assert!(labels("Storage::build('").is_empty());
    }

    #[test]
    fn offers_view_facade_names() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "resources/views/pages/home.blade.php", "<h1>hi</h1>\n");
        put(r, "resources/views/emails/welcome.php", "<p>w</p>\n");
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(labels("View::make('"), ["emails.welcome", "pages.home"]);
        assert_eq!(labels("View::exists('"), ["emails.welcome", "pages.home"]);
        assert_eq!(
            labels("View::first(['pages.home', '"),
            ["emails.welcome", "pages.home"]
        );
        assert!(labels("View::first('").is_empty());
        assert!(labels("View::make('pages.home', ['").is_empty());
    }

    #[test]
    fn offers_config_facade_keys() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "config/app.php",
            "<?php\nreturn [\n    'name' => 'Acme',\n    'debug' => true,\n];\n",
        );
        // Config keys come in fixture file order (the helper
        // completion is unsorted), so compare as sorted sets.
        let labels = |line: &str| {
            let mut v: Vec<_> =
                items(r, None, line).into_iter().map(|i| i.label).collect();
            v.sort();
            v
        };
        assert_eq!(labels("Config::get('"), ["app.debug", "app.name"]);
        assert_eq!(labels("Config::set('"), ["app.debug", "app.name"]);
        assert_eq!(labels("Config::set(['"), ["app.debug", "app.name"]);
        // Array values (set values, getMany defaults) stay silent.
        assert!(labels("Config::set(['app.name' => '").is_empty());
        assert!(labels("Config::get(['app.name' => '").is_empty());
        // Nested array data is not keys.
        assert!(labels("Config::set(['app.options' => ['").is_empty());
        assert!(labels("Config::get(['app.key' => ['").is_empty());
        assert!(labels("Config::get('app.name', '").is_empty());
    }

    #[test]
    fn offers_fluent_rule_tables_and_columns() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "database/migrations/2024_01_01_000000_create_users_table.php",
            "<?php\nSchema::create('users', function (Blueprint $table) {\n    $table->id();\n    $table->string('name');\n    $table->string('email');\n});\n",
        );
        put(
            r,
            "database/migrations/2024_02_01_000000_create_posts_table.php",
            "<?php\nSchema::create('posts', function (Blueprint $table) {\n    $table->id();\n    $table->string('title');\n});\n",
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(labels("Rule::exists('"), ["posts", "users"]);
        assert_eq!(labels("Rule::unique('"), ["posts", "users"]);
        assert_eq!(
            labels("Rule::exists('users', '"),
            ["email", "id", "name"]
        );
        assert_eq!(labels("Rule::unique('posts', '"), ["id", "title"]);
        // A variable table fails closed, and a prior call's literal
        // never donates its table.
        assert!(labels("Rule::exists($table, '").is_empty());
        assert!(labels(
            "Rule::exists('posts', 'a'), Rule::unique($t, '"
        )
        .is_empty());
        assert!(labels("// Rule::exists('users', '").is_empty());
    }

    #[test]
    fn offers_middleware_aliases() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "bootstrap/app.php",
            concat!(
                "<?php\nreturn Application::configure()\n",
                "    ->withMiddleware(function ($m) {\n",
                "        $m->alias(['admin' => AdminOnly::class]);\n",
                "        $m->alias($shared);\n",
                "    })\n",
                "    ->withExceptions(function ($e) {\n",
                "        $e->render(fn () => ['message' => 'nope']);\n",
                "    });\n",
                "$x = \"->alias(['fake' => Y::class]);\";\n",
            ),
        );
        put(
            r,
            "app/Http/Kernel.php",
            concat!(
                "<?php\nclass Kernel {\n",
                "    protected $middlewareGroups = [\n",
                "        'web' => [],\n",
                "        'api' => ['inner' => \\App\\X::class],\n",
                "    ];\n",
                "    protected $middlewareAliases = [\n",
                "        'auth' => \\App\\Http\\Middleware\\Authenticate::class,\n",
                "        // 'ghost' => Ghost::class,\n",
                "    ];\n",
                "    protected $custom = ['driver' => 'redis'];\n",
                "}\n",
            ),
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        let all = ["admin", "api", "auth", "web"];
        assert_eq!(labels("Route::get('/a', $c)->middleware('"), all);
        assert_eq!(labels("Route::middleware('"), all);
        // The exact-equality asserts above are the scope negatives:
        // only declaration arrays are harvested, so the unrelated
        // 'message' => render array, the in-string "->alias([" text,
        // the ->alias($shared) non-array call, the commented-out
        // 'ghost', the nested group key 'inner' and the unrelated
        // $custom property key 'driver' invent nothing.
        assert!(labels("->middleware('throttle:6").is_empty());
        assert!(labels("->middleware(Authenticate::class").is_empty());
    }

    #[test]
    fn offers_query_columns() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "database/migrations/2024_01_01_000000_create_users_table.php",
            "<?php\nSchema::create('users', function (Blueprint $table) {\n    $table->id();\n    $table->string('name');\n    $table->string('email');\n});\n",
        );
        put(
            r,
            "database/migrations/2024_02_01_000000_create_posts_table.php",
            "<?php\nSchema::create('posts', function (Blueprint $table) {\n    $table->id();\n    $table->string('title');\n});\n",
        );
        let labels = |line: &str| {
            items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels("DB::table('users')->where('"),
            ["email", "id", "name"]
        );
        assert_eq!(labels("DB::table('posts')->orderBy('"), ["id", "title"]);
        // Inside a nested chain the inner base decides.
        assert_eq!(
            labels("DB::table('users')->whereIn('id', DB::table('posts')->select('"),
            ["id", "title"]
        );
        // After the nested subquery closes the outer base decides.
        let closed = concat!(
            "DB::table('users')->whereIn('id', DB::table('posts')",
            "->select('u'))->where('"
        );
        assert_eq!(labels(closed), ["email", "id", "name"]);
        // No chain base, an unrelated receiver across a statement
        // boundary, and the value position stay silent.
        assert!(labels("User::where('").is_empty());
        assert!(labels(
            "$x = DB::table('users')->count(); $q->where('"
        )
        .is_empty());
        assert!(labels("DB::table('users')->where('email', '").is_empty());
    }

    #[test]
    fn offers_blueprint_methods() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "database/migrations/2024_01_01_000000_create_users_table.php",
            "<?php\n",
        );
        let migration = r.join("database/migrations/2024_01_01_000000_create_users_table.php");
        let other = r.join("app/Http/Controllers/ReportController.php");
        let labels = |line: &str| {
            items(r, Some(migration.as_path()), line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        let got = labels("$table->");
        assert_eq!(got.len(), BLUEPRINT_METHODS.len());
        assert!(got.contains(&"string".to_string()));
        assert!(got.contains(&"foreignId".to_string()));
        assert!(got.contains(&"timestamps".to_string()));
        assert!(got.contains(&"nullableTimestamps".to_string()));
        assert!(got.contains(&"primary".to_string()));
        assert!(got.contains(&"dropColumn".to_string()));
        assert!(got.contains(&"renameColumn".to_string()));
        assert_eq!(labels("$table->str"), got);
        assert!(labels("$table->string('na").is_empty());
        assert!(labels("$this->table->str").is_empty());
        // `$table->` is a convention, not a type: outside
        // database/migrations the same text offers nothing.
        assert!(items(r, Some(other.as_path()), "$table->str").is_empty());
        assert!(items(r, None, "$table->str").is_empty());
    }

    #[test]
    fn detects_livewire_event_context() {
        assert_eq!(context("$dispatch('"), Some(Kind::LivewireEvent));
        assert_eq!(context("$this->dispatch('"), Some(Kind::LivewireEvent));
        assert_eq!(context("Livewire.dispatch(\""), Some(Kind::LivewireEvent));
        assert_eq!(context("$wire.emit('"), Some(Kind::LivewireEvent));
        assert_eq!(context("$dispatch('saved')"), None);
        assert_eq!(context("mydispatch('"), None);
    }

    #[test]
    fn dispatch_call_wins_inside_wire_value() {
        assert_eq!(
            context("wire:click=\"$dispatch('"),
            Some(Kind::LivewireEvent)
        );
        assert_eq!(
            context("wire:click=\"$dispatch('count.ch"),
            Some(Kind::LivewireEvent)
        );
        // No call in the value: the attribute context still applies.
        assert_eq!(context("wire:model=\"form.ti"), Some(Kind::LivewireModel));
        assert_eq!(context("wire:click=\"save"), Some(Kind::LivewireAction));
        // An unknown call word keeps the attribute context too.
        assert_eq!(
            context("wire:click=\"confirm('"),
            Some(Kind::LivewireAction)
        );
    }

    #[test]
    fn offers_livewire_events() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "app/Livewire/Counter.php",
            "<?php\n#[On('post-created')]\npublic function refresh() {}\n$this->dispatch('count.changed');\n",
        );
        put(
            r,
            "resources/views/c.blade.php",
            "<button x-on:click=\"$dispatch('saved')\">x</button>\n",
        );
        let got = labels(items(r, None, "$dispatch('"));
        assert!(got.contains(&"post-created".to_string()), "{got:?}");
        assert!(got.contains(&"count.changed".to_string()), "{got:?}");
        assert!(got.contains(&"saved".to_string()), "{got:?}");
    }

    #[test]
    fn detects_alpine_event_context() {
        assert_eq!(context("<button x-on:"), Some(Kind::AlpineEvent));
        assert_eq!(context("<button x-on:cl"), Some(Kind::AlpineEvent));
        assert_eq!(context("<button @"), Some(Kind::AlpineEvent));
        assert_eq!(context("<button @key"), Some(Kind::AlpineEvent));
        // A Blade directive outside a tag is not an Alpine event, and an
        // `@` in text content after a closed tag is not in a tag either.
        assert_eq!(context("@foreach"), None);
        assert_eq!(context("</div> @cl"), None);
        assert_eq!(context("<div>@key"), None);
        // Attribute value and modifier contexts are unchanged.
        assert_eq!(context("<div x-on:click."), Some(Kind::AlpineModifier));
        assert_eq!(context("<div x-on:click=\"$foo"), None);
    }

    #[test]
    fn offers_alpine_events() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let got = labels(items(r, None, "<button x-on:"));
        assert!(got.contains(&"click".to_string()), "{got:?}");
        assert!(got.contains(&"submit".to_string()), "{got:?}");
        assert!(got.contains(&"keydown".to_string()), "{got:?}");
    }

    #[test]
    fn detects_livewire_attribute_context() {
        assert_eq!(context("<input wire:"), Some(Kind::LivewireAttribute));
        assert_eq!(context("<input wire:mo"), Some(Kind::LivewireAttribute));
        // Outside a tag: no.
        assert_eq!(context("wire:mo"), None);
        // Modifier and value contexts are unchanged.
        assert_eq!(
            context("<input wire:model."),
            Some(Kind::LivewireModifier)
        );
        assert_eq!(
            context("<input wire:model=\"form.ti"),
            Some(Kind::LivewireModel)
        );
    }

    #[test]
    fn offers_livewire_attributes() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let got = labels(items(r, None, "<input wire:"));
        for n in ["click", "model", "loading", "navigate", "poll"] {
            assert!(got.contains(&n.to_string()), "{n} {got:?}");
        }
        // Each name once, and labels carry no `wire:` prefix.
        assert_eq!(got.iter().filter(|n| n.as_str() == "click").count(), 1);
        assert!(got.iter().all(|n| !n.starts_with("wire:")), "{got:?}");
    }

    #[test]
    fn offers_livewire_properties() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "resources/views/livewire/admin/user-profile.blade.php", "<input wire:model=\"email\">\n");
        put(r, "app/Livewire/Admin/UserProfile.php", "<?php\nclass UserProfile extends \\Livewire\\Component {\n    public $email = '';\n    public ?string $name;\n    public array $items = [];\n    public static $shared;\n    protected $secret;\n    public function save() {}\n}\n");
        put(r, "resources/views/livewire/counter.blade.php", "<input wire:model=\"count\">\n");
        put(r, "app/Http/Livewire/Counter.php", "<?php\nclass Counter extends \\Livewire\\Component {\n    public $count = 0;\n}\n");
        let labels = |view: &Path, line: &str| {
            items(r, Some(view), line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        let v3 = r.join("resources/views/livewire/admin/user-profile.blade.php");
        let got = labels(&v3, "<input wire:model=\"");
        assert!(got.contains(&"email".to_string()));
        assert!(got.contains(&"name".to_string()));
        assert!(got.contains(&"items".to_string()));
        assert!(!got.contains(&"shared".to_string()));
        assert!(!got.contains(&"secret".to_string()));
        assert!(!got.contains(&"save".to_string()));
        // Livewire v2 layout.
        let v2 = r.join("resources/views/livewire/counter.blade.php");
        assert_eq!(labels(&v2, "<input wire:model=\"co"), ["count"]);
        // A view outside resources/views/livewire has no paired class.
        put(r, "resources/views/pages/home.blade.php", "<input>\n");
        let plain = r.join("resources/views/pages/home.blade.php");
        assert!(labels(&plain, "<input wire:model=\"").is_empty());
        // Without a file there is nothing to pair.
        assert!(items(r, None, "<input wire:model=\"").is_empty());
    }

    #[test]
    fn offers_livewire_methods() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "resources/views/livewire/user-profile.blade.php", "<button wire:click=\"save\">\n");
        put(r, "app/Livewire/UserProfile.php", "<?php\nclass UserProfile extends \\Livewire\\Component {\n    public $email = '';\n    public function save() {}\n    public function delete() {}\n    private function helper() {}\n    protected function boot() {}\n    public static function make() {}\n}\n");
        let view = r.join("resources/views/livewire/user-profile.blade.php");
        let labels = |line: &str| {
            items(r, Some(view.as_path()), line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        let methods = labels("<button wire:click=\"");
        assert!(methods.contains(&"save".to_string()));
        assert!(methods.contains(&"delete".to_string()));
        // Private, protected and static members are excluded.
        assert!(!methods.contains(&"helper".to_string()));
        assert!(!methods.contains(&"boot".to_string()));
        assert!(!methods.contains(&"make".to_string()));
        // Properties are not methods and methods are not properties.
        assert!(!methods.contains(&"email".to_string()));
        let props = labels("<input wire:model=\"");
        assert!(props.contains(&"email".to_string()));
        assert!(!props.contains(&"save".to_string()));
    }

    #[test]
    fn offers_livewire_targets() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "resources/views/livewire/user-profile.blade.php", "<div></div>\n");
        put(r, "app/Livewire/UserProfile.php", "<?php\nclass UserProfile extends \\Livewire\\Component {\n    public $username = '';\n    public function save() {}\n    public function remove() {}\n}\n");
        let view = r.join("resources/views/livewire/user-profile.blade.php");
        let labels = |line: &str| {
            items(r, Some(view.as_path()), line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        // Actions and properties both complete; no other members.
        let got = labels("<div wire:loading wire:target=\"");
        assert!(got.contains(&"save".to_string()));
        assert!(got.contains(&"remove".to_string()));
        assert!(got.contains(&"username".to_string()));
        assert_eq!(got.len(), 3);
        // Comma-separated continuation keeps the context.
        assert_eq!(labels("<div wire:target=\"save, u"), got);
        // A view with no paired class offers nothing.
        assert!(items(r, None, "<div wire:target=\"").is_empty());
    }
    fn labels(items: Vec<CompletionItem>) -> Vec<String> {
        items.into_iter().map(|i| i.label).collect()
    }

    const BLADE_VIEW: &str = r#"@props(['disabled' => false, 'type' => 'submit', 'flat'])
@inject('metrics', 'App\Services\Metrics')
@php
    $title = 'Hello';
    $count = $metrics->total();
    $bad == $title;
    $pair = ['k' => 1];
@endphp
@foreach($items as $idx => $item)
    <li>{{ $item->name }} {{ $idx }}</li>
@endforeach
@forelse($users as $user)
    <p>{{ $user->email }}</p>
@endforelse
{{-- $ghost = 1; @inject('sneaky', 'X') --}}
@if($flag)
    {{ $x }}
@endif
"#;

    #[test]
    fn offers_gate_abilities() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "app/Policies/PostPolicy.php", "<?php
final class PostPolicy {
    public function before() {}
    public function viewAny(User $user) {}
    public function update(User $user, Post $post) {}
    protected function hidden() {}
}
");
        put(r, "app/Providers/AuthServiceProvider.php", "<?php
Gate::define('sell-vinyl', [ShopGate::class, 'sell']);
Gate::define(\"buy-cd\", 'shop.buy');
");
        let mut labels = items(r, None, "@can('")
            .into_iter()
            .map(|i| i.label)
            .collect::<Vec<_>>();
        labels.sort();
        assert_eq!(
            labels,
            [
                "buy-cd".to_string(),
                "sell-vinyl".to_string(),
                "update".to_string(),
                "viewAny".to_string(),
            ]
        );
        // The whole family routes to the same kind.
        for line in [
            "@cannot('",
            "@canany(['",
            "$user->can('",
            "$user?->cannot('",
            "$this->authorize('",
            "Gate::allows('",
            "Gate::denies('",
            "Gate::check('",
        ] {
            assert_eq!(context(line), Some(Kind::Gate), "{line}");
        }
        // Bare `can(` is not a Gate context.
        assert_eq!(context("can('"), None);
        assert_eq!(context("Str::of('"), None);
    }

    #[test]
    fn offers_asset_paths() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "public/css/app.css", "body {}");
        put(r, "public/js/app.js", "");
        put(r, "public/images/logo dark.png", "");
        put(r, "public/.hidden/x.css", "");
        // Missing public/ -> no asset items, other kinds unaffected.
        let labels = |line: &str| {
            let mut v = items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>();
            v.sort();
            v
        };
        assert_eq!(
            labels("asset('"),
            [
                "css/app.css".to_string(),
                "images/logo dark.png".to_string(),
                "js/app.js".to_string(),
            ]
        );
        assert_eq!(labels("secure_asset('"), labels("asset('"));
        assert_eq!(labels("mix('"), labels("asset('"));
        assert!(!labels("asset('").iter().any(|l| l.starts_with(".hidden")));
        let e = tempfile::tempdir().unwrap();
        assert!(items(e.path(), None, "asset('").is_empty());
    }

    #[test]
    fn component_tag_context() {
        assert_eq!(context("<x-"), Some(Kind::ComponentTag));
        assert_eq!(context("<x-al"), Some(Kind::ComponentTag));
        assert_eq!(context(" <x-forms.in"), Some(Kind::ComponentTag));
        assert_eq!(context("route(\"x\") <x-btn"), Some(Kind::ComponentTag));
        // Multibyte chars before the tag must not break the tail scan.
        assert_eq!(context("\u{e9}<x-fo"), Some(Kind::ComponentTag));
        assert_eq!(context("\u{65e5}<x-"), Some(Kind::ComponentTag));
        // Inside an attribute value, after a name char, slots, closed
        // tag name, CSS class values, bare `<x`.
        assert_eq!(context("<x-alert type=\"x-"), None);
        assert_eq!(context("a<x-b"), None);
        assert_eq!(context("x_<x-a"), None);
        assert_eq!(context("<x-slot"), None);
        assert_eq!(context("<x-slot:na"), None);
        // Since item 375 the attribute position after the tag name is
        // the prop-name context (previously no context fired here).
        assert_eq!(context("<x-alert "), Some(Kind::BladeProp));
        assert_eq!(context("<div class=\"x-foo"), None);
        assert_eq!(context("<x"), None);
    }

    #[test]
    fn dotted_name_context_and_word_len() {
        assert!(dotted_name_context("route('admin.us"));
        assert!(dotted_name_context("config('app."));
        assert!(dotted_name_context("view('pages.ho"));
        assert!(dotted_name_context("  @include('partials.he"));
        assert!(dotted_name_context("@extends('layouts.ap"));
        assert!(dotted_name_context("__('messages.we"));
        assert!(dotted_name_context("$user->can('edit-po"));
        assert!(dotted_name_context("@livewire('admin.us"));
        assert!(!dotted_name_context("document.getE"));
        assert!(!dotted_name_context("$this->foo->b"));
        assert!(!dotted_name_context("env('APP_"));
        assert_eq!(dotted_word_len("route('admin."), 6);
        assert_eq!(dotted_word_len("config('app."), 4);
        assert_eq!(dotted_word_len("__('messages.welcome-"), 17);
        assert_eq!(dotted_word_len("route('"), 0);
        assert_eq!(dotted_word_len("'"), 0);
        assert_eq!(dotted_word_len(""), 0);
    }

    #[test]
    fn context_detects_the_bare_quote() {
        // The popup opens on these (empty input, no letter typed yet).
        assert_eq!(context("route('"), Some(Kind::Route));
        assert_eq!(context("route(\""), Some(Kind::Route));
        assert_eq!(context("config('"), Some(Kind::Config));
        assert_eq!(context("view('"), Some(Kind::View));
        assert_eq!(context("  @include('"), Some(Kind::View));
        assert_eq!(context("@livewire('"), Some(Kind::LivewireName));
        assert_eq!(context("__('"), Some(Kind::Lang));
        assert_eq!(context("<input wire:model=\""), Some(Kind::LivewireModel));
        assert_eq!(context("<button wire:click=\""), Some(Kind::LivewireAction));
        // Closed string / unknown caller / second argument: nothing.
        assert_eq!(context("route('x')"), None);
        assert_eq!(context("route('x', ["), None);
        assert_eq!(context("console.log('"), None);
    }

    #[test]
    fn call_context_stays_in_the_name_argument() {
        // A quoted string in a LATER argument is not a name position:
        // `$entangle('count', '...` is the live flag, `route('x', '...`
        // the parameters, `$dispatch('x', '...` the payload.
        assert_eq!(context("$entangle('count', '"), None);
        assert_eq!(context("<button @click=\"$entangle('count', '"), None);
        assert_eq!(context("route('home', '"), None);
        assert_eq!(context("$dispatch('post', '"), None);
        assert_eq!(context("__('messages.w', '"), None);
        assert_eq!(context("@include('a', '"), None);
        assert_eq!(context("view('a', '"), None);
        // Nested calls keep their own argument counting.
        assert_eq!(context("route('a', foo('"), None);
        // `@each`'s third argument is the item variable, not a view.
        assert_eq!(context("@each('row', $rows, 'it"), None);
        // First-argument behavior is preserved.
        assert_eq!(context("$entangle('co"), Some(Kind::LivewireEntangle));
        assert_eq!(context("route('"), Some(Kind::Route));
        assert_eq!(context("config('app."), Some(Kind::Config));
        assert_eq!(context("$dispatch('"), Some(Kind::LivewireEvent));
        assert_eq!(context("@include('partials."), Some(Kind::View));
        // `@includeWhen($cond, 'view')` takes the view SECOND, and the
        // `@includeFirst(['a', 'b'])` array stays argument zero.
        assert_eq!(context("@includeWhen($ok, 'par"), Some(Kind::View));
        assert_eq!(context("@includeWhen('v"), None);
        assert_eq!(context("@includeWhen($ok, 'v', '"), None);
        assert_eq!(context("@includeFirst(['one', 'tw"), Some(Kind::View));
        assert_eq!(context("@includeFirst(['one', 'two'], '"), None);
    }

    #[test]
    fn offers_alpine_modifiers() {
        assert_eq!(context("<button x-on:click."), Some(Kind::AlpineModifier));
        assert_eq!(context("<button @click."), Some(Kind::AlpineModifier));
        assert_eq!(context("<input @keydown.en"), Some(Kind::AlpineModifier));
        // Blade directives are not Alpine events.
        assert_eq!(context("@foreach ("), None);
        assert_eq!(context("<div x-data=\"{ o: false }\""), None);
        let labels = alpine_modifiers_for("<button @click.")
            .into_iter()
            .map(|i| i.label)
            .collect::<Vec<_>>();
        for want in ["prevent", "once", "outside", "debounce.250ms"] {
            assert!(labels.contains(&want.to_string()), "{want}");
        }
        let labels = alpine_modifiers_for("<input x-on:keydown.")
            .into_iter()
            .map(|i| i.label)
            .collect::<Vec<_>>();
        assert!(labels.contains(&"enter".to_string()));
        assert!(!labels.contains(&"outside".to_string()));
        // An attribute value or a non-event name offers nothing.
        assert!(alpine_modifiers_for("<button @click=\"open = true").is_empty());
        assert!(alpine_modifiers_for("<div @foreach.").is_empty());
    }

    #[test]
    fn offers_livewire_modifiers() {
        assert_eq!(context("<input wire:model."), Some(Kind::LivewireModifier));
        assert_eq!(context("<input wire:model.li"), Some(Kind::LivewireModifier));
        assert_eq!(context("<input wire:keydown.en"), Some(Kind::LivewireModifier));
        // The attribute value context wins once `=` is typed.
        assert_eq!(context("<input wire:model=\""), Some(Kind::LivewireModel));
        let labels = wire_modifiers_for("<input wire:model.")
            .into_iter()
            .map(|i| i.label)
            .collect::<Vec<_>>();
        for want in ["live", "blur", "debounce.150ms", "throttle.500ms"] {
            assert!(labels.contains(&want.to_string()), "{want}");
        }
        let labels = wire_modifiers_for("<input wire:keydown.")
            .into_iter()
            .map(|i| i.label)
            .collect::<Vec<_>>();
        assert!(labels.contains(&"enter".to_string()));
        assert!(labels.contains(&"arrow-up".to_string()));
        let labels = wire_modifiers_for("<form wire:submit.")
            .into_iter()
            .map(|i| i.label)
            .collect::<Vec<_>>();
        assert_eq!(labels, ["prevent", "stop"]);
        // Events without modifiers offer nothing.
        assert!(wire_modifiers_for("<div wire:poll.").is_empty());
    }

    #[test]
    fn offers_livewire_directive_modifiers() {
        assert_eq!(context("<div wire:loading."), Some(Kind::LivewireModifier));
        assert_eq!(context("<div wire:loading.cl"), Some(Kind::LivewireModifier));
        assert_eq!(context("<span wire:dirty."), Some(Kind::LivewireModifier));
        assert_eq!(context("<a wire:navigate.h"), Some(Kind::LivewireModifier));
        assert_eq!(context("<div wire:ignore."), Some(Kind::LivewireModifier));
        let labels = wire_modifiers_for("<div wire:loading.")
            .into_iter()
            .map(|i| i.label)
            .collect::<Vec<_>>();
        for want in ["class", "class.remove", "attr", "remove", "delay", "delay.longest"] {
            assert!(labels.contains(&want.to_string()), "{want}");
        }
        let labels = wire_modifiers_for("<div wire:dirty.")
            .into_iter()
            .map(|i| i.label)
            .collect::<Vec<_>>();
        assert!(labels.contains(&"class.remove".to_string()));
        assert!(!labels.iter().any(|l| l.starts_with("delay")));
        // Loading is not an action event: its value completes nothing.
        assert_eq!(context("<div wire:loading=\""), None);
    }

    #[test]
    fn parses_x_data_keys() {
        let keys = |v: &str| parse_data_keys(v);
        assert_eq!(keys("{ open: false, count: 0 }"), ["open", "count"]);
        // Nested objects and commas inside strings do not split.
        assert_eq!(keys("{ user: { name: 'a,b' }, ok: true }"), ["user", "ok"]);
        // Methods, function values and arrow functions.
        assert_eq!(
            keys("{ open: false, toggle() { this.open = !this.open } }"),
            ["open", "toggle"]
        );
        assert_eq!(keys("{ cb: function() { return 1; }, x: 2 }"), ["cb", "x"]);
        assert_eq!(keys("{ a: () => 1, b: 2 }"), ["a", "b"]);
        // Quoted keys, shorthand, spread, ternary values.
        assert_eq!(keys("{ 'foo-bar': 1, open: true }"), ["foo-bar", "open"]);
        assert_eq!(keys("{ ...base, open }"), ["open"]);
        assert_eq!(keys("{ open: cond ? 'a' : 'b', x: 1 }"), ["open", "x"]);
        // A call or a truncated object is not an object literal.
        assert!(keys("dropdown()").is_empty());
        assert_eq!(keys("{ open: false, second: 1"), ["open", "second"]);
    }

    #[test]
    fn finds_x_data_on_the_innermost_open_tag() {
        // Parent tag provides the properties for child expressions.
        assert_eq!(
            x_data_keys_at(r#"<div x-data="{ open: false }"><span x-show=""#),
            ["open"]
        );
        // Same tag.
        assert_eq!(
            x_data_keys_at(r#"<div x-data="{ open: false }" x-show=""#),
            ["open"]
        );
        // A `>` inside a quoted value does not close the tag early.
        assert_eq!(
            x_data_keys_at(r#"<div x-data="{ open: count > 0 }"><span x-show=""#),
            ["open"]
        );
        // Closed scopes no longer apply.
        assert!(
            x_data_keys_at(r#"<div x-data="{ open: false }"></div><p x-text=""#)
                .is_empty()
        );
        // The innermost x-data wins.
        assert_eq!(
            x_data_keys_at(
                r#"<div x-data="{ a: 1 }"><div x-data="{ b: 2 }"><span x-show=""#
            ),
            ["b"]
        );
        // An unclosed sibling tag does not hide the provider.
        assert_eq!(
            x_data_keys_at(
                r#"<div x-data="{ open: false }"><input value="x"><span x-show=""#
            ),
            ["open"]
        );
    }

    #[test]
    fn offers_x_data_properties_in_expression_values() {
        let labels = |before: &str, line: &str| {
            alpine_data_property_items(before, line, None)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        let doc = r#"<div x-data="{ open: false, count: 0 }"><span x-show="op"#;
        assert_eq!(labels(doc, r#"<span x-show="op"#), ["open", "count"]);
        let doc = r#"<div x-data="{ open: false }"><span @click="to"#;
        assert_eq!(labels(doc, r#"<span @click="to"#), ["open"]);
        let doc = r#"<div x-data="{ open: false }"><span :class="op"#;
        assert_eq!(labels(doc, r#"<span :class="op"#), ["open"]);
        // Plain attributes and closed values are not expression contexts.
        let doc = r#"<div x-data="{ open: false }"><span class="op"#;
        assert!(labels(doc, r#"<span class="op"#).is_empty());
        let doc = r#"<div x-data="{ open: false }"><span title="op"#;
        assert!(labels(doc, r#"<span title="op"#).is_empty());
        // wire: values stay Livewire contexts, not Alpine ones.
        let doc = r#"<div x-data="{ open: false }"><input wire:model="em"#;
        assert!(labels(doc, r#"<input wire:model="em"#).is_empty());
    }

    /// Regression: the exact ws365 host fixture where the GUI showed the
    /// ancestor keys only as buffer words (item 365b).
    #[test]
    fn offers_x_data_properties_from_an_ancestor_tag() {
        let before = concat!(
            "<!DOCTYPE html>\n",
            "<html>\n",
            "<body>\n",
            "<div x-data=\"{ count: 0, label: 'hi' }\" x-text=\"\"></div>\n",
            "<div x-data=\"{ outerKey: 1 }\">\n",
            "  <div x-data=\"{ innerKey: 2, bump() { this.innerKey++ } }\">\n",
            "    <span x-text=\"inn",
        );
        let line = "    <span x-text=\"inn";
        let labels: Vec<_> = alpine_data_property_items(before, line, None)
            .into_iter()
            .map(|i| i.label)
            .collect();
        assert_eq!(labels, ["innerKey", "bump"]);
    }

    /// Regression: the exact ws365b host fixture - non-ASCII object keys
    /// (`caf\u{e9}`, `na\u{ef}ve`) are valid JavaScript identifiers and must
    /// be offered (item 365b).
    #[test]
    fn offers_unicode_x_data_keys() {
        let before = concat!(
            "<!DOCTYPE html>\n",
            "<html>\n",
            "<body>\n",
            "<div x-data=\"{ caf\u{e9}: 1, na\u{ef}ve: 2 }\" x-text=\"ca",
        );
        let line = "<div x-data=\"{ caf\u{e9}: 1, na\u{ef}ve: 2 }\" x-text=\"ca";
        let labels: Vec<_> = alpine_data_property_items(before, line, None)
            .into_iter()
            .map(|i| i.label)
            .collect();
        assert_eq!(labels, ["caf\u{e9}", "na\u{ef}ve"]);
    }

    /// Regression: an unterminated quoted value on an earlier line must not
    /// swallow following tags - the innermost well-formed x-data still wins
    /// (item 365b).
    #[test]
    fn recovers_x_data_scope_after_malformed_prior_markup() {
        let before = concat!(
            "<!DOCTYPE html>\n",
            "<html>\n",
            "<body>\n",
            "<div x-data=\"{ broken: \"><span x-text=\"\"></span>\n",
            "<div x-data=\"{ caf\u{e9}: 1, naive: 2 }\" x-text=\"caf",
        );
        let line = "<div x-data=\"{ caf\u{e9}: 1, naive: 2 }\" x-text=\"caf";
        let labels: Vec<_> = alpine_data_property_items(before, line, None)
            .into_iter()
            .map(|i| i.label)
            .collect();
        assert_eq!(labels, ["caf\u{e9}", "naive"]);
    }

    /// Item 370: a named `x-data` component resolves to the keys of its
    /// registered `Alpine.data` factory.
    #[test]
    fn resolves_named_x_data_component_properties() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "resources/js/components.js",
            "Alpine.data('accordion', () => ({ section: 0 }));\n\
             Alpine.data('dropdown', () => ({ open: false, items: [], toggle() { this.open = !this.open } }));\n",
        );
        let labels = |before: &str, line: &str| {
            alpine_data_property_items(before, line, Some(r))
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        // Properties of the named component in scope.
        assert_eq!(
            labels(
                r#"<div x-data="dropdown"><span x-show="op"#,
                r#"<span x-show="op"#
            ),
            ["open", "items", "toggle"]
        );
        // The first registration wins; other names are not mixed in.
        assert_eq!(
            labels(
                r#"<div x-data="accordion"><span x-show="sec"#,
                r#"<span x-show="sec"#
            ),
            ["section"]
        );
        // An inner inline object still shadows the outer named scope.
        assert_eq!(
            labels(
                r#"<div x-data="dropdown"><div x-data="{ local: 1 }"><span x-show=""#,
                r#"<span x-show=""#
            ),
            ["local"]
        );
        // Unknown names and block-body factories offer nothing.
        assert!(labels(
            r#"<div x-data="missing"><span x-show="op"#,
            r#"<span x-show="op"#
        )
        .is_empty());
        // Without a project root there is nothing to resolve against.
        assert!(alpine_data_property_items(
            r#"<div x-data="dropdown"><span x-show="op"#,
            r#"<span x-show="op"#,
            None,
        )
        .is_empty());
    }

    #[test]
    fn parses_data_factory_keys() {
        let js = "Alpine.data('tabs', () => ({ tab: 'a', select(t) { this.tab = t } }));";
        assert_eq!(data_factory_keys(js, "tabs"), Some(vec!["tab".to_string(), "select".to_string()]));
        // Different quote styles and several registrations in one file.
        let js = "Alpine.data(\"a\", () => ({ x: 1 })); Alpine.data('b', () => ({ y: 2 }));";
        assert_eq!(data_factory_keys(js, "b"), Some(vec!["y".to_string()]));
        // A shared function reference or a missing name resolves to nothing.
        assert_eq!(data_factory_keys("Alpine.data('c', shared);", "c"), None);
        assert_eq!(data_factory_keys("Alpine.data('c', () => ({ x: 1 }));", "d"), None);
    }

    #[test]
    fn offers_livewire_directive_names() {
        assert_eq!(context("@livewire('"), Some(Kind::LivewireName));
        assert_eq!(context("  @livewire('admin.us"), Some(Kind::LivewireName));
        // Not the directive: no @, so no completion.
        assert_eq!(context("livewire('admin.us"), None);
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "app/Livewire/Admin/UserProfile.php", "");
        put(r, "app/Livewire/Counter.php", "");
        put(r, "resources/views/pages/home.blade.php", "");
        let view = r.join("resources/views/pages/home.blade.php");
        let labels = items(r, Some(&view), "@livewire('")
            .into_iter()
            .map(|i| i.label)
            .collect::<Vec<_>>();
        assert!(labels.contains(&"admin.user-profile".to_string()));
        assert!(labels.contains(&"counter".to_string()));
    }

    #[test]
    fn offers_livewire_tags() {
        assert_eq!(context("<livewire:"), Some(Kind::LivewireTag));
        assert_eq!(context("  <livewire:admin.us"), Some(Kind::LivewireTag));
        assert_eq!(context("<livewire:counter :n=\"1\""), None);
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "app/Livewire/Admin/UserProfile.php", "");
        put(r, "app/Livewire/Counter.php", "");
        put(r, "resources/views/livewire/only-view.blade.php", "");
        put(r, "resources/views/pages/home.blade.php", "");
        let view = r.join("resources/views/pages/home.blade.php");
        let labels = items(r, Some(&view), "<livewire:")
            .into_iter()
            .map(|i| i.label)
            .collect::<Vec<_>>();
        assert!(labels.contains(&"admin.user-profile".to_string()));
        assert!(labels.contains(&"counter".to_string()));
        assert!(labels.contains(&"only-view".to_string()));
        let plain = r.join("routes/web.php");
        put(r, "routes/web.php", "<?php");
        assert!(items(r, Some(&plain), "<livewire:").is_empty());
    }

    #[test]
    fn offers_component_tags() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "resources/views/components/alert.blade.php", "");
        put(r, "resources/views/components/forms/input.blade.php", "");
        put(r, "resources/views/components/card/index.blade.php", "");
        put(r, "resources/views/components/index.blade.php", "");
        put(r, "resources/views/components/plain.php", "");
        put(r, "resources/views/pages/home.blade.php", "");
        let view = r.join("resources/views/pages/home.blade.php");
        let labels = items(r, Some(&view), "<x-")
            .into_iter()
            .map(|i| i.label)
            .collect::<Vec<_>>();
        assert!(labels.contains(&"alert".to_string()));
        assert!(labels.contains(&"forms.input".to_string()));
        // An index.blade.php names its directory; the top-level index
        // names nothing; non-Blade files are not components.
        assert!(labels.contains(&"card".to_string()));
        assert!(!labels.contains(&"index".to_string()));
        assert!(!labels.contains(&"plain".to_string()));
        let item = items(r, Some(&view), "<x-al").into_iter().next().unwrap();
        assert_eq!(item.kind, Some(CompletionItemKind::CLASS));
        // Gate: component tags only complete inside a .blade.php view.
        let plain = r.join("routes/web.php");
        put(r, "routes/web.php", "<?php");
        assert!(items(r, Some(&plain), "<x-").is_empty());
        assert!(items(r, None, "<x-").is_empty());
    }

    #[test]
    fn blade_variables_collects_view_scope() {
        let v = labels(blade_variables(BLADE_VIEW, "    {{ $fo"));
        assert_eq!(
            v,
            [
                "title", "count", "pair", "idx", "item", "user", "disabled",
                "type", "flat", "metrics", "errors", "slot", "attributes"
            ]
        );
        let item = blade_variables(BLADE_VIEW, "{{ $fo")
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(item.kind, Some(CompletionItemKind::VARIABLE));
    }

    #[test]
    fn blade_variables_gate() {
        // Only inside a `$<partial>` token at the end of the line.
        assert!(blade_variables(BLADE_VIEW, "plain text").is_empty());
        assert!(blade_variables(BLADE_VIEW, "{{ $x->fo").is_empty());
        assert!(blade_variables("<?php $$x = 1;", "$$").is_empty());
        assert!(!blade_variables(BLADE_VIEW, "{{ $").is_empty());
    }

    #[test]
    fn blade_variables_excludes_non_definitions() {
        // No view variable is defined; only the always-shared $errors shows.
        assert_eq!(labels(blade_variables("<?php $x += 1;", "$")), ["errors"]);
        assert_eq!(
            labels(blade_variables("<?php $x == 1;", "$x == 1; $")),
            ["errors"]
        );
        assert_eq!(
            labels(blade_variables("{{ $this->foo }}", "{{ $")),
            ["errors"]
        );
        assert_eq!(
            labels(blade_variables("<?php $1abc = 1; $ok2 = 2;", "$")),
            ["ok2", "errors"]
        );
        // Values in @props are not prop names.
        assert_eq!(
            labels(blade_variables(
                "@props([\"tone\" => 'info', \"size\"])",
                "$"
            )),
            ["tone", "size", "errors", "slot", "attributes"]
        );
    }

    #[test]
    fn blade_variables_adds_builtin_scope() {
        // $loop inside an unclosed @foreach, not after @endforeach.
        let t = "@foreach ($items as $i)\n    {{ $lo";
        let v = labels(blade_variables(t, t));
        assert!(v.contains(&"loop".to_string()));
        let t = "@foreach ($items as $i) @endforeach {{ $lo";
        assert!(!labels(blade_variables(t, t)).contains(&"loop".to_string()));
        let t = "@forelse ($items as $i)\n{{ $lo";
        assert!(labels(blade_variables(t, t)).contains(&"loop".to_string()));
        // $message only inside @error.
        let t = "@error('title') {{ $me";
        assert!(labels(blade_variables(t, t)).contains(&"message".to_string()));
        let t = "@error('title') @enderror {{ $me";
        assert!(!labels(blade_variables(t, t)).contains(&"message".to_string()));
        // $errors is always shared; a view-defined $errors wins (no dup).
        let t = "@php($errors = []) @endphp {{ $er";
        let v = labels(blade_variables(t, t));
        assert_eq!(v.iter().filter(|x| *x == "errors").count(), 1);
        // $slot / $attributes only in a component view.
        assert!(!labels(blade_variables("{{ $sl", "{{ $sl"))
            .contains(&"slot".to_string()));
    }

    #[test]
    fn blade_variables_non_ascii_safe() {
        let mut fuzz = String::new();
        let pieces = ["ab", "$", "\u{e9}", "\u{65e5}", " \t", "=", "<>", "@", "'\"", "{{}}"];
        let mut n = 0usize;
        while fuzz.len() < 4000 {
            fuzz.push_str(pieces[n % pieces.len()]);
            n += 1;
        }
        let _ = blade_variables(&fuzz, "$a");
        let _ = blade_variables(&fuzz, "\u{e9}$x");
        let _ = blade_variables(BLADE_VIEW, "{{ $n\u{e9}");
    }
}
