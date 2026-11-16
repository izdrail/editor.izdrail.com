//! Editable diagnostics projection core, with guarded source mappings.
//! Planning is read-only: callers must preflight every source identity/revision
//! and apply the whole plan in one UI-thread transaction, or apply nothing.
use std::{collections::BTreeMap, ops::Range, path::PathBuf};

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub path: PathBuf,
    pub buffer_id: u64,
    pub revision: u64,
    pub read_only: bool,
    pub text: String,
}
#[derive(Clone, Debug)]
pub struct Excerpt {
    pub source: PathBuf,
    pub buffer_id: u64,
    pub revision: u64,
    pub source_range: Range<usize>,
    pub display_range: Range<usize>,
}
#[derive(Clone, Debug, Default)]
pub struct Projection {
    pub text: String,
    pub excerpts: Vec<Excerpt>,
}
#[derive(Clone, Debug)]
pub struct DisplayEdit { pub range: Range<usize>, pub replacement: String }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceEdit {
    pub source: PathBuf,
    pub buffer_id: u64,
    pub revision: u64,
    pub range: Range<usize>,
    pub replacement: String,
}
fn valid(text: &str, r: &Range<usize>) -> bool {
    r.start <= r.end && r.end <= text.len()
        && text.is_char_boundary(r.start) && text.is_char_boundary(r.end)
}
fn snapshots(sources: &[Snapshot]) -> Result<BTreeMap<&PathBuf, &Snapshot>, &'static str> {
    let mut found = BTreeMap::new();
    for source in sources {
        if found.insert(&source.path, source).is_some() { return Err("Duplicate source identity"); }
    }
    Ok(found)
}
impl Projection {
    /// Callers supply already bounded, line-aligned context windows. Overlapping
    /// windows merge so the same source bytes never appear twice in this view.
    pub fn build(sources: &[Snapshot], windows: &[(PathBuf, Range<usize>)], byte_limit: usize) -> Result<Self, &'static str> {
        let found = snapshots(sources)?;
        let mut grouped: BTreeMap<&PathBuf, Vec<Range<usize>>> = BTreeMap::new();
        for (path, range) in windows {
            let source = found.get(path).ok_or("Unknown source")?;
            if !valid(&source.text, range) { return Err("Invalid source range"); }
            grouped.entry(path).or_default().push(range.clone());
        }
        let mut view = Self::default();
        for (path, mut ranges) in grouped {
            let source = found[path];
            ranges.sort_by_key(|r| (r.start, r.end));
            let mut merged: Vec<Range<usize>> = Vec::new();
            for range in ranges {
                if let Some(last) = merged.last_mut().filter(|last| range.start <= last.end) {
                    last.end = last.end.max(range.end);
                } else { merged.push(range); }
            }
            for range in merged {
                // File names are data. Escape control characters so a name
                // cannot impersonate another source header or code excerpt.
                let name: String = path.to_string_lossy().chars().flat_map(char::escape_default).collect();
                let header = format!("\n{name}\n");
                let added = header.len().checked_add(range.len()).and_then(|n| n.checked_add(1)).ok_or("Projection size overflow")?;
                if added > byte_limit.saturating_sub(view.text.len()) { return Err("Projection byte limit reached"); }
                view.text.push_str(&header);
                let start = view.text.len();
                view.text.push_str(&source.text[range.clone()]);
                let end = view.text.len();
                view.text.push('\n');
                view.excerpts.push(Excerpt { source:path.clone(), buffer_id:source.buffer_id, revision:source.revision, source_range:range, display_range:start..end });
            }
        }
        Ok(view)
    }
    /// Reject header edits, edits spanning excerpts, stale buffers, invalid
    /// UTF-8 boundaries and overlapping operations before returning any plan.
    pub fn plan(&self, current: &[Snapshot], edits: &[DisplayEdit]) -> Result<Vec<SourceEdit>, &'static str> {
        let found = snapshots(current)?;
        let mut planned = Vec::new();
        for edit in edits {
            if !valid(&self.text, &edit.range) { return Err("Invalid display range"); }
            let excerpt = self.excerpts.iter().find(|e| edit.range.start >= e.display_range.start && edit.range.end <= e.display_range.end).ok_or("Headers and excerpt boundaries are read-only")?;
            if !valid(&self.text, &excerpt.display_range) || excerpt.display_range.len() != excerpt.source_range.len() { return Err("Invalid excerpt mapping"); }
            let source = found.get(&excerpt.source).ok_or("Source was closed")?;
            if source.read_only { return Err("Source is read-only"); }
            if source.buffer_id != excerpt.buffer_id || source.revision != excerpt.revision { return Err("Source changed; refresh before editing"); }
            let range = (excerpt.source_range.start + edit.range.start - excerpt.display_range.start)..(excerpt.source_range.start + edit.range.end - excerpt.display_range.start);
            if !valid(&source.text, &range) || !valid(&source.text, &excerpt.source_range)
                || source.text[excerpt.source_range.clone()] != self.text[excerpt.display_range.clone()] { return Err("Source no longer matches this excerpt"); }
            planned.push(SourceEdit { source:source.path.clone(), buffer_id:source.buffer_id, revision:source.revision, range, replacement:edit.replacement.clone() });
        }
        planned.sort_by(|a,b| a.source.cmp(&b.source).then(a.range.start.cmp(&b.range.start)).then(a.range.end.cmp(&b.range.end)));
        for pair in planned.windows(2) {
            let (a,b)=(&pair[0],&pair[1]);
            if a.source == b.source && (a.range.end > b.range.start || a.range.start == b.range.start) { return Err("Overlapping source edits"); }
        }
        // Descending offsets within each source keep earlier positions stable.
        planned.sort_by(|a,b| a.source.cmp(&b.source).then(b.range.start.cmp(&a.range.start)));
        Ok(planned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(path:&str,text:&str)->Snapshot { Snapshot{path:path.into(),buffer_id:1,revision:7,read_only:false,text:text.into()} }
    fn edit(range:Range<usize>)->DisplayEdit { DisplayEdit{range,replacement:"fixed".into()} }
    #[test] fn merges_duplicate_overlapping_context() {
        let s=source("a.php","abcdef"); let p=Projection::build(&[s],&[("a.php".into(),0..3),("a.php".into(),2..5),("a.php".into(),5..6)],100).unwrap();
        assert_eq!(p.excerpts.len(),1);assert_eq!(p.excerpts[0].source_range,0..6);
    }
    #[test] fn maps_unicode_crlf_and_nonzero_source_offsets() {
        let s=source("a.php","old\r\nλxyz\r\n");let p=Projection::build(&[s.clone()],&[(s.path.clone(),5..12)],100).unwrap();
        let start=p.excerpts[0].display_range.start;
        let plan=p.plan(&[s],&[edit(start+2..start+3)]).unwrap();assert_eq!(plan[0].range,7..8);
    }
    #[test] fn rejects_headers_cross_excerpt_edits_and_split_unicode() {
        let s=source("a.php","λabc");let p=Projection::build(&[s.clone()],&[(s.path.clone(),0..2),(s.path.clone(),3..5)],100).unwrap();
        assert!(p.plan(&[s.clone()],&[edit(0..1)]).is_err());
        let a=&p.excerpts[0].display_range;let b=&p.excerpts[1].display_range;
        assert!(p.plan(&[s.clone()],&[edit(a.start..b.end)]).is_err());
        assert!(p.plan(&[s],&[edit(a.start+1..a.start+1)]).is_err());
    }
    #[test] fn refuses_changed_revision_reopened_buffer_and_same_revision_wrong_text() {
        let s=source("a","hello");let p=Projection::build(&[s.clone()],&[(s.path.clone(),0..5)],100).unwrap();let r=p.excerpts[0].display_range.clone();
        let mut changed=s.clone();changed.revision+=1;assert!(p.plan(&[changed],&[edit(r.clone())]).is_err());
        let mut changed=s.clone();changed.buffer_id+=1;assert!(p.plan(&[changed],&[edit(r.clone())]).is_err());
        let mut changed=s;changed.text="other".into();assert!(p.plan(&[changed],&[edit(r)]).is_err());
    }
    #[test] fn multi_file_batch_is_all_or_nothing_and_sorted_descending() {
        let sources=vec![source("a","abcd"),source("b","efgh")];let p=Projection::build(&sources,&[("a".into(),0..4),("b".into(),0..4)],100).unwrap();
        let a=p.excerpts[0].display_range.start;let b=p.excerpts[1].display_range.start;
        let plan=p.plan(&sources,&[edit(a..a+1),edit(a+3..a+4),edit(b..b+1)]).unwrap();assert_eq!(plan[0].range,3..4);assert_eq!(plan.len(),3);
        assert!(p.plan(&sources,&[edit(a..a+1),edit(0..1)]).is_err());
        assert!(p.plan(&sources,&[edit(a..a+2),edit(a+1..a+3)]).is_err());
    }
    #[test] fn empty_files_allow_insertion_but_never_header_replacement() {
        let s=source("empty","");let p=Projection::build(&[s.clone()],&[(s.path.clone(),0..0)],100).unwrap();let r=p.excerpts[0].display_range.clone();assert_eq!(p.plan(&[s],&[edit(r)]).unwrap()[0].range,0..0);
    }
    #[test] fn bounds_size_and_rejects_duplicate_sources_or_invalid_ranges() {
        let s=source("a","abc");assert!(Projection::build(&[s.clone()],&[(s.path.clone(),0..3)],1).is_err());
        assert!(Projection::build(&[s.clone(),s.clone()],&[],100).is_err());
        assert!(Projection::build(&[s.clone()],&[(s.path.clone(),0..4)],100).is_err());
    }
    #[test] fn read_only_or_closed_source_aborts_entire_batch() {
        let sources=vec![source("a","abc"),source("b","def")];let p=Projection::build(&sources,&[("a".into(),0..3),("b".into(),0..3)],100).unwrap();
        let edits=vec![edit(p.excerpts[0].display_range.clone()),edit(p.excerpts[1].display_range.clone())];
        let mut current=sources.clone();current[1].read_only=true;assert!(p.plan(&current,&edits).is_err());
        assert!(p.plan(&sources[..1],&edits).is_err());
        assert_eq!(sources[0].text,"abc");
    }
    #[test] fn hostile_names_cannot_inject_header_lines() {
        let s=source("bad\nname\t.php","x");let p=Projection::build(&[s.clone()],&[(s.path.clone(),0..1)],100).unwrap();assert!(p.text.contains("bad\\nname\\t.php"));
    }
}

