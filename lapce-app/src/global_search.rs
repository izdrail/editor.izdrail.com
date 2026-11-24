use std::{ops::Range, path::PathBuf, rc::Rc};

use floem::{
    ext_event::create_ext_action,
    keyboard::Modifiers,
    reactive::{Memo, RwSignal, Scope, SignalGet, SignalUpdate, SignalWith},
    views::VirtualVector,
};
use indexmap::IndexMap;
use lapce_core::{mode::Mode, selection::Selection};
use lapce_rpc::proxy::{ProxyResponse, SearchMatch};
use lapce_xi_rope::Rope;

use crate::{
    command::{CommandExecuted, CommandKind},
    editor::EditorData,
    keypress::{KeyPressFocus, condition::Condition},
    main_split::MainSplitData,
    window_tab::CommonData,
};

#[derive(Clone)]
pub struct SearchMatchData {
    pub expanded: RwSignal<bool>,
    pub matches: RwSignal<im::Vector<SearchMatch>>,
    pub line_height: Memo<f64>,
}

impl SearchMatchData {
    pub fn height(&self) -> f64 {
        let line_height = self.line_height.get();
        let count = if self.expanded.get() {
            self.matches.with(|m| m.len()) + 1
        } else {
            1
        };
        line_height * count as f64
    }
}

#[derive(Clone, Debug)]
pub struct GlobalSearchData {
    pub editor: EditorData,
    pub search_result: RwSignal<IndexMap<PathBuf, SearchMatchData>>,
    pub search_status: RwSignal<String>,
    pub search_generation: RwSignal<u64>,
    pub main_split: MainSplitData,
    pub common: Rc<CommonData>,
}

impl KeyPressFocus for GlobalSearchData {
    fn get_mode(&self) -> Mode {
        Mode::Insert
    }

    fn check_condition(&self, condition: Condition) -> bool {
        matches!(condition, Condition::PanelFocus)
    }

    fn run_command(
        &self,
        command: &crate::command::LapceCommand,
        count: Option<usize>,
        mods: Modifiers,
    ) -> CommandExecuted {
        match &command.kind {
            CommandKind::Workbench(_) => {}
            CommandKind::Scroll(_) => {}
            CommandKind::Focus(_) => {}
            CommandKind::Edit(_)
            | CommandKind::Move(_)
            | CommandKind::MultiSelection(_) => {
                return self.editor.run_command(command, count, mods);
            }
            CommandKind::MotionMode(_) => {}
        }
        CommandExecuted::No
    }

    fn receive_char(&self, c: &str) {
        self.editor.receive_char(c);
    }
}

impl VirtualVector<(PathBuf, SearchMatchData)> for GlobalSearchData {
    fn total_len(&self) -> usize {
        self.search_result.with(|result| {
            result
                .iter()
                .map(|(_, data)| {
                    if data.expanded.get() {
                        data.matches.with(|m| m.len()) + 1
                    } else {
                        1
                    }
                })
                .sum()
        })
    }

    fn slice(
        &mut self,
        _range: Range<usize>,
    ) -> impl Iterator<Item = (PathBuf, SearchMatchData)> {
        self.search_result.get().into_iter()
    }
}

impl GlobalSearchData {
    pub fn new(cx: Scope, main_split: MainSplitData) -> Self {
        let common = main_split.common.clone();
        let editor = main_split.editors.make_local(cx, common.clone());
        let search_result = cx.create_rw_signal(IndexMap::new());

        let global_search = Self {
            editor,
            search_result,
            search_status: cx.create_rw_signal("Type to search workspace files.".into()),
            search_generation: cx.create_rw_signal(0),
            main_split,
            common,
        };

        {
            let global_search = global_search.clone();
            let buffer = global_search.editor.doc().buffer;
            cx.create_effect(move |_| {
                let pattern = buffer.with(|buffer| buffer.to_string());
                // Every edit, including clearing, invalidates older replies.
                let generation = global_search.search_generation.get_untracked().wrapping_add(1);
                global_search.search_generation.set(generation);
                global_search.search_result.update(|r| r.clear());
                if pattern.is_empty() {
                    global_search.search_status.set("Type to search workspace files.".into());
                    return;
                }
                let case_sensitive = global_search.common.find.case_sensitive(true);
                let whole_word = global_search.common.find.whole_words.get();
                let is_regex = global_search.common.find.is_regex.get();
                if is_regex {
                    if let Err(error) = regex::Regex::new(&pattern) {
                        global_search.search_status.set(format!("Invalid regular expression: {error}"));
                        return;
                    }
                }
                global_search.search_status.set("Searching workspace files...".into());
                let send = {
                    let global_search = global_search.clone();
                    create_ext_action(cx, move |result: Result<ProxyResponse, lapce_rpc::RpcError>| {
                        if generation != global_search.search_generation.get_untracked() { return; }
                        match result {
                            Ok(ProxyResponse::GlobalSearchResponse { matches }) => {
                                let count: usize = matches.values().map(Vec::len).sum();
                                let files = matches.len();
                                global_search.search_status.set(search_result_status(count, files));
                                global_search.update_matches(matches);
                            }
                            Err(error) => global_search.search_status.set(format!("Search failed: {}", error.message)),
                            _ => global_search.search_status.set("Search returned an unexpected response. Try again.".into()),
                        }
                    })
                };
                global_search.common.proxy.global_search(
                    pattern,
                    case_sensitive,
                    whole_word,
                    is_regex,
                    move |result| {
                        send(result);
                    },
                );
            });
        }

        {
            let buffer = global_search.editor.doc().buffer;
            let main_split = global_search.main_split.clone();
            cx.create_effect(move |_| {
                let content = buffer.with(|buffer| buffer.to_string());
                main_split.set_find_pattern(Some(content));
            });
        }

        global_search
    }

    fn update_matches(&self, matches: IndexMap<PathBuf, Vec<SearchMatch>>) {
        let current = self.search_result.get_untracked();

        self.search_result.set(
            matches
                .into_iter()
                .map(|(path, matches)| {
                    let match_data =
                        current.get(&path).cloned().unwrap_or_else(|| {
                            SearchMatchData {
                                expanded: self.common.scope.create_rw_signal(true),
                                matches: self
                                    .common
                                    .scope
                                    .create_rw_signal(im::Vector::new()),
                                line_height: self.common.ui_line_height,
                            }
                        });

                    match_data.matches.set(matches.into());

                    (path, match_data)
                })
                .collect(),
        );
    }

    pub fn set_pattern(&self, pattern: String) {
        let pattern_len = pattern.len();
        self.editor.doc().reload(Rope::from(pattern), true);
        self.editor
            .cursor()
            .update(|cursor| cursor.set_insert(Selection::region(0, pattern_len)));
    }
}

fn search_result_status(matches: usize, files: usize) -> String {
    if matches == 0 { return "No matches found.".into(); }
    format!("{matches} {} in {files} {}", if matches == 1 { "match" } else { "matches" }, if files == 1 { "file" } else { "files" })
}

#[cfg(test)]
mod search_status_tests {
    use super::search_result_status;
    #[test]
    fn empty_and_counted_states_are_explicit() {
        assert_eq!(search_result_status(0, 0), "No matches found.");
        assert_eq!(search_result_status(1, 1), "1 match in 1 file");
        assert_eq!(search_result_status(4, 1), "4 matches in 1 file");
        assert_eq!(search_result_status(4, 2), "4 matches in 2 files");
    }
}
