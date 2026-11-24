# Snippet keyword reference

Type a prefix in a matching file, request completion and choose the snippet. Tab/Shift+Tab move snippet placeholders after insertion. Laravel-family prefixes were renamed to lv-; old aliases were removed.

## blade

| Prefix | Description |
| --- | --- |
| lv-blade-if | Blade @if |
| lv-blade-ifelse | Blade @if / @else |
| lv-blade-elseif | Blade @elseif |
| lv-blade-unless | Blade @unless |
| lv-blade-isset | Blade @isset |
| lv-blade-empty | Blade @empty |
| lv-blade-foreach | Blade @foreach |
| lv-blade-forelse | Blade @forelse with empty case |
| lv-blade-for | Blade @for |
| lv-blade-while | Blade @while |
| lv-blade-switch | Blade @switch |
| lv-blade-auth | Blade @auth |
| lv-blade-guest | Blade @guest |
| lv-blade-can | Blade @can |
| lv-blade-env | Blade @env |
| lv-blade-production | Blade @production |
| lv-blade-php | Blade @php block |
| lv-blade-once | Blade @once |
| lv-blade-extends | Blade @extends |
| lv-blade-section | Blade @section |
| lv-blade-yield | Blade @yield |
| lv-blade-include | Blade @include |
| lv-blade-includeif | Blade @includeIf |
| lv-blade-includewhen | Blade @includeWhen |
| lv-blade-push | Blade @push |
| lv-blade-stack | Blade @stack |
| lv-blade-props | Blade component @props |
| lv-blade-component | Blade @component |
| lv-blade-slot | Blade @slot |
| lv-blade-csrf | CSRF token field |
| lv-blade-method | Method spoofing field |
| lv-blade-error | Validation error for a field |
| lv-blade-class | Conditional classes |
| lv-blade-json | Blade @json |
| lv-blade-vite | Vite assets |
| lv-blade-lang | Translation |
| lv-blade-echo | Escaped output |
| lv-blade-raw | Unescaped output |
| lv-blade-comment | Blade comment |
| lv-blade-xcomponent | Blade component tag |
| lv-blade-x-slot | Named component slot |
| lv-blade-livewire | Livewire component tag |
| lv-blade-atlivewire | Livewire @livewire directive |
| lv-blade-livewire-assets | Livewire styles and scripts |
| lv-blade-wire-click | Livewire click action |
| lv-blade-wire-model | Livewire model binding |
| lv-blade-wire-model-live | Livewire live model binding (v3) |
| lv-blade-wire-model-blur | Livewire blur model binding (v3) |
| lv-blade-wire-submit | Livewire form submit (v3) |
| lv-blade-wire-submit-prevent | Livewire form submit (v2) |
| lv-blade-wire-loading | Livewire loading state |
| lv-blade-wire-key | Livewire key for loops |
| lv-blade-wire-poll | Livewire polling |
| lv-blade-entangle | Livewire/Alpine entangle |
| lv-blade-pushonce | Blade @pushOnce |
| lv-blade-prepend | Blade @prepend |
| lv-blade-session | Blade @session |
| lv-blade-checked | Blade @checked |
| lv-blade-selected | Blade @selected |
| lv-blade-disabled | Blade @disabled |
| lv-blade-required | Blade @required |
| lv-blade-readonly | Blade @readonly |
| lv-blade-style | Conditional inline styles |
| lv-blade-aware | Blade @aware |
| lv-blade-fragment | Blade @fragment |
| lv-blade-hassection | Blade @hasSection |
| lv-blade-sectionmissing | Blade @sectionMissing |
| lv-blade-verbatim | Blade @verbatim |
| lv-blade-nonce | Vite CSP nonce attribute |
| lv-blade-links | Pagination links |
| lv-blade-wire-navigate | Livewire SPA-style navigation link |
| lv-blade-wire-confirm | Confirm before running a Livewire action |
| lv-blade-wire-dirty | Show only when input state diverges from the server |
| lv-blade-wire-offline | Show only when the user is offline |
| lv-blade-wire-cloak | Hide until Livewire initializes |
| lv-blade-wire-init | Livewire: run an action when the element loads |
| lv-blade-wire-ignore | Livewire: skip DOM diffing for this element |
| lv-blade-wire-stream | Livewire: streamed response target |
| lv-blade-wire-target | Livewire: scope loading/offline states to an action |
| lv-blade-wire-transition | Livewire: transition when the element is added or removed |
| lv-blade-x-data | Alpine: component state |
| lv-blade-x-init | Alpine: run when the component initialises |
| lv-blade-x-show | Alpine: toggle visibility |
| lv-blade-x-on | Alpine: event listener (x-on avoids the Blade @ clash) |
| lv-blade-x-bind | Alpine: bind an attribute |
| lv-blade-x-model | Alpine: two-way binding |
| lv-blade-x-text | Alpine: set text content |
| lv-blade-x-html | Alpine: set inner HTML (trusted content only) |
| lv-blade-x-ref | Alpine: element reference ($refs) |
| lv-blade-x-cloak | Alpine: hide until Alpine has initialised (needs [x-cloak]{display:none}) |
| lv-blade-x-transition | Alpine: transition for x-show |
| lv-blade-x-for | Alpine: loop |
| lv-blade-x-if | Alpine: conditional render |
| lv-blade-x-effect | Alpine: re-run when dependencies change |
| lv-blade-x-teleport | Alpine: move markup elsewhere in the DOM |
| lv-blade-x-ignore | Alpine: do not initialise this subtree |
| lv-blade-x-id | Alpine: scoped ids ($id) |
| lv-blade-x-modelable | Alpine: expose a property to x-model on the parent |
| lv-blade-dd | Blade: dump and die |
| lv-blade-dump | Blade: dump without stopping |
| lv-blade-canany | Blade: any of these abilities |
| lv-blade-cannot | Blade: missing ability |
| lv-blade-each | Blade: render a view per item |
| lv-blade-includefirst | Blade: first view that exists |
| lv-blade-inject | Blade: resolve a service |
| lv-blade-use | Blade: import a class |
| lv-blade-pushif | Blade: conditional @push |
| lv-blade-prependonce | Blade: @prependOnce block |