/// Read-only materialization of a complete batch, used before live document
/// mutation. Normalization belongs to the native source-buffer staging step;
/// its result must match these expected strings before any buffer is published.
pub fn expected_sources(current: &[Snapshot], plan: &[SourceEdit]) -> Result<BTreeMap<PathBuf,String>, &'static str> {
    let found=snapshots(current)?;
    let mut grouped:BTreeMap<&PathBuf,Vec<&SourceEdit>>=BTreeMap::new();
    for edit in plan {
        let source=found.get(&edit.source).ok_or("Source was closed")?;
        if source.read_only || source.buffer_id != edit.buffer_id || source.revision != edit.revision || !valid(&source.text,&edit.range) { return Err("Source changed before staging"); }
        grouped.entry(&edit.source).or_default().push(edit);
    }
    let mut result=BTreeMap::new();
    for (path,mut edits) in grouped {
        edits.sort_by_key(|e|(e.range.start,e.range.end));
        for pair in edits.windows(2) {
            if pair[0].range.end>pair[1].range.start || pair[0].range.start==pair[1].range.start { return Err("Overlapping source edits"); }
        }
        let mut text=found[path].text.clone();
        for edit in edits.iter().rev() { text.replace_range(edit.range.clone(),&edit.replacement); }
        result.insert(path.clone(),text);
    }
    Ok(result)
}

