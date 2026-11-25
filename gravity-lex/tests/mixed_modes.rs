use gravity_lex::{lex, style, Mode, HtmlContext};
fn scope(s:&str,n:&str,st:u8) { let o=lex(s);for (at,_) in s.match_indices(n) { assert!(o.styles[at..at+n.len()].iter().all(|id|*id==st),"{n}: {:?}",&o.styles[at..at+n.len()]); } }
#[test]
fn markup_scopes_resume_around_php_attribute_and_comment_islands(){
 let s=r#"<section class="before <?= $class ?> after" data-id='x'>text <?php echo '?>'; ?> tail</section><!-- a <?= $comment ?> b -->"#;
 for n in ["section"] {scope(s,n,style::HTML_TAG)}
 for n in ["class","data-id"] { // class also appears as a PHP variable, check host occurrence only
  let at=s.find(n).unwrap();assert_eq!(lex(s).styles[at],style::HTML_ATTRIBUTE);
 }
 scope(s,"before ",style::HTML_STRING);scope(s," after",style::HTML_STRING);
 scope(s," tail",style::HTML);scope(s,"<!-- a ",style::HTML_COMMENT);scope(s," b -->",style::HTML_COMMENT);
 scope(s,"$class",style::VARIABLE);scope(s,"$comment",style::VARIABLE);
 let o=lex(s);assert_eq!(o.checkpoints.len(),7);
 assert_eq!(o.checkpoints[1].host,HtmlContext::Quoted(b'"'));
 assert_eq!(o.checkpoints[5].host,HtmlContext::Comment);
 assert!(o.checkpoints.windows(2).all(|w| w[0].offset<w[1].offset && w[0].mode!=w[1].mode));
 assert_eq!(o.checkpoints.last().unwrap().mode,Mode::Html);
}
#[test]
fn only_real_php_tags_change_mode(){
 for suffix in ["x","/**/","\u{000b}","\u{000c}","λ"] {
  let s=format!("<p><?php{suffix} untouched</p>");assert_eq!(lex(&s).checkpoints.len(),1,"{suffix:?}");
 }
 for ws in [" ","\t","\r","\n","\r\n"] {
  let s=format!("<?PhP{ws}$x=1;?>plain<?= $x ?>");
  assert_eq!(lex(&s).checkpoints.len(),5);scope(&s,"$x",style::VARIABLE);scope(&s,"plain",style::HTML);
 }
 for s in ["<?xml version='1.0'?>","<? echo 1;?>"] {assert_eq!(lex(s).checkpoints.len(),1)}
}
#[test]
fn comment_string_and_document_close_tags_do_not_escape_php(){
 for body in ["'?>'","\"?>\"","`?>`","/* ?> */","/** ?> */","<<<TXT\n?>\nTXT;","<<<'TXT'\n?>\nTXT;"] {
  let s=format!("before<?php {body}; $after=1; ?>after");
  scope(&s,"$after",style::VARIABLE);assert_eq!(lex(&s).checkpoints.len(),3,"{body}");
 }
 for comment in ["//","#"] {let s=format!("<?php {comment} closes ?>after");scope(&s,"after",style::HTML);}
}
#[test]
fn every_mixed_prefix_is_total_aligned_and_deterministic(){
 for s in ["<a title='before <?= $x ?> after'>λ</a>","<!-- <?= $x ?> -->", "<div x=\"<?notphp ?>\">λ</div>","é < <?php $x=<<<TXT\n?>\nTXT; ?>✓"] {
  for (end,_) in s.char_indices().chain(std::iter::once((s.len(),'\0'))) {
   let o=lex(&s[..end]);assert_eq!(o.styles.len(),end);assert_eq!(o.styles,lex(&s[..end]).styles);
   for (at,c) in s[..end].char_indices(){assert!(o.styles[at..at+c.len_utf8()].iter().all(|id|*id==o.styles[at]));}
  }
 }
}
