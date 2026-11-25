//! A stack-preserving worker continuation for large lexical invalidations.
//! Each permit grants bounded scanning work; unpublished state stays on worker.
use std::sync::{Arc,mpsc::{sync_channel,SyncSender,Receiver},atomic::{AtomicUsize,Ordering}};
use std::thread::JoinHandle;
use crate::{Driver,Output,HtmlContext};

pub enum Step {Yielded{scanned:usize},Complete(Output),Cancelled}
enum Event {Yielded(usize),Complete(Output),Cancelled}
pub(crate) struct Budget {left:usize,used:usize,permits:Receiver<usize>,events:SyncSender<Event>,cancel:Arc<AtomicUsize>}
impl Budget {
 pub(crate) fn tick(&mut self,amount:usize)->bool {
  if self.cancel.load(Ordering::Relaxed)!=0{return false;}
  let mut remaining=amount;
  while remaining>self.left {
   remaining-=self.left;self.used+=self.left;self.left=0;
   if self.events.send(Event::Yielded(self.used)).is_err(){return false;}
   match self.permits.recv(){Ok(next)=>{self.left=next.max(1);self.used=0;},Err(_)=>return false}
   if self.cancel.load(Ordering::Relaxed)!=0{return false;}
  }
  self.left-=remaining;self.used+=remaining;true
 }
}
pub struct Session {permits:Option<SyncSender<usize>>,events:Receiver<Event>,cancel:Arc<AtomicUsize>,thread:Option<JoinHandle<()>>,finished:bool}
impl Session {
 pub fn new(source:String,blade:bool,cancel:Arc<AtomicUsize>)->Self {
  let(permits,receive)=sync_channel(0);let(events,results)=sync_channel(1);let worker_cancel=cancel.clone();
  let thread=std::thread::spawn(move || {
   let first=match receive.recv(){Ok(value)=>value,Err(_)=>return};
   let budget=Budget{left:first,used:0,permits:receive,events:events.clone(),cancel:worker_cancel.clone()};
   let driver=Driver{blade,source:&source,styles:vec![crate::style::HTML;source.len()],checkpoints:vec![],line_checkpoints:vec![],html:HtmlContext::Text,cancel:Some(&worker_cancel),budget:Some(budget)};
   let output=driver.run();
   let event=if worker_cancel.load(Ordering::Acquire)!=0 {Event::Cancelled}else{Event::Complete(output)};
   let _=events.send(event);
  });
  Self{permits:Some(permits),events:results,cancel,thread:Some(thread),finished:false}
 }
 pub fn step(&mut self,budget:usize)->Step {
  if self.finished{return Step::Cancelled;}
  if self.permits.as_ref().is_none_or(|send|send.send(budget.max(1)).is_err()){self.finished=true;return Step::Cancelled;}
  match self.events.recv(){Ok(Event::Yielded(scanned))=>Step::Yielded{scanned},Ok(Event::Complete(output))=>{self.finished=true;Step::Complete(output)},_=>{self.finished=true;Step::Cancelled}}
 }
}
impl Drop for Session {
 fn drop(&mut self){
  if !self.finished{self.cancel.store(1,Ordering::Release);}
  self.permits.take();
  if let Some(thread)=self.thread.take(){let _=thread.join();}
 }
}
