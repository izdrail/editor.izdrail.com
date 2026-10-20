//! lapce-index: the embedded project search index.
//!
//! One tantivy index per workspace holds every indexed file and every
//! extracted symbol, so file search, workspace-symbol search and the
//! local-first completion cascade all read from a single structure.
//! The schema and the build plan live in docs/project-index.md.

pub mod walker;
pub mod symbols;

use std::fs;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, QueryParser, QueryParserError, RegexQuery, TermQuery};
use tantivy::schema::{Field, IndexRecordOption, Schema, Value, INDEXED, STORED, STRING, TEXT};
use tantivy::{doc, Index, IndexReader, IndexWriter, TantivyDocument, TantivyError, Term};

/// What an indexed record represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A whole indexed file (body = file text).
    File,
    /// A symbol definition extracted from a parse tree.
    SymbolDef,
    /// A symbol reference extracted from a parse tree.
    SymbolRef,
}

impl EntryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EntryKind::File => "file",
            EntryKind::SymbolDef => "symbol-def",
            EntryKind::SymbolRef => "symbol-ref",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "file" => Some(EntryKind::File),
            "symbol-def" => Some(EntryKind::SymbolDef),
            "symbol-ref" => Some(EntryKind::SymbolRef),
            _ => None,
        }
    }
}

/// One record to add to the index.
#[derive(Debug, Clone)]
pub struct IndexEntry {
    pub path: String,
    pub kind: EntryKind,
    pub language: String,
    pub symbol: Option<String>,
    pub line: u64,
    pub col: u64,
    pub body: String,
}

