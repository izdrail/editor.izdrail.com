use gravity_lex::{lex,lex_blade,cooperative::{Session,Step}};
use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
#[test]
fn resumable_php_blade_multiline_scans_match_full_output(){
 for blade in [false,true] {for source in [format!("<?php $x='{}'; ?>tail", "λ\n".repeat(2000)),format!("{{--{}--}}<p>{{{{ $x }}}}</p>","long text\n".repeat(2000)),format!("<?php /*{}*/ $x=1;", "x\n".repeat(2000)),format!("@php $x='{}'; @endphp", "x\n".repeat(2000)),format!("<?php $x=<<<TXT\n{}TXT;\n", "line\n".repeat(2000)),format!("<?php $identifier{}=1;", "long".repeat(2000)),format!("<?php $x={};", "1234".repeat(2000))] {
  let expected=if blade{lex_blade(&source)}else{lex(&source)};
  let mut session=Session::new(source,blade,Arc::new(AtomicUsize::new(0)));let mut yields=0;
  loop {match session.step(128){Step::Yielded{scanned}=>{assert!(scanned<=128);yields+=1;},Step::Complete(output)=>{assert_eq!(output.styles,expected.styles);break;},Step::Cancelled=>panic!("unexpected cancellation")}}
  assert!(yields>0);
 }}
}
#[test]
fn cancelling_suspended_work_does_not_publish_partial_output(){
 let flag=Arc::new(AtomicUsize::new(0));let mut session=Session::new(format!("<?php $x='{}';","x".repeat(10000)),false,flag.clone());assert!(matches!(session.step(32),Step::Yielded{..}));flag.store(1,Ordering::Release);assert!(matches!(session.step(32),Step::Cancelled));
}
#[test]
fn dropping_suspended_work_releases_worker(){
 let mut session=Session::new(format!("<?php /*{}*/","x".repeat(10000)),false,Arc::new(AtomicUsize::new(0)));assert!(matches!(session.step(32),Step::Yielded{..}));drop(session);
}

#[test]
fn mid_scan_cancellation_during_long_multiline_input_is_observed(){
 let flag=Arc::new(AtomicUsize::new(0));let source=format!("<?php $x=<<<TXT\n{}TXT;\n","long line\n".repeat(10000));let mut session=Session::new(source,false,flag.clone());
 for _ in 0..5{assert!(matches!(session.step(1024),Step::Yielded{..}));}
 flag.store(1,Ordering::Release);assert!(matches!(session.step(1024),Step::Cancelled));
}
