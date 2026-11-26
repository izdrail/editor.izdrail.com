//! Bounded, cached local PHP analysis. Built with the document's existing AST
//! on the syntax worker. No project I/O, parser, runtime or UI work here.
use crate::{cancelled, Error};
use lapce_xi_rope::Rope;
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
    sync::{atomic::AtomicUsize, Arc},
};
use tree_sitter::Node;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confidence {
    Declared,
    Documented,
    Inferred,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Type {
    pub name: String,
    pub confidence: Confidence,
}
#[derive(Clone, Debug)]
pub struct Binding {
    pub name: String,
    pub range: Range<usize>,
    pub ty: Option<Type>,
    pub parameter: bool,
    pub captured: bool,
    pub available: usize,
}
#[derive(Clone, Debug)]
pub struct Notice {
    pub range: Range<usize>,
    pub code: &'static str,
    pub message: String,
}
#[derive(Clone, Debug)]
pub struct Scope {
    pub range: Range<usize>,
    pub parent: Option<usize>,
    pub bindings: Vec<Binding>,
    pub return_type: Option<Type>,
    pub diagnostics: Vec<Notice>,
    pub dynamic: bool,
    signature: u64,
}
#[derive(Clone, Debug, Default)]
pub struct Analysis {
    pub scopes: Vec<Arc<Scope>>,
}
fn text(n: Node, rope: &Rope) -> String {
    rope.slice_to_cow(n.byte_range()).into_owned()
}
fn is_scope(n: Node) -> bool {
    matches!(
        n.kind(),
        "function_definition"
            | "method_declaration"
            | "anonymous_function"
            | "arrow_function"
    )
}
fn child_nodes(n: Node) -> Vec<Node> {
    let mut c = n.walk();
    n.named_children(&mut c).collect()
}
fn type_text(raw: &str, confidence: Confidence) -> Option<Type> {
    let name = raw.trim().trim_start_matches('?');
    // Unions, generics, arrays and intersections are deliberately unknown to
    // member completion until a proper join can establish an owner.
    (!name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '\\')))
    .then(|| Type {
        name: name.to_owned(),
        confidence,
    })
}
fn param_doc(comment: &str, tag: &str, name: Option<&str>) -> Option<Type> {
    for line in comment.lines() {
        let line = line
            .trim()
            .trim_start_matches('/')
            .trim_start_matches('*')
            .trim();
        if let Some(rest) = line.strip_prefix(tag) {
            if !rest.starts_with(char::is_whitespace) {
                continue;
            }
            let mut words = rest.split_whitespace();
            let ty = words.next()?;
            if name.is_none() || words.next() == name {
                return type_text(ty, Confidence::Documented);
            }
        }
    }
    None
}
fn doc_before(n: Node, rope: &Rope) -> String {
    n.prev_named_sibling()
        .filter(|p| p.kind() == "comment")
        .map(|p| text(p, rope))
        .filter(|s| s.starts_with("/**"))
        .unwrap_or_default()
}
impl Analysis {
    pub(crate) fn build(
        root: Node,
        rope: &Rope,
        cancel: &AtomicUsize,
        old: Option<&Self>,
    ) -> Result<Self, Error> {
        use std::hash::{Hash, Hasher};
        let old_by_range = old
            .map(|a| {
                a.scopes
                    .iter()
                    .map(|s| {
                        ((s.range.start, s.range.end, s.parent, s.signature), s)
                    })
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        let mut result = Self::default();
        let mut pending = vec![(root, None)];
        let mut visited = 0;
        while let Some((n, parent)) = pending.pop() {
            visited += 1;
            if visited > 100_000 {
                break;
            }
            cancelled(cancel)?;
            let mut next_parent = parent;
            if is_scope(n) {
                // Fingerprints permit unchanged function reuse even when a sibling
                // changed. Offset shifts intentionally invalidate absolute ranges.
                let mut hash = std::collections::hash_map::DefaultHasher::new();
                for chunk in rope.iter_chunks(n.byte_range()) {
                    cancelled(cancel)?;
                    chunk.hash(&mut hash);
                }
                doc_before(n, rope).hash(&mut hash);
                let signature = hash.finish();
                let reused = old_by_range
                    .get(&(n.start_byte(), n.end_byte(), parent, signature))
                    .map(|s| Arc::clone(s));
                let scope = if let Some(scope) = reused {
                    scope
                } else {
                    Arc::new(analyze_scope(n, parent, signature, rope, cancel)?)
                };
                next_parent = Some(result.scopes.len());
                result.scopes.push(scope);
            }
            let mut children = child_nodes(n);
            children.reverse();
            pending.extend(children.into_iter().map(|c| (c, next_parent)));
        }
        for i in 0..result.scopes.len() {
            if let Some(parent) = result.scopes[i].parent {
                let at = result.scopes[i].range.start;
                let source = result.scopes[parent]
                    .bindings
                    .iter()
                    .filter(|b| b.parameter || b.available <= at)
                    .cloned()
                    .collect::<Vec<_>>();
                let current = Arc::make_mut(&mut result.scopes[i]);
                for b in &mut current.bindings {
                    if b.captured {
                        b.ty = source
                            .iter()
                            .rev()
                            .find(|outer| outer.name == b.name)
                            .and_then(|outer| outer.ty.clone());
                    }
                }
            }
        }
        Ok(result)
    }
    pub fn scope_at(&self, offset: usize) -> Option<&Scope> {
        self.scopes
            .iter()
            .filter(|s| s.range.start <= offset && offset <= s.range.end)
            .min_by_key(|s| s.range.end - s.range.start)
            .map(Arc::as_ref)
    }
    pub fn bindings_at(&self, offset: usize) -> BTreeMap<String, Binding> {
        let mut out = BTreeMap::new();
        if let Some(scope) = self.scope_at(offset) {
            for b in &scope.bindings {
                if b.parameter || b.available <= offset {
                    out.insert(b.name.clone(), b.clone());
                }
            }
        }
        out
    }
    pub fn diagnostics(&self) -> impl Iterator<Item = &Notice> {
        self.scopes.iter().flat_map(|s| s.diagnostics.iter())
    }
}
fn uncertain_at(node: Node, scope: Node) -> usize {
    let mut at = node.start_byte();
    let mut ancestor = node.parent();
    while let Some(n) = ancestor {
        if n == scope {
            break;
        }
        if matches!(
            n.kind(),
            "for_statement"
                | "foreach_statement"
                | "while_statement"
                | "do_statement"
        ) {
            at = n.start_byte();
        }
        ancestor = n.parent();
    }
    at
}
fn analyze_scope(
    n: Node,
    parent: Option<usize>,
    signature: u64,
    rope: &Rope,
    cancel: &AtomicUsize,
) -> Result<Scope, Error> {
    let doc = doc_before(n, rope);
    let mut scope = Scope {
        range: n.byte_range(),
        parent,
        bindings: vec![],
        return_type: n
            .child_by_field_name("return_type")
            .and_then(|t| type_text(&text(t, rope), Confidence::Declared))
            .or_else(|| param_doc(&doc, "@return", None)),
        diagnostics: vec![],
        dynamic: false,
        signature,
    };
    let params = n.child_by_field_name("parameters");
    if let Some(params) = params {
        for p in child_nodes(params) {
            if let Some(name) = p.child_by_field_name("name") {
                let name_text =
                    text(name, rope).trim_start_matches('&').trim().to_owned();
                let ty = p
                    .child_by_field_name("type")
                    .and_then(|t| type_text(&text(t, rope), Confidence::Declared))
                    .or_else(|| param_doc(&doc, "@param", Some(&name_text)));
                scope.bindings.push(Binding {
                    name: name_text,
                    range: name.byte_range(),
                    ty,
                    parameter: true,
                    captured: false,
                    available: n.start_byte(),
                });
            }
        }
    }
    // Explicit closure captures are variables in that closure, never the whole
    // parent scope. Arrow free variables are captured only where referenced.
    let mut nodes = vec![n];
    let mut reads = Vec::<(String, Range<usize>, bool)>::new();
    let mut assignments = vec![];
    let mut uncertain_writes = Vec::<(String, usize)>::new();
    let mut uncertain_all = Vec::<usize>::new();
    let mut visited = 0;
    while let Some(node) = nodes.pop() {
        visited += 1;
        if visited > 20_000 {
            scope.dynamic = true;
            break;
        }
        if visited % 128 == 0 {
            cancelled(cancel)?;
        }
        if node != n
            && (is_scope(node)
                || matches!(node.kind(), "class_declaration" | "anonymous_class"))
        {
            scope.dynamic = true;
            continue;
        }
        if matches!(
            node.kind(),
            "foreach_statement"
                | "catch_clause"
                | "global_declaration"
                | "function_static_declaration"
                | "static_variable_declaration"
        ) {
            let mut cursor = node.walk();
            let mut after_as = node.kind() != "foreach_statement";
            let mut targets = Vec::new();
            for c in node.children(&mut cursor) {
                if c.kind() == "as" {
                    after_as = true;
                    continue;
                }
                if after_as
                    && c.is_named()
                    && node.child_by_field_name("body") != Some(c)
                    && c.kind() != "compound_statement"
                {
                    targets.push(c);
                }
            }
            while let Some(target) = targets.pop() {
                visited += 1;
                if visited % 128 == 0 {
                    cancelled(cancel)?;
                }
                if visited > 20_000 {
                    uncertain_all.push(uncertain_at(node, n));
                    break;
                }
                if matches!(
                    target.kind(),
                    "member_access_expression"
                        | "nullsafe_member_access_expression"
                        | "subscript_expression"
                        | "scoped_property_access_expression"
                ) {
                    continue;
                }
                if target.kind() == "variable_name" {
                    uncertain_writes
                        .push((text(target, rope), uncertain_at(node, n)));
                }
                targets.extend(child_nodes(target));
            }
        }
        if matches!(
            node.kind(),
            "dynamic_variable_name"
                | "include_expression"
                | "include_once_expression"
                | "require_expression"
                | "require_once_expression"
                | "global_declaration"
                | "static_variable_declaration"
                | "function_static_declaration"
                | "foreach_statement"
                | "catch_clause"
                | "if_statement"
                | "switch_statement"
                | "for_statement"
                | "while_statement"
                | "do_statement"
                | "try_statement"
                | "goto_statement"
        ) {
            scope.dynamic = true;
        }
        if matches!(
            node.kind(),
            "function_call_expression"
                | "member_call_expression"
                | "scoped_call_expression"
        ) {
            let mut arguments = node
                .child_by_field_name("arguments")
                .into_iter()
                .collect::<Vec<_>>();
            while let Some(arg) = arguments.pop() {
                visited += 1;
                if visited % 128 == 0 {
                    cancelled(cancel)?;
                }
                if visited > 20_000 {
                    scope.dynamic = true;
                    uncertain_all.push(node.start_byte());
                    break;
                }
                if arg.kind() == "variable_name" {
                    scope.dynamic = true;
                    uncertain_writes.push((text(arg, rope), uncertain_at(node, n)));
                }
                arguments.extend(child_nodes(arg));
            }
        }
        if node.kind() == "function_call_expression" {
            if let Some(f) = node.child_by_field_name("function") {
                if matches!(
                    text(f, rope).to_lowercase().as_str(),
                    "extract"
                        | "compact"
                        | "get_defined_vars"
                        | "parse_str"
                        | "eval"
                        | "isset"
                        | "empty"
                        | "unset"
                ) {
                    scope.dynamic = true;
                    if !matches!(
                        text(f, rope).to_lowercase().as_str(),
                        "compact" | "get_defined_vars" | "isset" | "empty"
                    ) {
                        uncertain_all.push(node.end_byte());
                    }
                }
            }
        }
        if matches!(
            node.kind(),
            "reference_assignment_expression"
                | "augmented_assignment_expression"
                | "update_expression"
                | "list_literal"
                | "unset_statement"
                | "global_declaration"
                | "null_coalescing_expression"
        ) {
            scope.dynamic = true;
        }
        if matches!(
            node.kind(),
            "dynamic_variable_name"
                | "include_expression"
                | "include_once_expression"
                | "require_expression"
                | "require_once_expression"
        ) {
            uncertain_all.push(node.start_byte());
        }
        if matches!(
            node.kind(),
            "reference_assignment_expression"
                | "augmented_assignment_expression"
                | "update_expression"
                | "unset_statement"
        ) {
            let mut pending = child_nodes(node);
            while let Some(child) = pending.pop() {
                visited += 1;
                if visited % 128 == 0 {
                    cancelled(cancel)?;
                }
                if visited > 20_000 {
                    uncertain_all.push(node.start_byte());
                    break;
                }
                if child.kind() == "variable_name" {
                    uncertain_writes
                        .push((text(child, rope), uncertain_at(node, n)));
                }
                pending.extend(child_nodes(child));
            }
        }
        if node.kind() == "assignment_expression" {
            if let Some(left) = node.child_by_field_name("left").filter(|left| {
                matches!(left.kind(), "list_literal" | "array_creation_expression")
            }) {
                let mut targets = vec![left];
                while let Some(target) = targets.pop() {
                    visited += 1;
                    if visited % 128 == 0 {
                        cancelled(cancel)?;
                    }
                    if visited > 20_000 {
                        uncertain_all.push(uncertain_at(node, n));
                        break;
                    }
                    if matches!(
                        target.kind(),
                        "member_access_expression"
                            | "nullsafe_member_access_expression"
                            | "subscript_expression"
                            | "scoped_property_access_expression"
                    ) {
                        continue;
                    }
                    if target.kind() == "variable_name" {
                        uncertain_writes
                            .push((text(target, rope), uncertain_at(node, n)));
                    }
                    targets.extend(child_nodes(target));
                }
            }
        }
        if node.kind() == "assignment_expression" {
            let mut ancestor = node.parent();
            let mut conditional = false;
            while let Some(p) = ancestor {
                if p == n {
                    break;
                }
                if matches!(
                    p.kind(),
                    "if_statement"
                        | "foreach_statement"
                        | "for_statement"
                        | "while_statement"
                        | "do_statement"
                        | "switch_statement"
                        | "try_statement"
                ) {
                    conditional = true;
                    break;
                }
                ancestor = p.parent();
            }
            if conditional {
                if let Some(left) = node
                    .child_by_field_name("left")
                    .filter(|l| l.kind() == "variable_name")
                {
                    uncertain_writes.push((text(left, rope), uncertain_at(node, n)));
                }
            } else {
                assignments.push(node);
            }
        }
        if node.kind() == "variable_name" {
            let name = text(node, rope);
            let p = node.parent();
            let parameter = p.is_some_and(|p| {
                matches!(
                    p.kind(),
                    "simple_parameter"
                        | "variadic_parameter"
                        | "property_promotion_parameter"
                ) || p.kind() == "by_ref"
                    && p.parent().is_some_and(|p| {
                        matches!(
                            p.kind(),
                            "simple_parameter"
                                | "variadic_parameter"
                                | "property_promotion_parameter"
                        )
                    })
            });
            let lhs = p.is_some_and(|p| {
                p.kind() == "assignment_expression"
                    && p.child_by_field_name("left") == Some(node)
            });
            let capture = p.is_some_and(|p| {
                p.kind() == "anonymous_function_use_clause"
                    || p.kind() == "by_ref"
                        && p.parent().is_some_and(|q| {
                            q.kind() == "anonymous_function_use_clause"
                        })
            });
            if capture {
                scope.bindings.push(Binding {
                    name: name.clone(),
                    range: node.byte_range(),
                    ty: None,
                    parameter: true,
                    captured: true,
                    available: n.start_byte(),
                });
            }
            if !parameter && !lhs && !capture {
                let uncertain = p.is_some_and(|p| {
                    matches!(
                        p.kind(),
                        "arguments"
                            | "argument"
                            | "reference_assignment_expression"
                            | "augmented_assignment_expression"
                            | "update_expression"
                    )
                });
                reads.push((name, node.byte_range(), uncertain));
            }
        }
        let mut children = child_nodes(node);
        children.reverse();
        nodes.extend(children);
    }
    for (name, at) in uncertain_writes {
        scope.bindings.push(Binding {
            name,
            range: at..at,
            ty: None,
            parameter: false,
            captured: false,
            available: at,
        });
    }
    for at in uncertain_all {
        let names = scope
            .bindings
            .iter()
            .map(|b| b.name.clone())
            .chain(
                assignments
                    .iter()
                    .filter_map(|a| a.child_by_field_name("left"))
                    .filter(|l| l.kind() == "variable_name")
                    .map(|l| text(l, rope)),
            )
            .collect::<BTreeSet<_>>();
        for name in names {
            scope.bindings.push(Binding {
                name,
                range: at..at,
                ty: None,
                parameter: false,
                captured: false,
                available: at,
            });
        }
    }
    assignments.sort_by_key(|n| n.start_byte());
    for a in assignments {
        let Some(left) = a
            .child_by_field_name("left")
            .filter(|l| l.kind() == "variable_name")
        else {
            scope.dynamic = true;
            continue;
        };
        let name = text(left, rope);
        let right = a.child_by_field_name("right");
        let comment = doc_before(a, rope);
        let statement_doc =
            a.parent().map(|p| doc_before(p, rope)).unwrap_or_default();
        let documented = param_doc(&comment, "@var", Some(&name))
            .or_else(|| param_doc(&statement_doc, "@var", Some(&name)));
        let inferred = right.and_then(|r| match r.kind() {
            "object_creation_expression" => child_nodes(r)
                .into_iter()
                .find(|c| {
                    matches!(c.kind(), "name" | "qualified_name" | "relative_name")
                })
                .and_then(|t| type_text(&text(t, rope), Confidence::Inferred)),
            "variable_name" => scope
                .bindings
                .iter()
                .filter(|b| b.name == text(r, rope) && b.available <= a.start_byte())
                .max_by_key(|b| b.available)
                .and_then(|b| b.ty.clone()),
            "integer" => Some(Type {
                name: "int".into(),
                confidence: Confidence::Inferred,
            }),
            "float" => Some(Type {
                name: "float".into(),
                confidence: Confidence::Inferred,
            }),
            "string" | "encapsed_string" => Some(Type {
                name: "string".into(),
                confidence: Confidence::Inferred,
            }),
            _ => None,
        });
        scope.bindings.push(Binding {
            name,
            range: left.byte_range(),
            ty: documented.or(inferred),
            parameter: false,
            captured: false,
            available: a.end_byte(),
        });
    }
    if n.kind() == "arrow_function" {
        for (name, range, _) in &reads {
            if !scope.bindings.iter().any(|b| b.name == *name) {
                scope.bindings.push(Binding {
                    name: name.clone(),
                    range: range.clone(),
                    ty: None,
                    parameter: true,
                    captured: true,
                    available: n.start_byte(),
                });
            }
        }
    }
    // Dynamic or malformed scopes still yield useful declared types but never
    // diagnostics. Conservative straight-line analysis avoids branch false alarms.
    if !scope.dynamic && !n.has_error() && n.kind() != "arrow_function" {
        let superglobals = [
            "$this",
            "$GLOBALS",
            "$_GET",
            "$_POST",
            "$_SERVER",
            "$_COOKIE",
            "$_FILES",
            "$_ENV",
            "$_REQUEST",
            "$_SESSION",
            "$argc",
            "$argv",
            "$http_response_header",
            "$php_errormsg",
        ];
        let mut emitted = BTreeSet::new();
        for (name, range, uncertain) in &reads {
            if *uncertain || superglobals.contains(&name.as_str()) {
                continue;
            }
            let defined = scope.bindings.iter().any(|b| {
                b.name == *name && (b.parameter || b.available <= range.start)
            });
            if !defined && emitted.insert(name.clone()) {
                scope.diagnostics.push(Notice {
                    range: range.clone(),
                    code: "undefined-local",
                    message: format!(
                        "Undefined local variable {name} (native, high confidence)"
                    ),
                });
            }
        }
        let used = reads.iter().map(|r| r.0.as_str()).collect::<BTreeSet<_>>();
        let mut unused = BTreeSet::new();
        for b in &scope.bindings {
            if !b.parameter
                && !b.range.is_empty()
                && !b.name.starts_with("$_")
                && !used.contains(b.name.as_str())
                && unused.insert(b.name.clone())
            {
                scope.diagnostics.push(Notice {
                    range: b.range.clone(),
                    code: "unused-local",
                    message: format!(
                        "Unused local variable {} (native, high confidence)",
                        b.name
                    ),
                });
            }
        }
        if let Some(body) = n.child_by_field_name("body") {
            let mut ended = false;
            for statement in child_nodes(body) {
                if matches!(
                    statement.kind(),
                    "comment" | "function_definition" | "named_label_statement"
                ) {
                    continue;
                }
                if ended {
                    scope.diagnostics.push(Notice{range:statement.byte_range(),code:"unreachable",message:"Unreachable statement after unconditional return/throw (native, high confidence)".into()});
                    break;
                }
                ended = matches!(
                    statement.kind(),
                    "return_statement" | "throw_expression" | "throw_statement"
                );
            }
        }
    }
    scope.bindings.sort_by_key(|b| b.available);
    Ok(scope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;
    fn document(s: &str) -> Document {
        Document::new(Rope::from(s), false, &AtomicUsize::new(0)).unwrap()
    }
    #[test]
    fn params_assignments_docs_and_new() {
        let s="<?php /** @param Account $account\n * @return Receipt */ function run(User $user, $account) { /** @var Team $team */ $team=find(); $copy=$user; $obj=new Company(); $unused=1; echo $missing; return $obj; echo 1; }";
        let d = document(s);
        let a = &d.analysis;
        let b = a.bindings_at(s.rfind("return").unwrap());
        assert_eq!(b["$user"].ty.as_ref().unwrap().name, "User");
        assert_eq!(
            b["$account"].ty.as_ref().unwrap().confidence,
            Confidence::Documented
        );
        assert_eq!(b["$team"].ty.as_ref().unwrap().name, "Team");
        assert_eq!(b["$copy"].ty.as_ref().unwrap().name, "User");
        assert_eq!(b["$obj"].ty.as_ref().unwrap().name, "Company");
        assert!(a.diagnostics().any(|d| d.code == "unused-local"));
        assert!(a.diagnostics().any(|d| d.code == "undefined-local"));
        assert!(a.diagnostics().any(|d| d.code == "unreachable"));
    }
    #[test]
    fn nested_scopes_are_separate_and_captures_explicit() {
        let s="<?php function outer(User $user){$outer=1; $f=function($inner) use ($user){echo $inner; echo $user;}; $arrow=fn($x)=>$x+$outer; echo $outer;}";
        let d = document(s);
        let at = s.find("echo $inner").unwrap();
        let b = d.analysis.bindings_at(at);
        assert!(b.contains_key("$inner"));
        assert!(b.contains_key("$user"));
        assert!(!b.contains_key("$outer"));
        assert_eq!(d.analysis.scopes.len(), 3);
    }
    #[test]
    fn dynamic_and_control_flow_suppress_diagnostics() {
        for code in [
            "extract($data); echo $unknown;",
            "include 'file.php'; echo $unknown;",
            "$$name=1; echo $unknown;",
            "if($x){$a=1;} echo $a;",
        ] {
            let d = document(&format!("<?php function f(){{{code}}}"));
            assert_eq!(d.analysis.diagnostics().count(), 0, "{code}");
        }
    }
    #[test]
    fn reassignment_clears_old_type_and_error_suppresses_diagnostics() {
        let s = "<?php function f(){ $u=new User(); $u=unknown(); echo $u; }";
        let d = document(s);
        assert!(d.analysis.bindings_at(s.find("echo").unwrap())["$u"]
            .ty
            .is_none());
        let d = document("<?php function f(User $u){ echo $missing; $u->");
        assert_eq!(d.analysis.diagnostics().count(), 0);
    }
    #[test]
    fn future_assignment_not_available() {
        let s = "<?php function f(){ echo $later; $later=new User();}";
        let d = document(s);
        assert!(!d
            .analysis
            .bindings_at(s.find("echo").unwrap())
            .contains_key("$later"));
    }
    #[test]
    fn declared_return_and_explicit_capture_types() {
        let s="<?php /** @return Result */ function f(User $u){$f=function($u) {return $u;}; $g=function()use($u){return $u;}; $a=fn()=> $u;}";
        let d = document(s);
        assert_eq!(
            d.analysis.scopes[0].return_type.as_ref().unwrap().name,
            "Result"
        );
        assert!(d.analysis.scopes[1]
            .bindings
            .iter()
            .find(|b| b.name == "$u")
            .unwrap()
            .ty
            .is_none());
        for i in [2, 3] {
            assert_eq!(
                d.analysis.scopes[i]
                    .bindings
                    .iter()
                    .find(|b| b.name == "$u")
                    .unwrap()
                    .ty
                    .as_ref()
                    .unwrap()
                    .name,
                "User"
            );
        }
    }
    #[test]
    fn reuses_unchanged_scope_and_cancels() {
        let d = document("<?php function f(User $u){return $u;}");
        let rebuilt = Analysis::build(
            d.tree.root_node(),
            d.text(),
            &AtomicUsize::new(0),
            Some(&d.analysis),
        )
        .unwrap();
        assert!(Arc::ptr_eq(&d.analysis.scopes[0], &rebuilt.scopes[0]));
        assert_eq!(
            Analysis::build(
                d.tree.root_node(),
                d.text(),
                &AtomicUsize::new(1),
                None
            )
            .unwrap_err(),
            Error::Cancelled
        );
    }
    #[test]
    fn dynamic_read_write_regressions() {
        for body in [
            "preg_match($re,$s,$m); echo $m;",
            "$a=&$b;",
            "$x ??= 1;",
            "$i++;",
            "compact('x');",
            "$this->p=$x;",
            "return; function hoisted(){}",
            "return; label: goto label;",
        ] {
            let d = document(&format!("<?php function f(){{{body}}}"));
            assert_eq!(d.analysis.diagnostics().count(), 0, "{body}");
        }
        for body in [
            "$name='x'; echo \"Hi $name\";",
            "$name='x'; echo <<<TXT\nHi $name\nTXT;",
            "echo $http_response_header; echo $php_errormsg;",
        ] {
            let d = document(&format!("<?php function f(){{{body}}}"));
            assert!(!d.analysis.diagnostics().any(|n|n.code=="unused-local"||n.code=="undefined-local"),"{body}");
        }
    }
    #[test]
    fn types_survive_unrelated_uncertainty_and_not_own_rhs() {
        for body in [
            "foo($x);",
            "if($x){echo 1;}",
            "foreach($xs as $x){echo $x;}",
            "$u->x=1;",
            "foreach($u as $x){echo $x;}",
        ] {
            let s = format!("<?php function f(){{$u=new User(); {body} $u->go();}}");
            let d = document(&s);
            assert_eq!(
                d.analysis.bindings_at(s.rfind("$u->").unwrap())["$u"]
                    .ty
                    .as_ref()
                    .unwrap()
                    .name,
                "User",
                "{body}"
            );
        }
        let s = "<?php function f(User $user){$user=$user->name; echo $user;}";
        let d = document(s);
        assert_eq!(
            d.analysis.bindings_at(s.find("$user->").unwrap())["$user"]
                .ty
                .as_ref()
                .unwrap()
                .name,
            "User"
        );
        assert!(d.analysis.bindings_at(s.find("echo").unwrap())["$user"]
            .ty
            .is_none());
    }
    #[test]
    fn loop_carried_and_implicit_writes_clear_stale_types() {
        for body in [
            "foreach($xs as $x){$u->go(); $u=new B();}",
            "foreach($xs as $x){$u->go(); foo($u);}",
            "while($ok){$u->go(); $u=&$v;}",
            "foreach($xs as $x){while($ok){$u->go(); $u=new B();}}",
            "for(;;){$u->go(); $u=new B();}",
            "do {$u->go(); $u=new B();} while($ok);",
            "while($ok){$u=new B();} $u->go();",
            "[$u]=f(); $u->go();",
            "foreach($xs as $u){} $u->go();",
            "try {} catch(E $u) {$u->go();}",
            "global $u; $u->go();",
            "static $u; $u->go();",
        ] {
            let source = format!("<?php function f(){{$u=new A(); {body}}}");
            let d = document(&source);
            assert!(
                d.analysis.bindings_at(source.find("$u->go").unwrap())["$u"]
                    .ty
                    .is_none(),
                "{body}"
            );
        }
    }
    #[test]
    #[ignore]
    fn scope_5k_line_baseline() {
        let source = format!(
            "<?php function f(User $u){{\n{}\n}}",
            (0..5000)
                .map(|i| format!("$x{i}={i};\n"))
                .collect::<String>()
        );
        let mut samples = Vec::new();
        for _ in 0..5 {
            let start = std::time::Instant::now();
            let context = Document::new_context(
                Rope::from(source.as_str()),
                false,
                &AtomicUsize::new(0),
            )
            .unwrap();
            let published = Arc::new(context);
            let mut analysis = published.clone();
            Arc::make_mut(&mut analysis)
                .refresh_analysis(&AtomicUsize::new(0))
                .unwrap();
            assert!(!analysis.analysis.scopes.is_empty());
            samples.push(start.elapsed().as_secs_f64() * 1000.);
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "scope5000 debug parse_publish_clone_and_analysis min_ms={:.3} median_ms={:.3}",
            samples[0], samples[2]
        );
    }
    #[test]
    fn unicode_ranges_are_bytes() {
        let s = "<?php function f(){ $é=new User(); echo $é; }";
        let d = document(s);
        for b in &d.analysis.scopes[0].bindings {
            assert!(s.is_char_boundary(b.range.start));
            assert_eq!(&s[b.range.clone()], "$é");
        }
    }
}
