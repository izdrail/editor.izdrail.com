//! Safe line restarts. Large invalidations can be deferred before doing any work.
use crate::{Driver,Output,Mode,HtmlContext,lex,lex_blade};

#[derive(Clone,Debug)]
pub struct Cache {source:String, blade:bool, output:Output}
#[derive(Debug)]
pub enum Update {Ready{output:Output,restart:usize,relexed_bytes:usize},Deferred{restart:usize,required_bytes:usize},Cancelled}
impl Cache {
 pub fn new(source:String,blade:bool)->Self{let output=if blade{lex_blade(&source)}else{lex(&source)};Self{source,blade,output}}
 pub fn from_output(source:String,blade:bool,output:Output)->Option<Self>{
  (source.len()==output.styles.len()).then_some(Self{source,blade,output})
 }
 pub fn blade(&self)->bool{self.blade}
 pub fn output(&self)->&Output{&self.output}
 /// Caller supplies a per-job byte allowance. A deferred update leaves cache intact.
 /// Editing within a multiline token restarts at that token's opener.
 pub fn update(&mut self,source:String,byte_budget:usize)->Update{self.update_cancellable(source,byte_budget,None)}
 pub fn update_cancellable(&mut self,source:String,byte_budget:usize,cancel:Option<&std::sync::atomic::AtomicUsize>)->Update{
  if cancel.is_some_and(|flag|flag.load(std::sync::atomic::Ordering::Acquire)!=0){return Update::Cancelled;}
  let mut common=0;
  for(a,b)in self.source.bytes().zip(source.bytes()) {if a!=b{break;}common+=1;if common%4096==0 {std::thread::yield_now();if cancel.is_some_and(|flag|flag.load(std::sync::atomic::Ordering::Acquire)!=0){return Update::Cancelled;}}}
  let mut changed=common;
  while changed>0 && (!source.is_char_boundary(changed)||!self.source.is_char_boundary(changed)){changed-=1;}
  let line=source[..changed].rfind('\n').map_or(0,|n|n+1);
  let cp=self.output.line_checkpoints.iter().rev().find(|cp|cp.line_offset<=line);
  let(restart,mode,host)=cp.map_or((0,Mode::Html,HtmlContext::Text),|cp|(cp.restart_offset,cp.mode,cp.host));
  let required=source.len()-restart;
  if required>byte_budget{return Update::Deferred{restart,required_bytes:required};}
  let mut styles=vec![crate::style::HTML;source.len()];styles[..restart].copy_from_slice(&self.output.styles[..restart]);
  let driver=Driver{source:&source,blade:self.blade,styles,html:host,cancel,budget:None,
   checkpoints:self.output.checkpoints.iter().filter(|cp|cp.offset<restart).cloned().collect(),
   line_checkpoints:self.output.line_checkpoints.iter().filter(|cp|cp.line_offset<restart).cloned().collect()};
  let output=driver.run_from(restart,mode);
  if cancel.is_some_and(|flag|flag.load(std::sync::atomic::Ordering::Acquire)!=0){return Update::Cancelled;}
  self.source=source;self.output=output.clone();
  Update::Ready{output,restart,relexed_bytes:required}
 }
}