/// Expand diagnostic byte offsets to whole source lines with bounded context.
/// Offsets must already have been converted from LSP UTF-16 using the canonical
/// source rope. Never use LSP character columns as raw byte offsets here.
pub fn context_windows(source: &Snapshot, diagnostics: &[Range<usize>], context_lines: usize, max_windows: usize) -> Result<Vec<Range<usize>>, &'static str> {
    if diagnostics.len()>max_windows { return Err("Too many diagnostic windows"); }
    let mut starts=vec![0];
    for (i,byte) in source.text.bytes().enumerate() { if byte==b'\n' {starts.push(i+1);} }
    let mut windows=Vec::new();
    for range in diagnostics {
        if !valid(&source.text,range) {return Err("Invalid diagnostic byte range");}
        let first=starts.partition_point(|offset|*offset<=range.start).saturating_sub(1);
        // A range ending exactly at the next line's start does not include that line.
        let last_offset=if range.end>range.start {range.end-1}else{range.end};
        let last=starts.partition_point(|offset|*offset<=last_offset).saturating_sub(1);
        let from=starts[first.saturating_sub(context_lines)];
        let to=starts.get(last.saturating_add(context_lines).saturating_add(1)).copied().unwrap_or(source.text.len());
        windows.push(from..to);
    }
    windows.sort_by_key(|r|(r.start,r.end));
    let mut merged:Vec<Range<usize>>=Vec::new();
    for window in windows {
        if let Some(last)=merged.last_mut().filter(|last|window.start<=last.end) {last.end=last.end.max(window.end);} else {merged.push(window);}
    }
    Ok(merged)
}

#[cfg(test)]
mod materialization_tests {
    use super::*;
    fn snapshot(path:&str,text:&str)->Snapshot {Snapshot{path:path.into(),buffer_id:9,revision:13,read_only:false,text:text.into()}}
    #[test] fn materializes_multiple_files_without_mutating_current() {
        let sources=vec![snapshot("a","abcdef"),snapshot("b","λxy")];
        let p=Projection::build(&sources,&[("a".into(),0..6),("b".into(),0..4)],100).unwrap();
        let a=p.excerpts[0].display_range.start;let b=p.excerpts[1].display_range.start;
        let plan=p.plan(&sources,&[DisplayEdit{range:a+1..a+3,replacement:"BIG".into()},DisplayEdit{range:a+5..a+6,replacement:"".into()},DisplayEdit{range:b..b+2,replacement:"L".into()}]).unwrap();
        let result=expected_sources(&sources,&plan).unwrap();
        assert_eq!(result[&PathBuf::from("a")],"aBIGde");assert_eq!(result[&PathBuf::from("b")],"Lxy");assert_eq!(sources[0].text,"abcdef");
    }
    #[test] fn stale_second_source_produces_no_partial_result() {
        let sources=vec![snapshot("a","a"),snapshot("b","b")];let p=Projection::build(&sources,&[("a".into(),0..1),("b".into(),0..1)],100).unwrap();
        let edits:Vec<_>=p.excerpts.iter().map(|e|DisplayEdit{range:e.display_range.clone(),replacement:"z".into()}).collect();let plan=p.plan(&sources,&edits).unwrap();
        let mut current=sources.clone();current[1].revision+=1;assert!(expected_sources(&current,&plan).is_err());assert_eq!(current[0].text,"a");
    }
    #[test] fn source_context_merges_and_handles_crlf_unicode_eof() {
        let source=snapshot("a","a\r\nλ\r\nc\r\nd");
        assert_eq!(context_windows(&source,&[3..5],0,10).unwrap(),vec![3..7]);
        assert_eq!(context_windows(&source,&[3..5,7..8],1,10).unwrap(),vec![0..11]);
        assert_eq!(context_windows(&source,&[11..11],0,10).unwrap(),vec![10..11]);
        assert!(context_windows(&source,&[4..5],0,10).is_err());
        assert!(context_windows(&source,&[0..1],0,0).is_err());
    }
    #[test] fn zero_length_empty_file_diagnostic_produces_editable_window() {
        let source=snapshot("a","");assert_eq!(context_windows(&source,&[0..0],3,1).unwrap(),vec![0..0]);
    }
}

/// Stage native source buffers without touching their live revision clocks.
/// The canonical map must be captured from MainSplit.docs in the same UI turn
/// as this call and publication. Never pass retained, closed document objects.
pub(crate) fn stage_native(
    current_docs: &std::collections::BTreeMap<PathBuf, std::rc::Rc<crate::doc::Doc>>,
    plan: &[SourceEdit],
) -> Result<crate::buffer_transaction::StagedBuffers<PathBuf>, String> {
    use floem::reactive::SignalWith;
    use lapce_core::buffer::rope_text::RopeText;
    if plan.len()>1024 {return Err("Too many source edits".into());}
    let inserted=plan.iter().try_fold(0usize,|n,e|n.checked_add(e.replacement.len())).ok_or("Edit size overflow")?;
    if inserted>4*1024*1024 {return Err("Source edits exceed the insertion budget".into());}
    let mut current=Vec::new();
    let mut source_bytes=0usize;
    let paths:std::collections::BTreeSet<_>=plan.iter().map(|edit|edit.source.clone()).collect();
    if paths.len()>128 {return Err("Too many source documents".into());}
    for path in paths {
        let doc=current_docs.get(&path).ok_or("Source was closed")?;
        if !doc.loaded() {return Err("Source is still loading".into());}
        let len=doc.buffer.with_untracked(|b|b.len());
        source_bytes=source_bytes.checked_add(len).ok_or("Source size overflow")?;
        if len>4*1024*1024 || source_bytes>16*1024*1024 {return Err("Source snapshots exceed the edit budget".into());}
        let read_only=doc.content.with_untracked(|c| {
            if c.path() != Some(&path) { return None; }
            Some(c.read_only())
        }).ok_or("Source identity no longer matches its path")?;
        current.push(Snapshot{path,buffer_id:doc.buffer_id.0,revision:doc.rev(),read_only,text:doc.buffer.with_untracked(|b|b.to_string())});
    }
    let expected=expected_sources(&current,plan)?;
    let mut staged=std::collections::BTreeMap::new();
    for (path,expected_text) in expected {
        let edits:Vec<_>=plan.iter().filter(|e|e.source==path).collect();
        let candidate=current_docs[&path].buffer.with_untracked(|b|stage_one(b,&edits,&expected_text))?;
        staged.insert(path,candidate);
    }
    Ok(staged)
}

