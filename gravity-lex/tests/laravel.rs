use gravity_lex::{lex,lex_blade,style,laravel::{self,Vocabulary}};
fn vocab()->Vocabulary<'static>{Vocabulary{facades:"Route Artisan Gate Event Notification App",route_methods:"get post middleware",artisan:"make:model migrate:fresh queue:work",helpers:"app route view dispatch event policy",eloquent:"where hasMany belongsTo with"}}
fn styles(s:&str)->Vec<u8>{let mut o=lex(s);assert!(laravel::overlay(s,&mut o.styles,&vocab()));o.styles}
fn scope(s:&str,word:&str,id:u8){let at=s.find(word).unwrap();let o=styles(s);assert!(o[at..at+word.len()].iter().all(|v|*v==id),"{word}: {:?}",&o[at..at+word.len()]);}
#[test]
fn facades_routes_models_relationships_helpers(){
 let s="<?php Route::get('/x', fn()=>view('x')); $u=User::where('x',1)->with('posts'); return $this->hasMany(Post::class);";
 scope(s,"Route",laravel::FACADE);scope(s,"get",laravel::METHOD);scope(s,"view",laravel::HELPER);scope(s,"where",laravel::METHOD);scope(s,"hasMany",laravel::METHOD);scope(s,"$u",style::VARIABLE);
}
#[test]
fn jobs_events_notifications_policies_providers_container(){
 let s="<?php use App\\Jobs\\Job; class Send extends ServiceProvider implements ShouldQueue { public function boot() { $this->app->singleton('x', fn()=>new Notification); Gate::define('edit', fn()=>true); Event::listen('sent', fn()=>null); $user->notify($notice); dispatch($job); } }";
 scope(s,"ServiceProvider",laravel::CONVENTION);scope(s,"ShouldQueue",laravel::CONVENTION);scope(s,"singleton",laravel::METHOD);scope(s,"Gate",laravel::FACADE);scope(s,"define",laravel::METHOD);scope(s,"listen",laravel::METHOD);scope(s,"notify",laravel::METHOD);scope(s,"dispatch",laravel::HELPER);
}
#[test]
fn contexts_do_not_leak_into_ordinary_strings_comments_or_declarations(){
 let s="<?php Route::middleware(['auth','throttle:60,1'])->get('/x',fn()=>null); Artisan::call('migrate:fresh'); $s='Route::get view()'; /* Route::get */ function view() {} $x='auth';";
 scope(s,"'auth'",laravel::CONTEXT_STRING);scope(s,"'throttle:60,1'",laravel::CONTEXT_STRING);scope(s,"'migrate:fresh'",laravel::CONTEXT_STRING);scope(s,"'Route::get view()'",style::SINGLE);scope(s,"/* Route::get */",style::COMMENT);
 let out=styles(s);let at=s.find("function view").unwrap()+9;assert_ne!(out[at],laravel::HELPER);let at=s.rfind("'auth'").unwrap();assert_eq!(out[at],style::SINGLE);
}
#[test]
fn blade_host_and_default_php_styles_are_not_replaced(){
 let s="<x-card>{{ route('home') }}</x-card>text Route::get('x')";let mut o=lex_blade(s);let before=o.styles.clone();laravel::overlay(s,&mut o.styles,&vocab());let at=s.find("route(").unwrap();assert_eq!(o.styles[at],laravel::HELPER);let at=s.find("x-card").unwrap();assert_eq!(o.styles[at],style::HTML_TAG);let at=s.find("Route::get").unwrap();assert_eq!(o.styles[at],before[at]);assert_eq!(o.styles.len(),s.len());
}
#[test]
fn malformed_and_utf8_prefixes_are_total(){
 let s="<?php Route::middleware(['auth', fn()=>view('λ')])->get('/'); class X extends ServiceProvider {";
 for(end,_)in s.char_indices().chain(std::iter::once((s.len(),'\0'))){let out=styles(&s[..end]);assert_eq!(out.len(),end);for(at,c)in s[..end].char_indices(){assert!(out[at..at+c.len_utf8()].iter().all(|v|*v==out[at]));}}
 assert!(!laravel::overlay("λ",&mut [0],&vocab()));
}

#[test]
fn unrelated_calls_and_qualified_helpers_are_not_promoted() {
 let s="<?php $x->route('x'); function app() {} new App(); Vendor\\view(); $c->call('migrate:fresh'); $x='auth';";
 let out=styles(s);
 for word in ["route", "app", "view", "'migrate:fresh'", "'auth'"] {let at=s.find(word).unwrap();assert!(!matches!(out[at],laravel::HELPER|laravel::CONTEXT_STRING),"{word}");}
}
