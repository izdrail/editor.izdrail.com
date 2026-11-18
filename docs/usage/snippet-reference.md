# Snippet keyword reference

Type a prefix in a matching file, request completion, choose the snippet and press Enter. Tab and Shift+Tab move between snippet placeholders. These are expansion prefixes, not PHP or JavaScript reserved words. See [Completion and keywords](completion.md) for scope and project detection.

## blade

Source: [`blade.json`](../../defaults/snippets/blade.json)

| Prefix | Description |
| --- | --- |
| if | Blade @if |
| ifelse | Blade @if / @else |
| elseif | Blade @elseif |
| unless | Blade @unless |
| isset | Blade @isset |
| empty | Blade @empty |
| foreach | Blade @foreach |
| forelse | Blade @forelse with empty case |
| for | Blade @for |
| while | Blade @while |
| switch | Blade @switch |
| auth | Blade @auth |
| guest | Blade @guest |
| can | Blade @can |
| env | Blade @env |
| production | Blade @production |
| php | Blade @php block |
| once | Blade @once |
| extends | Blade @extends |
| section | Blade @section |
| yield | Blade @yield |
| include | Blade @include |
| includeif | Blade @includeIf |
| includewhen | Blade @includeWhen |
| push | Blade @push |
| stack | Blade @stack |
| props | Blade component @props |
| component | Blade @component |
| slot | Blade @slot |
| csrf | CSRF token field |
| method | Method spoofing field |
| error | Validation error for a field |
| class | Conditional classes |
| json | Blade @json |
| vite | Vite assets |
| lang | Translation |
| echo | Escaped output |
| raw | Unescaped output |
| comment | Blade comment |
| xcomponent | Blade component tag |
| x-slot | Named component slot |
| livewire | Livewire component tag |
| atlivewire | Livewire @livewire directive |
| livewire-assets | Livewire styles and scripts |
| wire-click | Livewire click action |
| wire-model | Livewire model binding |
| wire-model-live | Livewire live model binding (v3) |
| wire-model-blur | Livewire blur model binding (v3) |
| wire-submit | Livewire form submit (v3) |
| wire-submit-prevent | Livewire form submit (v2) |
| wire-loading | Livewire loading state |
| wire-key | Livewire key for loops |
| wire-poll | Livewire polling |
| entangle | Livewire/Alpine entangle |
| pushonce | Blade @pushOnce |
| prepend | Blade @prepend |
| session | Blade @session |
| checked | Blade @checked |
| selected | Blade @selected |
| disabled | Blade @disabled |
| required | Blade @required |
| readonly | Blade @readonly |
| style | Conditional inline styles |
| aware | Blade @aware |
| fragment | Blade @fragment |
| hassection | Blade @hasSection |
| sectionmissing | Blade @sectionMissing |
| verbatim | Blade @verbatim |
| nonce | Vite CSP nonce attribute |
| links | Pagination links |
| wire-navigate | Livewire SPA-style navigation link |
| wire-confirm | Confirm before running a Livewire action |
| wire-dirty | Show only when input state diverges from the server |
| wire-offline | Show only when the user is offline |
| wire-cloak | Hide until Livewire initializes |
| wire-init | Livewire: run an action when the element loads |
| wire-ignore | Livewire: skip DOM diffing for this element |
| wire-stream | Livewire: streamed response target |
| wire-target | Livewire: scope loading/offline states to an action |
| wire-transition | Livewire: transition when the element is added or removed |
| x-data | Alpine: component state |
| x-init | Alpine: run when the component initialises |
| x-show | Alpine: toggle visibility |
| x-on | Alpine: event listener (x-on avoids the Blade @ clash) |
| x-bind | Alpine: bind an attribute |
| x-model | Alpine: two-way binding |
| x-text | Alpine: set text content |
| x-html | Alpine: set inner HTML (trusted content only) |
| x-ref | Alpine: element reference ($refs) |
| x-cloak | Alpine: hide until Alpine has initialised (needs [x-cloak]{display:none}) |
| x-transition | Alpine: transition for x-show |
| x-for | Alpine: loop |
| x-if | Alpine: conditional render |
| x-effect | Alpine: re-run when dependencies change |
| x-teleport | Alpine: move markup elsewhere in the DOM |
| x-ignore | Alpine: do not initialise this subtree |
| x-id | Alpine: scoped ids ($id) |
| x-modelable | Alpine: expose a property to x-model on the parent |
| dd | Blade: dump and die |
| dump | Blade: dump without stopping |
| canany | Blade: any of these abilities |
| cannot | Blade: missing ability |
| each | Blade: render a view per item |
| includefirst | Blade: first view that exists |
| inject | Blade: resolve a service |
| use | Blade: import a class |
| pushif | Blade: conditional @push |
| prependonce | Blade: @prependOnce block |

