use std::time::Instant;
fn main(){
 let input=std::fs::read("output/large.cpp").unwrap();let mut parser=tree_sitter::Parser::new();
 parser.set_language(&tree_sitter_cpp::LANGUAGE.into()).unwrap();
 let warm=parser.parse(&input,None).unwrap();assert!(!warm.root_node().has_error(),"fixture should parse cleanly");drop(warm);
 let mut times=Vec::new();for _ in 0..3{let t=Instant::now();let tree=parser.parse(&input,None).unwrap();times.push(t.elapsed().as_secs_f64());std::hint::black_box(tree.root_node().descendant_count());drop(tree);}
 times.sort_by(f64::total_cmp);println!("tree-sitter C++ fresh full parse: bytes={} median_s={:.6} MiB_s={:.2} samples_s={times:?}",input.len(),times[1],input.len()as f64/1048576.0/times[1]);
}
