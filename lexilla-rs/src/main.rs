use std::{ffi::{CStr,CString,c_char,c_void},time::Instant};
unsafe extern "C" {
 fn lx_new(name:*const c_char,text:*const c_char,n:usize)->*mut c_void;
 fn lx_free(p:*mut c_void); fn lx_words(p:*mut c_void,i:i32,w:*const c_char)->i32;
 fn lx_property(p:*mut c_void,k:*const c_char,v:*const c_char)->i32;
 fn lx_lex(p:*mut c_void,start:usize,n:usize,init:i32,fold:i32)->i32;
 fn lx_styles(p:*mut c_void)->*const u8;fn lx_calls(p:*mut c_void)->*const u64;
 fn lx_named(p:*mut c_void)->i32;fn lx_name(p:*mut c_void,i:i32)->*const c_char;
 fn lx_tags(p:*mut c_void,i:i32)->*const c_char;fn lx_count()->i32;
 fn lx_lexer_name(i:u32,out:*mut c_char,n:i32);
}
struct Lexer{p:*mut c_void,n:usize}
impl Drop for Lexer{fn drop(&mut self){unsafe{lx_free(self.p)}}}
impl Lexer{
 fn new(name:&str,text:&[u8])->Self{let name=CString::new(name).unwrap();let p=unsafe{lx_new(name.as_ptr(),text.as_ptr().cast(),text.len())};assert!(!p.is_null(),"unknown lexer");Self{p,n:text.len()}}
 fn words(&mut self,i:i32,w:&str){let w=CString::new(w).unwrap();assert!(unsafe{lx_words(self.p,i,w.as_ptr())}>=-1);}
 fn property(&mut self,k:&str,v:&str){let k=CString::new(k).unwrap();let v=CString::new(v).unwrap();assert!(unsafe{lx_property(self.p,k.as_ptr(),v.as_ptr())}>=-1);}
 fn lex(&mut self,start:usize,n:usize,init:i32,fold:bool){assert_eq!(unsafe{lx_lex(self.p,start,n,init,fold as i32)},0);}
 fn styles(&self)->&[u8]{unsafe{std::slice::from_raw_parts(lx_styles(self.p),self.n)}}
 fn metadata(&self,i:i32)->(String,String){unsafe{(CStr::from_ptr(lx_name(self.p,i)).to_string_lossy().into_owned(),CStr::from_ptr(lx_tags(self.p,i)).to_string_lossy().into_owned())}}
}
const METHODS:[&str;24]=["Version","SetErrorStatus","Length","GetCharRange","StyleAt","LineFromPosition","LineStart","GetLevel","SetLevel","GetLineState","SetLineState","StartStyling","SetStyleFor","SetStyles","DecorationSetCurrentIndicator","DecorationFillRange","ChangeLexerState","CodePage","IsDBCSLeadByte","BufferPointer","GetLineIndentation","LineEnd","GetRelativePosition","GetCharacterAndWidth"];
fn configure(l:&mut Lexer,name:&str){match name{
 "cpp"=>l.words(0,"int const char return void class public if else for while include"),
 "python"=>{l.words(0,"def return if else for while class import from in None True False");l.words(1,"len str print");},
 "hypertext"=>{l.words(0,"html body script doctype");l.words(1,"const let var function return");l.words(4,"function return echo");},_=>{}}
 l.property("fold","1");l.property("fold.compact","1");
}
fn runs(styles:&[u8])->Vec<(usize,usize,u8)>{let mut out=Vec::new();let mut i=0;while i<styles.len(){let s=styles[i];let mut j=i+1;while j<styles.len()&&styles[j]==s{j+=1;}out.push((i,j,s));i=j;}out}
fn main(){
 let args:Vec<_>=std::env::args().collect();
 if args.get(1).is_some_and(|s|s=="list") {let n=unsafe{lx_count()};println!("count={n}");for i in 0..n{let mut b=[0i8;100];unsafe{lx_lexer_name(i as u32,b.as_mut_ptr(),100);println!("{}",CStr::from_ptr(b.as_ptr()).to_string_lossy());}}return;}
 for (name,path) in [("cpp","samples/example.cpp"),("python","samples/example.py"),("hypertext","samples/example.php")] {
  let text=std::fs::read(path).unwrap();let mut l=Lexer::new(name,&text);configure(&mut l,name);l.lex(0,text.len(),0,true);
  let full=l.styles().to_vec();let mut report=format!("lexer={name} bytes={}\n",text.len());
  for (a,b,s) in runs(&full){report+=&format!("{a}..{b} style={s} text={:?}\n",String::from_utf8_lossy(&text[a..b]));}
  let calls=unsafe{std::slice::from_raw_parts(lx_calls(l.p),24)};let active:Vec<_>=METHODS.iter().zip(calls).filter(|(_,n)|**n>0).map(|(s,n)|format!("{s}={n}")).collect();report+=&format!("interface calls: {}\n",active.join(", "));
  // Relex from a line boundary retaining document styles/line states.
  let pos=text.iter().position(|b|*b==b'\n').unwrap()+1;let init=full[pos-1] as i32;l.lex(pos,text.len()-pos,init,true);assert_eq!(full,l.styles(),"partial relex mismatch");
  report+="partial relex equals full lex: PASS\n";
  std::fs::write(format!("output/{name}-runs.txt"),&report).unwrap();println!("{report}");
  if name=="cpp"{let mut meta=String::from("id\tname\ttags\n");for i in 0..unsafe{lx_named(l.p)}{let(n,t)=l.metadata(i);meta+=&format!("{i}\t{n}\t{t}\n");}std::fs::write("output/cpp-styles.tsv",&meta).unwrap();println!("{meta}");}
  // Fixed valid code repeated, minimum 12 MiB. Warm run + 5 measured full lexes.
  let repeats=(12usize*1024*1024).div_ceil(text.len());let large=text.repeat(repeats);
  let t=Instant::now();let mut big=Lexer::new(name,&large);configure(&mut big,name);let setup=t.elapsed().as_secs_f64();big.lex(0,large.len(),0,false);
  let mut durations=Vec::new();for _ in 0..5{let t=Instant::now();big.lex(0,large.len(),0,false);durations.push(t.elapsed().as_secs_f64());}
  let t=Instant::now();let r=runs(big.styles());let extraction=t.elapsed().as_secs_f64();
  durations.sort_by(f64::total_cmp);let med=durations[2];let result=format!("{name}: bytes={} setup_s={setup:.6} lex_median_s={med:.6} throughput_MiB_s={:.2} runs={} extraction_s={extraction:.6} samples_s={durations:?}\n",large.len(),large.len()as f64/1048576.0/med,r.len());
  print!("{result}");use std::io::Write;std::fs::OpenOptions::new().create(true).append(true).open("output/benchmark.txt").unwrap().write_all(result.as_bytes()).unwrap();
  if name=="cpp"{std::fs::write("output/large.cpp",&large).unwrap();}
 }
}