fn stage_one(live:&lapce_core::buffer::Buffer, edits:&[&SourceEdit], expected_text:&str) -> Result<(lapce_core::buffer::Buffer,(lapce_xi_rope::Rope,lapce_xi_rope::RopeDelta,lapce_core::buffer::InvalLines)),String> {
    use lapce_core::{editor::EditType,selection::Selection};
    let mut buffer=live.detached_edit_clone();
    let edits:Vec<_>=edits.iter().map(|e|(Selection::region(e.range.start,e.range.end),e.replacement.as_str())).collect();
    let delta=buffer.edit(&edits,EditType::Other);
    if buffer.to_string()!=expected_text {return Err("Source line-ending normalization changed the planned edit".into());}
    Ok((buffer,delta))
}

#[cfg(test)]
mod native_staging_tests {
    use super::*;
    use lapce_core::{buffer::Buffer, editor::EditType, selection::Selection};
    use std::sync::atomic::Ordering;
    #[test]
    fn source_batch_preview_is_detached_and_keeps_native_undo_history() {
        let mut live=Buffer::new("abc");
        live.edit(&[(Selection::caret(3),"d")],EditType::InsertChars);
        let clock=live.atomic_rev();let base_rev=live.rev();
        let current=vec![Snapshot{path:"a".into(),buffer_id:4,revision:base_rev,read_only:false,text:live.to_string()}];
        let projection=Projection::build(&current,&[("a".into(),0..4)],100).unwrap();
        let display=projection.excerpts[0].display_range.start;
        let plan=projection.plan(&current,&[DisplayEdit{range:display+1..display+2,replacement:"X".into()},DisplayEdit{range:display+3..display+4,replacement:"Y".into()}]).unwrap();
        let expected=expected_sources(&current,&plan).unwrap();
        let edits:Vec<_>=plan.iter().collect();
        let (mut staged,_)=stage_one(&live,&edits,&expected[&PathBuf::from("a")]).unwrap();
        assert_eq!(live.to_string(),"abcd");assert_eq!(clock.load(Ordering::Acquire),base_rev);
        assert_eq!(staged.to_string(),"aXcY");staged.adopt_revision_clock(&live);live=staged;
        assert_eq!(clock.load(Ordering::Acquire),live.rev());live.do_undo();assert_eq!(live.to_string(),"abcd");live.do_undo();assert_eq!(live.to_string(),"abc");
    }
    #[test]
    fn normalization_mismatch_cannot_publish_or_advance_live_clock() {
        let live=Buffer::new("a\r\nb\r\n");let clock=live.atomic_rev();let original=clock.load(Ordering::Acquire);
        let plan=SourceEdit{source:"a".into(),buffer_id:4,revision:live.rev(),range:1..1,replacement:"\n".into()};
        assert!(stage_one(&live,&[&plan],"a\n\r\nb\r\n").is_err());
        assert_eq!(clock.load(Ordering::Acquire),original);assert_eq!(live.to_string(),"a\r\nb\r\n");
    }
}