/// One search result.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub path: String,
    pub kind: EntryKind,
    pub language: String,
    pub symbol: Option<String>,
    pub line: u64,
    pub col: u64,
    pub score: f32,
}

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("tantivy: {0}")]
    Tantivy(#[from] TantivyError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("project index schema does not match this editor version; use a new cache directory")]
    SchemaMismatch,
    #[error("symbol extraction: {0}")]
    SymbolParse(String),
    #[error("query parse: {0}")]
    QueryParse(#[from] QueryParserError),
}

pub type Result<T> = std::result::Result<T, IndexError>;

pub use walker::{index_workspace, language_for_path, IndexStats};

#[derive(Clone, Copy)]
struct Fields {
    path: Field,
    kind: Field,
    language: Field,
    symbol: Field,
    line: Field,
    col: Field,
    body: Field,
}

fn build_schema() -> (Schema, Fields) {
    let mut builder = Schema::builder();
    let path = builder.add_text_field("path", STRING | STORED);
    let kind = builder.add_text_field("kind", STRING | STORED);
    let language = builder.add_text_field("language", STRING | STORED);
    let symbol = builder.add_text_field("symbol", TEXT | STORED);
    let line = builder.add_u64_field("line", INDEXED | STORED);
    let col = builder.add_u64_field("col", INDEXED | STORED);
    let body = builder.add_text_field("body", TEXT | STORED);
    let fields = Fields {
        path,
        kind,
        language,
        symbol,
        line,
        col,
        body,
    };
    (builder.build(), fields)
}

/// The project index: one tantivy index wrapping file and symbol records.
pub struct ProjectIndex {
    index: Index,
    fields: Fields,
    writer: Mutex<IndexWriter>,
    reader: IndexReader,
}

impl ProjectIndex {
    /// A volatile in-memory index. Used by tests and by callers that
    /// reindex from scratch every session.
    pub fn in_memory() -> Result<Self> {
        let (schema, fields) = build_schema();
        let index = Index::create_in_ram(schema);
        Self::from_index(index, fields)
    }

    /// Open the on-disk index in `dir`, creating it when absent.
    pub fn open_or_create(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)?;
        let (schema, fields) = build_schema();
        // Only an absent metadata file means a new index. Corruption or
        // permission errors must not be mistaken for permission to recreate it.
        let index = if dir.join("meta.json").try_exists()? {
            let index = Index::open_in_dir(dir)?;
            // Fields are numeric IDs derived from build_schema; accepting a
            // different layout could read/write the wrong fields silently.
            if index.schema() != schema {
                return Err(IndexError::SchemaMismatch);
            }
            index
        } else {
            Index::create_in_dir(dir, schema)?
        };
        Self::from_index(index, fields)
    }

    fn from_index(index: Index, fields: Fields) -> Result<Self> {
        let writer = index.writer(50_000_000)?;
        let reader = index.reader()?;
        Ok(Self {
            index,
            fields,
            writer: Mutex::new(writer),
            reader,
        })
    }

    fn writer(&self) -> MutexGuard<'_, IndexWriter> {
        self.writer.lock().expect("index writer poisoned")
    }

    /// Queue one record. Call `commit` to make it searchable.
    pub fn add_entry(&self, entry: &IndexEntry) -> Result<()> {
        let fields = &self.fields;
        let mut document = doc!(
            fields.path => entry.path.as_str(),
            fields.kind => entry.kind.as_str(),
            fields.language => entry.language.as_str(),
            fields.line => entry.line,
            fields.col => entry.col,
            fields.body => entry.body.as_str(),
        );
        if let Some(symbol) = &entry.symbol {
            document.add_text(fields.symbol, symbol);
        }
        self.writer().add_document(document)?;
        Ok(())
    }

    /// Queue deletion of every record for `path` (exact match). Call
    /// `commit` to make the deletion visible. Used on save (delete, then
    /// re-add the file's records) and on file removal.
    pub fn remove_path(&self, path: &str) -> Result<()> {
        let term = Term::from_field_text(self.fields.path, path);
        self.writer().delete_term(term);
        Ok(())
    }

    /// Queue removal of every record before a full workspace rebuild.
    pub fn clear(&self) -> Result<()> {
        self.writer().delete_all_documents()?;
        Ok(())
    }

    /// Flush queued additions and deletions so searches can see them.
    pub fn commit(&self) -> Result<()> {
        self.writer().commit()?;
        self.reader.reload()?;
        Ok(())
    }

    /// Search symbol names and file bodies, best matches first.
    pub fn search(&self, query_text: &str, limit: usize) -> Result<Vec<SearchHit>> {
        let searcher = self.reader.searcher();
        let parser = QueryParser::for_index(
            &self.index,
            vec![self.fields.symbol, self.fields.body],
        );
        let query = parser.parse_query(query_text)?;
        let top_docs =
            searcher.search(&query, &TopDocs::with_limit(limit).order_by_score())?;
        let mut hits = Vec::with_capacity(top_docs.len());
        for (score, address) in top_docs {
            let document: TantivyDocument = searcher.doc(address)?;
            hits.push(self.hit_from_document(score, &document));
        }
        Ok(hits)
    }

    /// Prefix candidates from symbol definitions only. This is a lexical
    /// candidate source, not type-aware completion. Invalid or empty prefixes
    /// return no candidates rather than interpreting query/regex syntax.
    pub fn symbol_prefix(&self, prefix: &str, language: &str, limit: usize) -> Result<Vec<SearchHit>> {
        if limit == 0 || prefix.is_empty() || prefix.len() > 128
            || !prefix.chars().all(|ch| ch.is_alphanumeric() || ch == '_')
        {
            return Ok(Vec::new());
        }
        // TEXT's default tokenizer lowercases terms; restrict characters
        // above so the prefix cannot introduce a regex operator.
        let pattern = format!("{}.*", prefix.to_lowercase());
        let query = BooleanQuery::intersection(vec![
            Box::new(RegexQuery::from_pattern(&pattern, self.fields.symbol)?),
            Box::new(TermQuery::new(
                Term::from_field_text(self.fields.kind, EntryKind::SymbolDef.as_str()),
                IndexRecordOption::Basic,
            )),
            Box::new(TermQuery::new(
                Term::from_field_text(self.fields.language, language),
                IndexRecordOption::Basic,
            )),
        ]);
        let searcher = self.reader.searcher();
        let docs = searcher.search(&query, &TopDocs::with_limit(limit.min(200)).order_by_score())?;
        let mut hits = Vec::with_capacity(docs.len());
        for (score, address) in docs {
            let document: TantivyDocument = searcher.doc(address)?;
            hits.push(self.hit_from_document(score, &document));
        }
        Ok(hits)
    }

    fn hit_from_document(&self, score: f32, document: &TantivyDocument) -> SearchHit {
        SearchHit {
            path: self.text_value(document, self.fields.path).unwrap_or_default(),
            kind: self
                .text_value(document, self.fields.kind)
                .and_then(|value| EntryKind::from_str(&value))
                .unwrap_or(EntryKind::File),
            language: self
                .text_value(document, self.fields.language)
                .unwrap_or_default(),
            symbol: self.text_value(document, self.fields.symbol),
            line: self.u64_value(document, self.fields.line),
            col: self.u64_value(document, self.fields.col),
            score,
        }
    }

    fn text_value(&self, document: &TantivyDocument, field: Field) -> Option<String> {
        document
            .get_first(field)
            .and_then(|value| value.as_str())
            .map(str::to_owned)
    }

    fn u64_value(&self, document: &TantivyDocument, field: Field) -> u64 {
        document
            .get_first(field)
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_entry(path: &str, language: &str, body: &str) -> IndexEntry {
        IndexEntry {
            path: path.to_string(),
            kind: EntryKind::File,
            language: language.to_string(),
            symbol: None,
            line: 0,
            col: 0,
            body: body.to_string(),
        }
    }

    #[test]
    fn add_commit_search_finds_file() {
        let index = ProjectIndex::in_memory().unwrap();
        index
            .add_entry(&file_entry(
                "src/checkout.php",
                "php",
                "function render_checkout() { return 'cart'; }",
            ))
            .unwrap();
        index.commit().unwrap();

        let hits = index.search("render_checkout", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "src/checkout.php");
        assert_eq!(hits[0].kind, EntryKind::File);
        assert_eq!(hits[0].language, "php");
    }

    #[test]
    fn remove_path_drops_its_entries() {
        let index = ProjectIndex::in_memory().unwrap();
        index
            .add_entry(&file_entry("a.php", "php", "alpha beta"))
            .unwrap();
        index
            .add_entry(&file_entry("b.php", "php", "alpha gamma"))
            .unwrap();
        index.commit().unwrap();
        assert_eq!(index.search("alpha", 10).unwrap().len(), 2);

        index.remove_path("a.php").unwrap();
        index.commit().unwrap();
        let hits = index.search("alpha", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "b.php");
    }

    #[test]
    fn symbol_definitions_are_searchable_by_name() {
        let index = ProjectIndex::in_memory().unwrap();
        index
            .add_entry(&IndexEntry {
                path: "app/Models/Order.php".to_string(),
                kind: EntryKind::SymbolDef,
                language: "php".to_string(),
                symbol: Some("OrderRepository".to_string()),
                line: 12,
                col: 6,
                body: "class OrderRepository extends Repository".to_string(),
            })
            .unwrap();
        index.commit().unwrap();

        let hits = index.search("OrderRepository", 10).unwrap();
        assert_eq!(hits.len(), 1);
        let hit = &hits[0];
        assert_eq!(hit.kind, EntryKind::SymbolDef);
        assert_eq!(hit.symbol.as_deref(), Some("OrderRepository"));
        assert_eq!(hit.line, 12);
        assert_eq!(hit.col, 6);
    }

    #[test]
    fn open_or_create_round_trips_through_disk() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("lapce-index-test-{unique}"));

        {
            let index = ProjectIndex::open_or_create(&dir).unwrap();
            index
                .add_entry(&file_entry("web/app.js", "javascript", "const cart = {};"))
                .unwrap();
            index.commit().unwrap();
        }
        {
            let index = ProjectIndex::open_or_create(&dir).unwrap();
            let hits = index.search("cart", 10).unwrap();
            assert_eq!(hits.len(), 1);
            assert_eq!(hits[0].path, "web/app.js");
        }

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn opening_foreign_schema_fails_without_replacing_it() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("lapce-index-schema-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        let mut builder = Schema::builder();
        builder.add_text_field("foreign", TEXT | STORED);
        let foreign_schema = builder.build();
        drop(Index::create_in_dir(&dir, foreign_schema.clone()).unwrap());
        let before = fs::read(dir.join("meta.json")).unwrap();

        assert!(matches!(ProjectIndex::open_or_create(&dir), Err(IndexError::SchemaMismatch)));
        assert_eq!(fs::read(dir.join("meta.json")).unwrap(), before);
        assert_eq!(Index::open_in_dir(&dir).unwrap().schema(), foreign_schema);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn corrupt_metadata_is_not_recreated() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("lapce-index-corrupt-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        let bytes = b"not valid index metadata";
        fs::write(dir.join("meta.json"), bytes).unwrap();

        assert!(ProjectIndex::open_or_create(&dir).is_err());
        assert_eq!(fs::read(dir.join("meta.json")).unwrap(), bytes);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn symbol_prefix_filters_language_kind_and_case() {
        let index = ProjectIndex::in_memory().unwrap();
        for (path, language, kind, name) in [
            ("a.php", "php", EntryKind::SymbolDef, "CheckoutService"),
            ("b.ts", "typescript", EntryKind::SymbolDef, "CheckoutWidget"),
            ("c.php", "php", EntryKind::SymbolRef, "CheckoutService"),
            ("d.php", "php", EntryKind::SymbolDef, "InvoiceService"),
        ] {
            index.add_entry(&IndexEntry {
                path: path.into(), language: language.into(), kind,
                symbol: Some(name.into()), line: 1, col: 0, body: name.into(),
            }).unwrap();
        }
        index.add_entry(&file_entry("file.php", "php", "CheckoutService")).unwrap();
        index.commit().unwrap();
        let hits = index.symbol_prefix("CHECK", "php", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "a.php");
        assert_eq!(hits[0].symbol.as_deref(), Some("CheckoutService"));
        assert_eq!(index.symbol_prefix("check", "typescript", 10).unwrap().len(), 1);
        assert!(index.symbol_prefix("check", "vue", 10).unwrap().is_empty());
    }

    #[test]
    fn symbol_prefix_rejects_empty_query_syntax_and_zero_limit() {
        let index = ProjectIndex::in_memory().unwrap();
        for prefix in ["", "foo.*", "a:b", "a b", "(foo)", "foo\\bar"] {
            assert!(index.symbol_prefix(prefix, "php", 10).unwrap().is_empty());
        }
        assert!(index.symbol_prefix("foo", "php", 0).unwrap().is_empty());
        assert!(index.symbol_prefix(&"a".repeat(129), "php", 10).unwrap().is_empty());
    }

    #[test]
    fn searching_an_empty_index_returns_no_hits() {
        let index = ProjectIndex::in_memory().unwrap();
        let hits = index.search("anything", 10).unwrap();
        assert!(hits.is_empty());
    }
}
