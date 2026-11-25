use gravity_lex::{lex,lex_blade,style,incremental::{Cache,Update},cooperative::{Session,Step}};
use std::{path::PathBuf,process::{Command,Stdio},io::Write,sync::{Arc,atomic::AtomicUsize}};
fn corpus()->Vec<(String,String)>{let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");let mut files=std::fs::read_dir(root).unwrap().map(|entry|entry.unwrap().path()).collect::<Vec<_>>();files.sort();files.into_iter().map(|path|(path.file_name().unwrap().to_string_lossy().into_owned(),std::fs::read_to_string(path).unwrap())).collect()}
#[test]
fn php_tokenizer_owns_php_comment_variable_number_and_tag_spans(){
 let php=std::env::var("PHP_BINARY").unwrap_or_else(|_|"php".into());
 if Command::new(&php).arg("-v").output().is_err(){assert!(std::env::var_os("CI").is_none(),"PHP oracle required on CI");eprintln!("PHP absent, oracle skipped locally");return;}
 let oracle=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/php-token-oracle.php");
 for(name,source)in corpus(){
  let mut command=Command::new(&php);
  if let Ok(extension)=std::env::var("PHP_TOKENIZER_EXTENSION"){command.arg("-n").arg("-d").arg(format!("extension={extension}"));}
  let mut child=command.arg(&oracle).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();child.stdin.take().unwrap().write_all(source.as_bytes()).unwrap();let output=child.wait_with_output().unwrap();assert!(output.status.success(),"{name}: {}",String::from_utf8_lossy(&output.stderr));
  let truth:serde_json::Value=serde_json::from_slice(&output.stdout).expect("oracle JSON");assert_eq!(truth["bytes"].as_u64().unwrap() as usize,source.len());eprintln!("oracle {name} PHP {}",truth["php"]);
  let styles=lex(&source).styles;
  for token in truth["tokens"].as_array().unwrap(){let kind=token[0].as_str().unwrap();let at=token[1].as_u64().unwrap() as usize;let len=token[2].as_u64().unwrap() as usize;let text=&source[at..at+len];
   let expected=match kind{"T_COMMENT"=>Some(if text.starts_with("//")||text.starts_with('#'){style::LINE_COMMENT}else{style::COMMENT}),"T_DOC_COMMENT"=>Some(style::DOC),"T_VARIABLE"=>Some(style::VARIABLE),"T_LNUMBER"|"T_DNUMBER"=>Some(style::NUMBER),"T_OPEN_TAG_WITH_ECHO"=>Some(style::OPEN),"T_OPEN_TAG"=>Some(style::OPEN),"T_CLOSE_TAG"=>Some(style::CLOSE),_=>None};
   if kind=="T_STRING" {assert!(styles[at..at+len].iter().all(|id|matches!(*id,style::IDENTIFIER|style::CONSTANT|style::KEYWORD|style::TYPE|style::FUNCTION)),"{name} name at{at}: {text}");}
   if kind=="T_CONSTANT_ENCAPSED_STRING" {assert!(styles[at..at+len].iter().all(|id|matches!(*id,style::SINGLE|style::DOUBLE)),"{name} literal string at{at}: {text}");}
   if let Some(expected)=expected {let checked=if kind=="T_OPEN_TAG"{len.min(5)}else if kind=="T_CLOSE_TAG"{len.min(2)}else{len};assert!(styles[at..at+checked].iter().all(|id|*id==expected || (kind=="T_VARIABLE" && *id==style::INTERPOLATED)),"{name} {kind} at{at}: {text:?} {:?}",&styles[at..at+checked]);}
  }
 }
}
#[test]
fn corpus_utf8_prefix_and_rapid_typing_regressions(){
 for(name,source)in corpus(){let blade=name.ends_with(".blade.php");let mut cache=Cache::new(String::new(),blade);for(end,_)in source.char_indices().chain(std::iter::once((source.len(),'\0'))){let text=&source[..end];let expected=if blade{lex_blade(text)}else{lex(text)};match cache.update(text.into(),usize::MAX){Update::Ready{output,..}=>assert_eq!(output.styles,expected.styles,"{name} prefix{end}"),_=>panic!("unexpected defer")};}}
}
#[test]
fn large_corpus_requires_cooperative_yields_and_preserves_bytes(){
 let unit=corpus().into_iter().find(|(name,_)|name=="standard.php").unwrap().1;let source=unit.repeat(1000);let expected=lex(&source);let mut session=Session::new(source,false,Arc::new(AtomicUsize::new(0)));let mut yields=0;loop{match session.step(4096){Step::Yielded{scanned}=>{assert!(scanned<=4096);yields+=1},Step::Complete(output)=>{assert_eq!(output.styles,expected.styles);break},Step::Cancelled=>panic!("cancelled")}}assert!(yields>20);
}