## javascript

Source: [`javascript.json`](../../defaults/snippets/javascript.json)

| Prefix | Description |
| --- | --- |
| fn | Function declaration |
| afn | Async function declaration |
| arrow | Arrow function assigned to a const |
| imp | Default import |
| impn | Named import |
| expd | Default export |
| fetch | fetch with await and JSON parsing |
| map | Array map |
| filter | Array filter |
| reduce | Array reduce |
| foreach | Array forEach |
| eli | addEventListener |
| qs | querySelector |
| qsa | querySelectorAll |
| cl | console.log |
| prom | new Promise |
| tc | try / catch |
| class | Class with constructor |

## laravel

Source: [`laravel.json`](../../defaults/snippets/laravel.json)

| Prefix | Description |
| --- | --- |
| route-get | Laravel route to a controller method |
| route-resource | Laravel resource routes |
| route-group | Laravel route group |
| lv-model | Eloquent model class |
| lv-controller | Controller with index action |
| lv-migration | Create-table migration |
| lv-validate | Request validation |
| lv-hasmany | hasMany relation |
| lv-belongsto | belongsTo relation |
| lv-formrequest | FormRequest rules method |
| lv-factory | Model factory definition |
| lv-test | Feature test method |
| lv-log | Log::info call |
| lv-job | Queued job |
| lv-event | Event class |
| lv-listener | Event listener |
| lv-middleware | HTTP middleware |
| lv-resource | JsonResource |
| lv-policy | Policy class |
| lv-command | Console command |
| lv-scope | Eloquent local scope |
| lv-accessor | Eloquent attribute accessor/mutator |
| lv-belongstomany | belongsToMany relation |
| lv-hasone | hasOne relation |
| lv-pest | Pest test |
| route-api | apiResource routes |
| route-middleware | Route group with middleware |
| lv-dd | dd() |
| route-post | Laravel POST route to a controller method |
| route-put | Laravel PUT route to a controller method |
| route-patch | Laravel PATCH route to a controller method |
| route-delete | Laravel DELETE route to a controller method |
| route-view | Laravel route straight to a view |
| lv-seeder | Database seeder run method |
| lv-observer | Eloquent observer method |
| lv-notification | Notification mail channel message |
| lv-mail | Mailable envelope and content methods |
| lv-cast | Custom Eloquent cast get/set methods |
| lv-morphmany | Polymorphic one-to-many relation |
| lv-schedule | Scheduled artisan command (routes/console.php) |
| lv-cache | Cache::remember with a callback |

## livewire

Source: [`livewire.json`](../../defaults/snippets/livewire.json)

| Prefix | Description |
| --- | --- |
| lw-component | Livewire 3 component class |
| lw-rule | Livewire 3 #[Rule] property |
| lw-on | Livewire 3 #[On] listener |
| lw-computed | Livewire 3 #[Computed] property |
| lw-dispatch | Livewire 3 dispatch event |
| lw-validate | Livewire validate |
| lw-redirect | Livewire redirect |
| lw-mount | Livewire mount |
| lw-reset | Livewire reset property |
| lw-pagination | Livewire pagination trait |
| lw-render | Livewire render method |
| lw-form | Livewire form object class |
| lw-url | Property persisted in the URL query string |
| lw-locked | Property the front end cannot change |
| lw-modelable | Child property bindable from a parent with wire:model |
| lw-session | Property persisted in the session |
| lw-test | Livewire component test |