/// A cursor tied to source identity, never to an aggregate scratch-file offset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceAnchor {
    pub source: PathBuf,
    pub buffer_id: u64,
    pub revision: u64,
    pub offset: usize,
}
impl Projection {
    pub fn anchor(&self, display: usize) -> Option<SourceAnchor> {
        if !self.text.is_char_boundary(display) {return None;}
        let excerpt=self.excerpts.iter().find(|e|e.display_range.contains(&display) || display==e.display_range.end)?;
        Some(SourceAnchor{source:excerpt.source.clone(),buffer_id:excerpt.buffer_id,revision:excerpt.revision,offset:excerpt.source_range.start+display-excerpt.display_range.start})
    }
    pub fn display_offset(&self, anchor:&SourceAnchor)->Option<usize> {
        let excerpt=self.excerpts.iter().find(|e|e.source==anchor.source && e.buffer_id==anchor.buffer_id && e.revision==anchor.revision && anchor.offset>=e.source_range.start && anchor.offset<=e.source_range.end)?;
        let display=excerpt.display_range.start+anchor.offset-excerpt.source_range.start;
        self.text.is_char_boundary(display).then_some(display)
    }
    /// Keep the existing context windows after a committed edit. Diagnostics
    /// can refresh separately; they must not remove the actively edited excerpt.
    /// `after` must be the actual staged native snapshots, including revisions.
    pub fn after_edit(&self, before:&[Snapshot], after:&[Snapshot], plan:&[SourceEdit], byte_limit:usize)->Result<Self,&'static str> {
        let old=snapshots(before)?;let new=snapshots(after)?;
        let expected=expected_sources(before,plan)?;
        for (path,text) in &expected {
            let source=new.get(path).ok_or("Source disappeared after staging")?;
            let previous=old.get(path).ok_or("Missing source before staging")?;
            if source.buffer_id!=previous.buffer_id || source.revision<=previous.revision || &source.text!=text {return Err("Staged source does not match planned edit");}
        }
        let mut windows=Vec::new();
        for excerpt in &self.excerpts {
            let previous=old.get(&excerpt.source).ok_or("Missing excerpt source")?;
            let next=new.get(&excerpt.source).ok_or("Source disappeared after staging")?;
            if previous.buffer_id!=excerpt.buffer_id || previous.revision!=excerpt.revision || next.buffer_id!=excerpt.buffer_id {return Err("Excerpt identity changed");}
            if !expected.contains_key(&excerpt.source) && (next.revision!=previous.revision || next.text!=previous.text) {return Err("Unrelated source changed during staging");}
            let relevant:Vec<_>=plan.iter().filter(|e|e.source==excerpt.source).collect();
            let from=transform_offset(excerpt.source_range.start,&relevant,false)?;
            let to=transform_offset(excerpt.source_range.end,&relevant,true)?;
            windows.push((excerpt.source.clone(),from..to));
        }
        Self::build(after,&windows,byte_limit)
    }
}
/// Transform an old source offset through one simultaneous source edit batch.
/// Header/excerpt starts have left affinity; ends/carets may have right affinity.
fn transform_offset(offset:usize,edits:&[&SourceEdit],right:bool)->Result<usize,&'static str> {
    let mut sorted=edits.to_vec();sorted.sort_by_key(|edit|(edit.range.start,edit.range.end));
    let mut result=offset;
    for edit in sorted {
        if offset<edit.range.start {break;}
        if offset>edit.range.end || (offset==edit.range.end && edit.range.start!=edit.range.end) {
            result=result.checked_sub(edit.range.len()).and_then(|n|n.checked_add(edit.replacement.len())).ok_or("Anchor offset overflow")?;
        } else {
            result=result.checked_sub(offset-edit.range.start).ok_or("Anchor offset overflow")?;
            if right {result=result.checked_add(edit.replacement.len()).ok_or("Anchor offset overflow")?;}
            return Ok(result);
        }
    }
    Ok(result)
}
pub fn rebase_anchor(anchor:&SourceAnchor, after:&Snapshot, plan:&[SourceEdit],right:bool)->Result<SourceAnchor,&'static str> {
    if anchor.source!=after.path || anchor.buffer_id!=after.buffer_id {return Err("Anchor source identity changed");}
    let edits:Vec<_>=plan.iter().filter(|e|e.source==anchor.source).collect();
    if edits.iter().any(|e|e.buffer_id!=anchor.buffer_id || e.revision!=anchor.revision) {return Err("Anchor revision changed");}
    if !edits.is_empty() && after.revision<=anchor.revision {return Err("Source revision did not advance");}
    if edits.is_empty() && after.revision!=anchor.revision {return Err("Unrelated source changed");}
    let offset=transform_offset(anchor.offset,&edits,right)?;
    if offset>after.text.len() || !after.text.is_char_boundary(offset) {return Err("Invalid rebased anchor");}
    Ok(SourceAnchor{source:anchor.source.clone(),buffer_id:anchor.buffer_id,revision:after.revision,offset})
}
#[cfg(test)]
mod anchor_tests {
    use super::*;
    fn source()->Snapshot {Snapshot{path:"a.php".into(),buffer_id:8,revision:4,read_only:false,text:"head\nλabc\ntail\n".into()}}
    #[test]fn roundtrip_anchor_rejects_headers_and_split_unicode() {
        let s=source();let p=Projection::build(&[s.clone()],&[(s.path.clone(),5..11)],100).unwrap();let start=p.excerpts[0].display_range.start;
        assert!(p.anchor(0).is_none());assert!(p.anchor(start+1).is_none());
        for offset in [start,start+2,start+5,start+6] {let anchor=p.anchor(offset).unwrap();assert_eq!(p.display_offset(&anchor),Some(offset));}
    }
    #[test]fn insertion_keeps_end_of_excerpt_and_cursor_source_location() {
        let s=source();let p=Projection::build(&[s.clone()],&[(s.path.clone(),5..11)],100).unwrap();let end=p.excerpts[0].display_range.end;let anchor=p.anchor(end).unwrap();
        let plan=p.plan(&[s.clone()],&[DisplayEdit{range:end..end,replacement:"new\n".into()}]).unwrap();let texts=expected_sources(&[s.clone()],&plan).unwrap();
        let mut after=s.clone();after.text=texts[&s.path].clone();after.revision+=1;
        let p2=p.after_edit(&[s],&[after.clone()],&plan,100).unwrap();assert_eq!(p2.excerpts[0].source_range,5..15);
        let anchor=rebase_anchor(&anchor,&after,&plan,true).unwrap();assert_eq!(anchor.offset,15);assert_eq!(p2.display_offset(&anchor),Some(p2.excerpts[0].display_range.end));
    }
    #[test]fn edits_shift_later_windows_without_dropping_them() {
        let s=source();let p=Projection::build(&[s.clone()],&[(s.path.clone(),0..5),(s.path.clone(),11..16)],100).unwrap();
        let start=p.excerpts[0].display_range.start;let plan=p.plan(&[s.clone()],&[DisplayEdit{range:start..start+4,replacement:"H".into()}]).unwrap();
        let mut after=s.clone();after.text=expected_sources(&[s.clone()],&plan).unwrap()[&s.path].clone();after.revision+=1;
        let p2=p.after_edit(&[s],&[after],&plan,100).unwrap();assert_eq!(p2.excerpts.len(),2);assert_eq!(p2.excerpts[1].source_range,8..13);assert!(p2.text.contains("tail"));
    }
    #[test]fn projection_preflight_rejects_unrelated_external_change() {
        let s=source();let p=Projection::build(&[s.clone()],&[(s.path.clone(),0..5)],100).unwrap();let mut after=s.clone();after.revision+=1;
        assert!(p.after_edit(&[s],&[after],&[],100).is_err());
    }
}

