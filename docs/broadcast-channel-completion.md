# Broadcast channel completion (item 416)

The channels an event broadcasts to - `new Channel('...')` /
`new PrivateChannel('...')` / `new PresenceChannel('...')` in a
`broadcastOn()` return (Laravel 12.x `Broadcasting\Channel::
__construct($name)` :22) and the `Broadcast::on('...')` /
`Broadcast::private('...')` / `Broadcast::presence('...')` /
`Broadcast::broadcast([...], ...)` anonymous-event forms - offer
the names registered with `Broadcast::channel('name', ...)`,
scanned in `app/` and `routes/` (conventionally
`routes/channels.php`). `{param}` wildcards are part of the
offered name; replace them with the id after accepting.

`Broadcast::channel(...)` itself stays silent - it REGISTERS a
channel, the same declaration doctrine that keeps
`Schema::create(...)` out of table completion (item 381).
`Broadcast::on` accepts the array form (strings inside the
first-argument array complete); `private` / `presence` take a
string and `broadcast` an array, so the mismatched shapes stay
silent, as do a second constructor argument, look-alike words
(`new MyChannel(...)`, `renew Channel(...)`), and calls written
inside a comment or a string literal. The constructor prefixes
(`private-` / `presence-`) are never part of the offered name.

Tests: `detects_broadcast_channel_context`,
`offers_broadcast_channels`, plus
`name_replacement_kinds_are_php_file_only` pins.