## javascript

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

| Prefix | Description |
| --- | --- |
| lv-route-get | Laravel route to a controller method |
| lv-route-resource | Laravel resource routes |
| lv-route-group | Laravel route group |
| lv-model | Eloquent model class |
| lv-controller | Controller with index action |
| lv-migration | Create-table migration |
| lv-validate | Request validation |
| lv-hasmany | hasMany relation with fully qualified return type |
| lv-belongsto | belongsTo relation with fully qualified return type |
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
| lv-belongstomany | belongsToMany relation with fully qualified return type |
| lv-hasone | hasOne relation with fully qualified return type |
| lv-pest | Pest test |
| lv-route-api | apiResource routes |
| lv-route-middleware | Route group with middleware |
| lv-dd | dd() |
| lv-route-post | Laravel POST route to a controller method |
| lv-route-put | Laravel PUT route to a controller method |
| lv-route-patch | Laravel PATCH route to a controller method |
| lv-route-delete | Laravel DELETE route to a controller method |
| lv-route-view | Laravel route straight to a view |
| lv-seeder | Database seeder run method |
| lv-observer | Eloquent observer method |
| lv-notification | Notification mail channel message |
| lv-mail | Mailable envelope and content methods |
| lv-cast | Custom Eloquent cast get/set methods |
| lv-morphmany | Polymorphic one-to-many relation with fully qualified return type |
| lv-schedule | Scheduled artisan command (routes/console.php) |
| lv-cache | Cache::remember with a callback |
| lv-route-named | Named GET route, route file |
| lv-route-prefix | URI and name prefix group, route file |
| lv-route-domain | Subdomain group, register before root-domain routes |
| lv-route-fallback | Fallback route, place after normal routes |
| lv-route-redirect | Redirect route, explicit status |
| lv-route-where-number | Numeric route parameter constraint |
| lv-route-where-uuid | UUID route parameter constraint |
| lv-route-invokable | Route to a controller implementing __invoke |
| lv-route-binding | Implicit model binding with typed argument |
| lv-relation-hasmanythrough | hasManyThrough relation method with fully qualified return type |
| lv-relation-hasonethrough | hasOneThrough relation method with fully qualified return type |
| lv-relation-morphone | morphOne relation method with fully qualified return type |
| lv-relation-morphto | morphTo relation method with fully qualified return type |
| lv-relation-morphtomany | morphToMany relation method with fully qualified return type |
| lv-relation-latest-of-many | One-of-many relation using sortable primary key |
| lv-response-json | JSON response in a controller or route |
| lv-response-redirect | Redirect to a named route with a flash message |
| lv-response-download | Download an existing local file, authorize access first |
| lv-response-file | Inline local file response, authorize access first |
| lv-response-stream-download | Streaming download response callback |
| lv-response-stream-json | Stream a JSON collection, review model serialization and authorization |
| lv-http-get | HTTP GET with explicit timeout and HTTP error propagation |
| lv-http-post | HTTP POST, no automatic retry of non-idempotent requests |
| lv-upload-store | Validate and store upload on an explicit disk, private local by default |
| lv-validation-nested-array | Validate each item of a nested array |
| lv-validation-messages | Inline validation with explicit custom error message |
| lv-form-request-authorize | FormRequest authorization method, choose the matching policy ability/model |
| lv-form-request-class | Complete FormRequest class, policy authorization must be configured |
| lv-form-request-after | FormRequest after-validation hook, replace the explicit condition |
| lv-validation-rule-class | Modern ValidationRule object, replace the explicit validation condition |

## livewire

| Prefix | Description |
| --- | --- |
| lv-livewire-component | Livewire 3 component class |
| lv-livewire-rule | Livewire 3 #[Rule] property |
| lv-livewire-on | Livewire 3 #[On] listener |
| lv-livewire-computed | Livewire 3 #[Computed] property |
| lv-livewire-dispatch | Livewire 3 dispatch event |
| lv-livewire-validate | Livewire validate |
| lv-livewire-redirect | Livewire redirect |
| lv-livewire-mount | Livewire mount |
| lv-livewire-reset | Livewire reset property |
| lv-livewire-pagination | Livewire pagination trait |
| lv-livewire-render | Livewire render method |
| lv-livewire-form | Livewire form object class |
| lv-livewire-url | Property persisted in the URL query string |
| lv-livewire-locked | Property the front end cannot change |
| lv-livewire-modelable | Child property bindable from a parent with wire:model |
| lv-livewire-session | Property persisted in the session |
| lv-livewire-test | Livewire component test |

## symfony

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