/// Convert one native rope delta to edits in its original display coordinate
/// space. Keep separate replacements separate so a multi-cursor edit cannot be
/// flattened into one replacement spanning protected excerpt headers.
pub fn display_edits_from_delta(base:&str,delta:&lapce_xi_rope::RopeDelta)->Result<Vec<DisplayEdit>, &'static str> {
    use lapce_xi_rope::DeltaElement;
    if delta.base_len!=base.len() {return Err("Projection revision changed before editing");}
    let mut edits=Vec::new();let mut old=0usize;let mut inserted=String::new();let mut anchor=0usize;
    for element in &delta.els {
        match element {
            DeltaElement::Copy(start,end)=>{
                if *start<old || start>end || !valid(base,&(*start..*end)) {return Err("Invalid projection delta");}
                if *start!=anchor || !inserted.is_empty() {
                    edits.push(DisplayEdit{range:anchor..*start,replacement:std::mem::take(&mut inserted)});
                }
                old=*end;anchor=*end;
            }
            DeltaElement::Insert(text)=>{
                if edits.len()>1024 || inserted.len().saturating_add(text.len())>4*1024*1024 {return Err("Projection delta exceeds edit budget");}
                inserted.push_str(&text.to_string());
            }
        }
    }
    if old>base.len() {return Err("Invalid projection delta");}
    if anchor!=base.len() || !inserted.is_empty() {edits.push(DisplayEdit{range:anchor..base.len(),replacement:inserted});}
    if edits.len()>1024 {return Err("Too many projection edits");}
    Ok(edits)
}
#[cfg(test)]
mod display_delta_tests {
    use super::*;
    use lapce_xi_rope::{DeltaBuilder,Rope};
    #[test]fn disjoint_native_edits_do_not_consume_headers() {
        let base="abc\nHEADER\ndef";
        let mut builder=DeltaBuilder::new(base.len());builder.replace(1..2,Rope::from("BIG"));builder.replace(12..13,Rope::from("X"));let delta=builder.build();
        let edits=display_edits_from_delta(base,&delta).unwrap();assert_eq!(edits.len(),2);assert_eq!(edits[0].range,1..2);assert_eq!(edits[1].range,12..13);
        assert_eq!(edits[0].replacement,"BIG");assert_eq!(edits[1].replacement,"X");
    }
    #[test]fn deletions_insertions_empty_buffers_and_noops() {
        for (base,range,replacement) in [("abc",0..3,""),("",0..0,"λ"),("abc",3..3,"!"),("abc",0..0,"!")] {
            let delta=lapce_xi_rope::RopeDelta::simple_edit(range.clone(),Rope::from(replacement),base.len());let edits=display_edits_from_delta(base,&delta).unwrap();assert_eq!(edits.len(),1);assert_eq!(edits[0].range,range);assert_eq!(edits[0].replacement,replacement);
        }
        let delta=DeltaBuilder::new(3).build();assert!(display_edits_from_delta("abc",&delta).unwrap().is_empty());
    }
}

/// A fully staged user edit. Construct and publish it in one UI callback, with
/// no await or event-loop yield between canonical snapshot capture and commit.
/// The future UI owns save/undo commands; the projection is never a disk file.
pub(crate) struct PreparedProjectionEdit {
    pub projection: Projection,
    pub staged: crate::buffer_transaction::StagedBuffers<PathBuf>,
    pub cursor: Option<usize>,
}
pub(crate) fn prepare_projection_edit(
    projection:&Projection,
    docs:&BTreeMap<PathBuf,std::rc::Rc<crate::doc::Doc>>,
    delta:&lapce_xi_rope::RopeDelta,
    old_cursor:usize,
    byte_limit:usize,
)->Result<PreparedProjectionEdit,String> {
    use floem::reactive::SignalWith;
    use lapce_core::buffer::rope_text::RopeText;
    let mut before=Vec::new();let mut total=0usize;
    let paths:std::collections::BTreeSet<_>=projection.excerpts.iter().map(|e|e.source.clone()).collect();
    if paths.len()>128 {return Err("Too many source documents".into());}
    for path in paths {
        let doc=docs.get(&path).ok_or("Source was closed")?;
        if !doc.loaded() {return Err("Source is still loading".into());}
        let len=doc.buffer.with_untracked(|b|b.len());total=total.checked_add(len).ok_or("Source size overflow")?;
        if len>4*1024*1024 || total>16*1024*1024 {return Err("Source snapshots exceed the edit budget".into());}
        let read_only=doc.content.with_untracked(|c| (c.path()==Some(&path)).then(||c.read_only())).ok_or("Source identity no longer matches its path")?;
        before.push(Snapshot{path,buffer_id:doc.buffer_id.0,revision:doc.rev(),read_only,text:doc.buffer.with_untracked(|b|b.to_string())});
    }
    let display_edits=display_edits_from_delta(&projection.text,delta)?;
    let plan=projection.plan(&before,&display_edits)?;
    let staged=stage_native(docs,&plan)?;
    let mut after=before.clone();
    for source in &mut after {
        if let Some((buffer,_))=staged.get(&source.path) {source.revision=buffer.rev();source.text=buffer.to_string();}
    }
    let next=projection.after_edit(&before,&after,&plan,byte_limit)?;
    if delta.apply(&lapce_xi_rope::Rope::from(projection.text.as_str())).to_string()!=next.text {return Err("Native edit changed protected projection metadata".into());}
    let cursor=projection.anchor(old_cursor).and_then(|anchor| {
        let source=after.iter().find(|s|s.path==anchor.source)?;
        rebase_anchor(&anchor,source,&plan,true).ok().and_then(|anchor|next.display_offset(&anchor))
    });
    Ok(PreparedProjectionEdit{projection:next,staged,cursor})
}

#[derive(Clone)]
pub(crate) struct ProjectionBinding {
    pub projection: std::rc::Rc<std::cell::RefCell<Projection>>,
    pub docs: floem::reactive::RwSignal<im::HashMap<PathBuf,std::rc::Rc<crate::doc::Doc>>>,
    pub notice: floem::reactive::RwSignal<String>,
}
/// Clip source-relative lexer runs into one visible projection segment.
/// Both ranges use byte offsets in their respective lines, not UTF-16 columns.
pub(crate) fn projected_line_styles(
    source: &[lapce_rpc::style::LineStyle],
    source_columns: std::ops::Range<usize>,
    display_column: usize,
) -> Vec<lapce_rpc::style::LineStyle> {
    source.iter().filter_map(|run| {
        let start=run.start.max(source_columns.start);
        let end=run.end.min(source_columns.end);
        if start>=end {return None;}
        Some(lapce_rpc::style::LineStyle {
            start:display_column+start-source_columns.start,
            end:display_column+end-source_columns.start,
            style:run.style.clone(),
        })
    }).collect()
}
#[cfg(test)]
mod projected_style_tests {
    use super::*;
    use lapce_rpc::style::{LineStyle,Style};
    #[test] fn clips_runs_and_keeps_style_identity() {
        let source=vec![LineStyle{start:0,end:4,style:Style{fg_color:Some("keyword".into())}},LineStyle{start:4,end:8,style:Style{fg_color:Some("string".into())}}];
        let result=projected_line_styles(&source,2..6,10);
        assert_eq!(result.len(),2);assert_eq!((result[0].start,result[0].end),(10,12));assert_eq!((result[1].start,result[1].end),(12,14));assert_eq!(result[1].style,source[1].style);
    }
    #[test] fn excludes_empty_and_outside_runs() {
        let source=vec![LineStyle{start:2,end:5,style:Style{fg_color:None}}];
        assert!(projected_line_styles(&source,5..8,0).is_empty());
        assert!(projected_line_styles(&source,0..2,0).is_empty());
        assert!(projected_line_styles(&source,3..3,0).is_empty());
    }
}

