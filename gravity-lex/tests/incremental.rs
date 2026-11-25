use gravity_lex::{lex,lex_blade,incremental::{Cache,Update}};
fn check(cache:&mut Cache,s:String,blade:bool){let expected=if blade{lex_blade(&s)}else{lex(&s)};match cache.update(s,usize::MAX){Update::Ready{output,..}=>{assert_eq!(output.styles,expected.styles);assert_eq!(output.line_checkpoints,expected.line_checkpoints);},_=>panic!("unlimited update deferred")}}
#[test]
fn edits_inside_and_around_multiline_modes_match_full_lex(){
 for blade in [false,true]{for s in ["<p>hi</p>\n<?php\n$x='one\ntwo';\n?>tail\n", "<x-card title=\"{{ $a }}\">\n@php\n$x=<<<TXT\n@endphp\nTXT;\n@endphp\n</x-card>", "<?php\n/* one\ntwo */\n$x=1;\n", "{{-- a\nb --}}\n@if($x)\n{{ $y }}\n@endif"]{
  for(at,_)in s.char_indices().chain(std::iter::once((s.len(),'\0'))){let mut cache=Cache::new(s.into(),blade);let mut changed=s.to_owned();changed.insert_str(at,"λ\n");check(&mut cache,changed,blade);check(&mut cache,s.into(),blade);}
 }}
}
#[test]
fn ordinary_tail_edits_restart_nearby_and_budget_defers_without_mutation(){
 let s=format!("<?php\n{}$last=1;\n","$x=1;\n".repeat(1000));let mut cache=Cache::new(s.clone(),false);let changed=s.replace("$last=1","$last=2");
 match cache.update(changed.clone(),32){Update::Ready{restart,relexed_bytes,..}=>{assert!(restart>5000);assert!(relexed_bytes<32)},other=>panic!("{other:?}")}
 let before=cache.output().styles.clone();let changed=format!("<?php /*\n{}",changed);
 assert!(matches!(cache.update(changed,32),Update::Deferred{..}));assert_eq!(cache.output().styles,before);
}
#[test]
fn incomplete_multiline_edits_have_safe_opening_restart(){
 let s="<?php\n$x='one\ntwo\nthree";let mut cache=Cache::new(s.into(),false);let changed=format!("{s}' ;");
 match cache.update(changed,usize::MAX){Update::Ready{restart,..}=>assert_eq!(restart,9),other=>panic!("{other:?}")}
}

#[test]
fn repeated_random_edits_keep_styles_identical_to_full_lex(){
 let atoms=["λ", "\n", "'", "<?php ", "?>", "{{--", "--}}", "{{", "}}", "@php", "@endphp", "/*", "*/", "<x-a>", "$x", "\\", ";"];
 let mut seed=19usize;
 for blade in [false,true]{let mut source=String::from("<p>start</p>\n<?php\n$x=1;\n?>tail\n");let mut cache=Cache::new(source.clone(),blade);for _ in 0..1000 {seed=seed.wrapping_mul(1664525).wrapping_add(1013904223);let points=source.char_indices().map(|(at,_)|at).chain(std::iter::once(source.len())).collect::<Vec<_>>();let at=points[seed%points.len()];seed=seed.wrapping_mul(1664525).wrapping_add(1013904223);if seed%3==0 && at<source.len(){let end=at+source[at..].chars().next().unwrap().len_utf8();source.replace_range(at..end,"");}else{source.insert_str(at,atoms[seed%atoms.len()]);}check(&mut cache,source.clone(),blade);}}
}

#[test]
fn cancellation_never_publishes_partial_cache(){
 use std::sync::atomic::AtomicUsize;
 let mut cache=Cache::new("<?php $x=1;".into(),false);let before=cache.output().styles.clone();let flag=AtomicUsize::new(1);
 assert!(matches!(cache.update_cancellable("<?php /* changed".into(),usize::MAX,Some(&flag)),Update::Cancelled));assert_eq!(cache.output().styles,before);
}