## symfony

Source: [`symfony.json`](../../defaults/snippets/symfony.json)

| Prefix | Description |
| --- | --- |
| sf-controller | Controller with a Route attribute |
| sf-route | Route attribute |
| sf-entity | Doctrine entity |
| sf-column | Doctrine string column |
| sf-service | Constructor property promotion |
| sf-command | Console command |
| sf-formtype | FormType buildForm |
| sf-subscriber | EventSubscriber events |
| sf-voter | Security voter skeleton |
| sf-test | WebTestCase smoke test |
| sf-messenger | AsMessageHandler class |
| sf-repository | ServiceEntityRepository skeleton |
| sf-twigextension | Twig filter extension |
| sf-listener | AsEventListener class |
| sf-constraint | Custom validation constraint |
| sf-constraint-validator | Validator for a custom constraint |
| sf-authenticator | Custom authenticator (AbstractAuthenticator) |
| sf-normalizer | Serializer normalizer methods |
| sf-twig-component | UX Twig component class |
| sf-live-component | UX Live component class |
| sf-schedule-task | Scheduler task with a cron trigger |
| sf-mail | Send a templated email |
| sf-event-listener | Event listener via AsEventListener |

## twig

Source: [`twig.json`](../../defaults/snippets/twig.json)

| Prefix | Description |
| --- | --- |
| for | Twig for loop |
| if | Twig if |
| ifelse | Twig if/else |
| extends | Twig extends |
| block | Twig block |
| include | Twig include |
| set | Twig set |
| path | Symfony path() |
| asset | Symfony asset() |
| trans | Translation filter |
| forelse | Twig for / else |
| macro | Twig macro |
| import | Twig import |
| from | Twig from import |
| embed | Twig embed |
| with | Twig with |
| apply | Twig apply filter |
| autoescape | Twig autoescape |
| includewith | Twig include with variables |
| comment | Twig comment |
| echo | Twig output |
| pathp | Twig path with parameters |
| url | Twig absolute url |
| dump | Twig dump |
| form | Symfony form rendering |
| form_row | Symfony form row |
| csrf | Symfony CSRF token |
| flash | Symfony flash messages |
| isgranted | Symfony is_granted |
| twig-component | Twig component tag |
| parent | Twig parent block |
| setblock | Twig set block |

## wordpress

Source: [`wordpress.json`](../../defaults/snippets/wordpress.json)

| Prefix | Description |
| --- | --- |
| wp-action | WordPress action hook |
| wp-filter | WordPress filter hook |
| wp-plugin | Plugin main file header |
| wp-cpt | register_post_type |
| wp-query | WP_Query with the loop |
| wp-enqueue | Enqueue style and script |
| wp-shortcode | add_shortcode |
| wp-rest | register_rest_route |
| wp-esc | esc_html output |
| wp-nonce | wp_nonce_field |
| wp-i18n | Translatable string |
| wp-taxonomy | register_taxonomy on init |
| wp-metabox | add_meta_box with nonce and saved field |
| wp-ajax | wp_ajax_ hooks for logged-in and anonymous users |
| wp-nav-menu | register_nav_menus |
| wp-image-size | add_image_size |
| wp-remote-get | wp_remote_get with error check |
| wp-widget | WP_Widget skeleton |
| wp-cron | schedule a cron event |
| wp-transient | Transient get-or-set cache pattern |
| wp-option | Read an option with a default |
| wp-settings | Settings API: setting, section and field |
| wp-block | Register a block from its block.json directory |
| wp-customizer | Customizer section, setting and control |
| wp-prepare | wpdb prepared SELECT |
| wp-admin-page | add_menu_page with a render callback |
| wp-pagination | paginate_links for a WP_Query |
| wp-localize | Pass PHP data to an enqueued script |