impl Projection {
    /// Map only actual code-line starts. Headers and separator rows have no
    /// source number, even when an excerpt ends at a source newline.
    pub(crate) fn source_line_anchor(&self, display:usize)->Option<SourceAnchor> {
        if display>self.text.len() || (display>0 && self.text.as_bytes().get(display-1)!=Some(&b'\n')) {return None;}
        let excerpt=self.excerpts.iter().find(|e|e.display_range.contains(&display) || (e.display_range.is_empty() && display==e.display_range.start))?;
        self.anchor(excerpt.display_range.start.max(display))
    }
}
#[cfg(test)]
mod projected_line_number_tests {
    use super::*;
    fn build(text:&str,range:Range<usize>)->Projection {
        let source=Snapshot{path:"test.php".into(),buffer_id:12,revision:5,read_only:false,text:text.into()};
        Projection::build(&[source],&[("test.php".into(),range)],1000).unwrap()
    }
    #[test] fn source_offsets_preserve_nonzero_windows_and_unicode_crlf() {
        let text="omitted\r\nλx\r\ny\r\n";let start=text.find('λ').unwrap();let p=build(text,start..text.len());let display=p.excerpts[0].display_range.start;
        let a=p.source_line_anchor(display).unwrap();assert_eq!(a.offset,start);assert_eq!((a.buffer_id,a.revision),(12,5));
        assert_eq!(p.source_line_anchor(display+5).unwrap().offset,start+5);
        assert!(p.source_line_anchor(display+2).is_none());
    }
    #[test] fn headers_separators_and_out_of_range_have_no_source_number() {
        let p=build("code\n",0..5);let e=&p.excerpts[0];
        assert!(p.source_line_anchor(0).is_none());assert!(p.source_line_anchor(1).is_none());
        assert!(p.source_line_anchor(e.display_range.end).is_none());assert!(p.source_line_anchor(p.text.len()+1).is_none());
    }
    #[test] fn empty_and_unterminated_source_lines_remain_numbered() {
        let p=build("",0..0);assert_eq!(p.source_line_anchor(p.excerpts[0].display_range.start).unwrap().offset,0);
        let p=build("x",0..1);assert_eq!(p.source_line_anchor(p.excerpts[0].display_range.start).unwrap().offset,0);
    }
}

impl Projection {
    /// Navigation is valid inside code bytes, never at a separator that also
    /// happens to equal an excerpt's insertion boundary.
    pub(crate) fn navigation_anchor(&self,display:usize)->Option<SourceAnchor> {
        self.excerpts.iter().find(|e|e.display_range.contains(&display))?;
        self.anchor(display)
    }
}
#[cfg(test)]
mod projection_navigation_tests {
    use super::*;
    #[test] fn maps_code_with_original_identity_and_utf8_byte_offset() {
        let s=Snapshot{path:"a.php".into(),buffer_id:8,revision:9,read_only:false,text:"omit\nλname\r\n".into()};
        let p=Projection::build(&[s],&[("a.php".into(),5..13)],100).unwrap();let at=p.excerpts[0].display_range.start;
        let a=p.navigation_anchor(at+2).unwrap();assert_eq!(a.offset,7);assert_eq!(a.source,PathBuf::from("a.php"));assert_eq!((a.buffer_id,a.revision),(8,9));
        assert!(p.navigation_anchor(at+1).is_none());
    }
    #[test] fn headers_and_separator_boundaries_never_become_lsp_paths() {
        let s=Snapshot{path:"a.php".into(),buffer_id:8,revision:9,read_only:false,text:"abc\n".into()};
        let p=Projection::build(&[s],&[("a.php".into(),0..4)],100).unwrap();
        assert!(p.navigation_anchor(0).is_none());assert!(p.navigation_anchor(1).is_none());assert!(p.navigation_anchor(p.excerpts[0].display_range.end).is_none());assert!(p.navigation_anchor(p.text.len()).is_none());
    }
}

