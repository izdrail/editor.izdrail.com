<?php
// Execute selected unchanged editor expansions only in this disposable fixture.
require __DIR__.'/fixtures/laravel/vendor/autoload.php';
$app = require __DIR__.'/fixtures/laravel/bootstrap/app.php';
$app->make(Illuminate\Contracts\Console\Kernel::class)->bootstrap();
$app['config']->set('database.default', 'sqlite');
$app['config']->set('database.connections.sqlite', ['driver' => 'sqlite', 'database' => ':memory:', 'prefix' => '', 'foreign_key_constraints' => true]);
$entries = json_decode(file_get_contents($argv[1]), true, flags: JSON_THROW_ON_ERROR);
$snippets = [];
foreach ($entries as $entry) foreach ($entry['prefix'] as $prefix) $snippets[$prefix] = $entry['text'];
function code(string $prefix): string {
    global $snippets;
    if (!isset($snippets[$prefix])) throw new RuntimeException('Missing '.$prefix);
    return preg_replace('/^<\?php\s*/', '', $snippets[$prefix]);
}
function verify(bool $ok, string $message): void {
    if (!$ok) throw new RuntimeException($message);
    echo $message." PASS\n";
}
$schema = $app['db']->connection()->getSchemaBuilder();
$create = eval(code('lv-migration'));
$create->up();
verify($schema->hasTable('table') && $schema->hasColumn('table', 'id'), 'create migration');
$create->down();
verify(!$schema->hasTable('table'), 'create migration rollback');
$schema->create('users', fn ($table) => $table->id());
$modify = eval(code('lv-migration-modify'));
$modify->up();
verify($schema->hasColumn('users', 'nickname'), 'modify migration');
$modify->down();
verify(!$schema->hasColumn('users', 'nickname'), 'modify migration rollback');
$create->up();
$drop = eval(code('lv-migration-drop'));
$drop->up();
verify(!$schema->hasTable('table'), 'drop migration');
try { $drop->down(); throw new RuntimeException('unsafe rollback accepted'); }
catch (LogicException $expected) { echo "drop rollback refuses false restoration PASS\n"; }
$model = new class extends Illuminate\Database\Eloquent\Model {};
$model->setRawAttributes(['id' => 42, 'password' => 'fixture-only', 'email' => 'synthetic@example.invalid']);
$before = $model->getVisible();
eval(code('lv-model-to-array'));
eval(code('lv-model-to-json'));
verify($attributes === ['id' => 42] && json_decode($json, true) === ['id' => 42], 'allowlisted serialization');
verify($model->getVisible() === $before, 'serialization leaves original model visibility unchanged');
$request = Illuminate\Http\Request::create('/fixture', 'POST', ['archived' => 'true']);
eval(code('lv-request-boolean'));
verify($archived === true, 'request boolean true');
$request = Illuminate\Http\Request::create('/fixture', 'POST', ['archived' => 'false']);
eval(code('lv-request-boolean'));
verify($archived === false, 'request boolean false');
$rules = eval('return ['.code('lv-validation-required-if').'];');
verify(!$app['validator']->make([], $rules)->fails(), 'requiredIf false accepts absent input');
verify($app['validator']->make(['field' => []], $rules)->fails(), 'conditional rule retains string validation');
eval(code('lv-auth-user-provider'));
$provider = new App\Auth\CustomUserProvider();
$user = new Illuminate\Foundation\Auth\User();
verify($provider instanceof Illuminate\Contracts\Auth\UserProvider, 'custom provider implements real framework contract');
verify($provider->retrieveById(1) === null && $provider->retrieveByToken(1, 'synthetic') === null && $provider->retrieveByCredentials([]) === null, 'provider denies retrieval until implemented');
verify(!$provider->validateCredentials($user, []), 'provider denies credential validation');
foreach (['updateRememberToken', 'rehashPasswordIfRequired'] as $method) {
    try {
        $provider->$method($user, $method === 'updateRememberToken' ? 'synthetic' : []);
        throw new RuntimeException('unimplemented persistence accepted');
    } catch (LogicException $expected) { echo $method." explicitly refuses unimplemented persistence PASS\n"; }
}
echo "Selected real Laravel runtime fixtures PASS, not all-template runtime acceptance\n";
