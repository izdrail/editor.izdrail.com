use gravity_lex::{lex, lex_blade, style};
fn scope(s:&str,n:&str,st:u8) {let o=lex_blade(s);for(at,_)in s.match_indices(n){assert!(o.styles[at..at+n.len()].iter().all(|id|*id==st),"{n}: {:?}",&o.styles[at..at+n.len()]);}}
#[test]
fn directives_custom_and_nested_php_arguments(){
 let s="@if($user && call(['a' => fn($x) => $x]))<p>@custom_rule($thing)</p>@endif";
 for n in ["@if","@custom_rule","@endif"]{scope(s,n,style::KEYWORD)}
 for n in ["$user","$x","$thing"]{scope(s,n,style::VARIABLE)}
 let at=s.find("<p>").unwrap()+1;assert_eq!(lex_blade(s).styles[at],style::HTML_TAG);
}
#[test]
fn echo_comments_raw_and_quoted_terminators(){
 let s=r#"{{-- {{ $hidden }} --}}<p>{{ "}}" . $safe }}</p>{!! '<b>!!}</b>' . $raw !!}"#;
 scope(s,"$hidden",style::COMMENT);scope(s,"$safe",style::VARIABLE);scope(s,"$raw",style::VARIABLE);
 let at=s.find("<p>{{").unwrap()+3;assert_eq!(lex_blade(s).styles[at],style::OPEN);
}
#[test]
fn host_attributes_components_and_php_blocks(){
 let s=r#"<x-alert :title="{{ $title }}" wire:click="go">@php $x = '@endphp'; /* @endphp */ $y=1; @endphp<livewire:counter />tail</x-alert>"#;
 scope(s,"x-alert",style::HTML_TAG);scope(s,"livewire:counter",style::HTML_TAG);
 scope(s,":title",style::HTML_ATTRIBUTE);scope(s,"wire:click",style::HTML_ATTRIBUTE);
 scope(s,"$title",style::VARIABLE);scope(s,"$y",style::VARIABLE);scope(s,"tail",style::HTML);
 let at=s.find("go").unwrap();assert_eq!(lex_blade(s).styles[at],style::HTML_STRING);
}
#[test]
fn escaped_and_verbatim_are_literal_and_php_mode_is_unchanged(){
 let s="@@if @{{ $literal }} @{!! $raw !!} @verbatim {{ $inside }} @if @endverbatim {{ $active }}";
 for n in ["$literal","$raw","$inside"]{scope(s,n,style::HTML)}
 scope(s,"$active",style::VARIABLE);
 let plain="{{ $x }} @if($x)";assert!(lex(plain).styles.iter().all(|id|*id==style::HTML));
 let email="contact@example.com";assert!(lex_blade(email).styles.iter().all(|id|*id==style::HTML));
}
#[test]
fn every_utf8_prefix_is_total_deterministic_and_aligned(){
 let samples=["<x-λ a='{{ $name }}'>@if($x)✓@endif</x-λ>","{{--λ--}}{!! [1, ['x'=>2]] !!}","@php $x=<<<'TXT'\n@endphp\nTXT; @endphp","@custom([fn($x)=>['x'=>$x]]) @verbatim{{ λ }}@endverbatim"];
 for s in samples {for(end,_)in s.char_indices().chain(std::iter::once((s.len(),'\0'))){let o=lex_blade(&s[..end]);assert_eq!(o.styles.len(),end);assert_eq!(o.styles,lex_blade(&s[..end]).styles);for(at,c)in s[..end].char_indices(){assert!(o.styles[at..at+c.len_utf8()].iter().all(|id|*id==o.styles[at]));}}}
}

#[test]
fn adjacent_directives_and_multiline_arguments() {
 let s="hello@endif@if\n($x)yes@endif";
 scope(s,"@endif",style::KEYWORD);scope(s,"@if",style::KEYWORD);scope(s,"$x",style::VARIABLE);
}

#[test]
fn deterministic_noise_is_total_in_blade_mode() {
 let atoms=["λ","@","{{","}}","{!!","!!}","{{--","--}}","@php","@endphp","@if(","['x'=>", ")", "'", "\"", "<x-a", " title=", ">", "</x-a>", "\n", "/*", "*/", "\\", "@verbatim", "@endverbatim"];
 let mut seed=17usize;
 for _ in 0..2000 {let mut source=String::new();for _ in 0..24 {seed=seed.wrapping_mul(1664525).wrapping_add(1013904223);source.push_str(atoms[seed%atoms.len()]);}let o=lex_blade(&source);assert_eq!(o.styles.len(),source.len());for(at,c)in source.char_indices(){assert!(o.styles[at..at+c.len_utf8()].iter().all(|id|*id==o.styles[at]));}}
}