/// Compose sequential native deltas without collapsing unchanged header bytes
/// into a replacement. Every intermediate action must itself stay in code.
pub(crate) fn compose_projection_deltas(
    projection:&Projection,
    deltas:&[lapce_xi_rope::RopeDelta],
)->Result<lapce_xi_rope::RopeDelta,&'static str> {
    use lapce_xi_rope::{DeltaElement,RopeDelta};
    if deltas.is_empty() || deltas.len()>128 {return Err("Invalid compound edit count");}
    let mut pieces=vec![DeltaElement::Copy(0,projection.text.len())];
    let mut text=projection.text.clone();let mut ranges:Vec<_>=projection.excerpts.iter().map(|e|e.display_range.clone()).collect();
    for delta in deltas {
        let edits=display_edits_from_delta(&text,delta)?;
        for edit in &edits {
            if !ranges.iter().any(|range|edit.range.start>=range.start && edit.range.end<=range.end) {
                return Err("Compound edit touches protected excerpt metadata");
            }
        }
        let mut next=Vec::new();
        for element in &delta.els {
            match element {
                DeltaElement::Insert(rope)=>next.push(DeltaElement::Insert(rope.clone())),
                DeltaElement::Copy(from,to)=> {
                    let mut at=0usize;
                    for piece in &pieces {
                        let len=match piece {DeltaElement::Copy(a,b)=>b-a,DeltaElement::Insert(rope)=>rope.len()};
                        let begin=(*from).max(at);let end=(*to).min(at+len);
                        if begin<end {
                            next.push(match piece {
                                DeltaElement::Copy(a,_)=>DeltaElement::Copy(a+begin-at,a+end-at),
                                DeltaElement::Insert(rope)=>DeltaElement::Insert(rope.subseq(begin-at..end-at)),
                            });
                        }
                        at+=len;if at>=*to {break;}
                    }
                }
            }
            if next.len()>16384 {return Err("Compound edit exceeds piece budget");}
        }
        // Each edit is checked in that stage's coordinates, so an action that
        // changes then restores a header still fails rather than hiding it.
        for range in &mut ranges {
            let mut transformer=lapce_xi_rope::Transformer::new(delta);
            range.start=transformer.transform(range.start,false);
            range.end=transformer.transform(range.end,true);
        }
        let next_len=delta.new_document_len();
        if next_len>4*1024*1024 {return Err("Compound edit exceeds projection budget");}
        text=delta.apply(&lapce_xi_rope::Rope::from(text.as_str())).to_string();pieces=next;
    }
    let result=RopeDelta{base_len:projection.text.len(),els:pieces};
    if result.apply(&lapce_xi_rope::Rope::from(projection.text.as_str())).to_string()!=text {return Err("Compound edit composition mismatch");}
    Ok(result)
}
#[cfg(test)]
mod compound_projection_tests {
    use super::*;
    use lapce_xi_rope::{Rope,RopeDelta};
    fn fixture()->(Projection,Vec<Snapshot>) {
        let sources=vec![Snapshot{path:"a".into(),buffer_id:1,revision:1,read_only:false,text:"λabc\n".into()},Snapshot{path:"b".into(),buffer_id:2,revision:1,read_only:false,text:"def\n".into()}];
        let p=Projection::build(&sources,&[("a".into(),0..6),("b".into(),0..4)],100).unwrap();(p,sources)
    }
    #[test] fn composes_insert_then_edit_inserted_text_without_consuming_headers() {
        let (p,s)=fixture();let at=p.excerpts[0].display_range.start+2;
        let a=RopeDelta::simple_edit(at..at,Rope::from("XY"),p.text.len());
        let b=RopeDelta::simple_edit(at+1..at+2,Rope::from("Z"),a.new_document_len());
        let delta=compose_projection_deltas(&p,&[a,b]).unwrap();let edits=display_edits_from_delta(&p.text,&delta).unwrap();let plan=p.plan(&s,&edits).unwrap();assert_eq!(plan.len(),1);assert_eq!(plan[0].range,2..2);assert_eq!(plan[0].replacement,"XZ");
    }
    #[test] fn disjoint_sources_and_sequential_deletions_keep_metadata() {
        let (p,s)=fixture();let a=p.excerpts[0].display_range.start+2;let b=p.excerpts[1].display_range.start;
        let first=RopeDelta::simple_edit(a..a+1,Rope::from(""),p.text.len());
        let second=RopeDelta::simple_edit(b-1..b,Rope::from("!"),first.new_document_len());
        let delta=compose_projection_deltas(&p,&[first,second]).unwrap();let edits=display_edits_from_delta(&p.text,&delta).unwrap();let plan=p.plan(&s,&edits).unwrap();assert_eq!(plan.len(),2);
    }
    #[test] fn header_edits_cannot_be_laundered_by_restoring_them_later() {
        let (p,_)=fixture();let first=RopeDelta::simple_edit(1..2,Rope::from("X"),p.text.len());let second=RopeDelta::simple_edit(1..2,Rope::from("a"),p.text.len());assert!(compose_projection_deltas(&p,&[first,second]).is_err());
    }
    #[test] fn rejects_wrong_lengths_cross_excerpt_changes_and_split_utf8() {
        let (p,_)=fixture();let at=p.excerpts[0].display_range.start;
        for delta in [RopeDelta::simple_edit(at..at,Rope::from("x"),1),RopeDelta::simple_edit(at..p.excerpts[1].display_range.start,Rope::from("x"),p.text.len()),RopeDelta::simple_edit(at+1..at+1,Rope::from("x"),p.text.len())] {assert!(compose_projection_deltas(&p,&[delta]).is_err());}
    }
}

/// Publish a compound action with one aggregate undo entry, matching the one
/// canonical-source transaction rather than exposing intermediate revisions.
pub(crate) fn stage_compound_projection(
    live:&lapce_core::buffer::Buffer,
    composed:&lapce_xi_rope::RopeDelta,
    expected:&str,
)->Result<(lapce_core::buffer::Buffer,(lapce_xi_rope::Rope,lapce_xi_rope::RopeDelta,lapce_core::buffer::InvalLines)),&'static str> {
    use lapce_core::{buffer::rope_text::RopeText,selection::Selection,editor::EditType};
    let edits=display_edits_from_delta(&live.to_string(),composed)?;
    let edits:Vec<_>=edits.iter().map(|edit|(Selection::region(edit.range.start,edit.range.end),edit.replacement.as_str())).collect();
    let mut candidate=live.detached_edit_clone();let delta=candidate.edit(&edits,EditType::Other);
    if candidate.to_string()!=expected {return Err("Compound projection line endings changed during staging");}
    Ok((candidate,delta))
}
#[cfg(test)]
mod compound_undo_tests {
    use super::*;
    use lapce_core::buffer::{Buffer,rope_text::RopeText};
    use lapce_xi_rope::{Rope,RopeDelta};
    #[test] fn one_undo_redo_covers_all_sequential_deltas_without_touching_live_clock() {
        let source=Snapshot{path:"a".into(),buffer_id:1,revision:1,read_only:false,text:"{}\n".into()};
        let p=Projection::build(&[source],&[("a".into(),0..3)],100).unwrap();let at=p.excerpts[0].display_range.start+1;
        let live=Buffer::new(&p.text);let revision=live.rev();
        let a=RopeDelta::simple_edit(at..at,Rope::from("\n"),p.text.len());let b=RopeDelta::simple_edit(at+1..at+1,Rope::from("\n"),a.new_document_len());
        let composed=compose_projection_deltas(&p,&[a,b]).unwrap();let expected=composed.apply(&Rope::from(p.text.as_str())).to_string();
        let (mut candidate,_)=stage_compound_projection(&live,&composed,&expected).unwrap();assert_eq!(live.rev(),revision);assert_eq!(live.to_string(),p.text);
        candidate.do_undo().unwrap();assert_eq!(candidate.to_string(),p.text);candidate.do_redo().unwrap();assert_eq!(candidate.to_string(),expected);
    }
}
