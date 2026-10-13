//! Open VSX marketplace in the plugin panel: search, install, uninstall and
//! the list of imported extensions. Backed by `lapce_vsix::openvsx`. Only the
//! declarative content of an extension is imported; its code is never run.
use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use floem::{
    View,
    ext_event::create_ext_action,
    reactive::{RwSignal, Scope, SignalGet, SignalUpdate, SignalWith},
    style::CursorStyle,
    views::{Decorators, container, dyn_stack, empty, label, scroll::scroll, stack},
};
use lapce_core::directory::Directory;
use lapce_rpc::core::CoreNotification;
use lapce_vsix::{
    ImportReport,
    openvsx::{self, Client, Extension, Installed, SearchResults},
};
use lsp_types::MessageType;

use crate::{
    command::InternalCommand,
    config::{LapceConfig, color::LapceColor},
    plugin::PluginData,
};

const TITLE: &str = "Open VSX";

#[derive(Clone)]
pub struct VsxMarket {
    pub results: RwSignal<Vec<Extension>>,
    pub installed: RwSignal<Vec<Installed>>,
    /// Extension ids with an install or uninstall in flight.
    pub busy: RwSignal<HashSet<String>>,
    pub status: RwSignal<String>,
    generation: Arc<AtomicUsize>,
}

impl VsxMarket {
    pub fn new(cx: Scope) -> Self {
        let market = Self {
            results: cx.create_rw_signal(Vec::new()),
            installed: cx.create_rw_signal(Vec::new()),
            busy: cx.create_rw_signal(HashSet::new()),
            status: cx.create_rw_signal(String::new()),
            generation: Arc::new(AtomicUsize::new(0)),
        };
        market.refresh_installed();
        market
    }

    pub fn refresh_installed(&self) {
        let list = Directory::vsx_directory()
            .map(|root| openvsx::list_installed(&root))
            .unwrap_or_default();
        self.installed.set(list);
    }
}

fn is_installed(list: &[Installed], id: &str) -> bool {
    list.iter().any(|i| i.id.eq_ignore_ascii_case(id))
}

impl PluginData {
    fn vsx_message(&self, typ: MessageType, message: String) {
        self.core_rpc.notification(CoreNotification::ShowMessage {
            title: TITLE.to_string(),
            message: lsp_types::ShowMessageParams { typ, message },
        });
    }

    /// Search Open VSX. Waits briefly so typing does not fire a request per
    /// key, and drops answers that a newer search has superseded.
    pub fn vsx_search(&self, query: &str) {
        let market = self.vsx.clone();
        let generation = market.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let latest = market.generation.clone();
        market.status.set("Searching open-vsx.org...".to_string());
        let query = query.trim().to_owned();
        let plugin = self.clone();
        let send = create_ext_action(
            self.common.scope,
            move |result: anyhow::Result<SearchResults>| {
                if plugin.vsx.generation.load(Ordering::SeqCst) != generation {
                    return;
                }
                match result {
                    Ok(r) => {
                        plugin.vsx.status.set(format!(
                            "{} results on open-vsx.org (showing {})",
                            r.total,
                            r.extensions.len()
                        ));
                        plugin.vsx.results.set(r.extensions);
                    }
                    Err(e) => {
                        plugin.vsx.results.set(Vec::new());
                        plugin.vsx.status.set(format!("Search failed: {e:#}"));
                    }
                }
            },
        );
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(350));
            if latest.load(Ordering::SeqCst) != generation {
                return;
            }
            send(Client::open_vsx().and_then(|c| c.search(&query, 30, 0)));
        });
    }

    pub fn vsx_install(&self, id: String) {
        if self.vsx.busy.with_untracked(|b| b.contains(&id)) {
            return;
        }
        self.vsx.busy.update(|b| {
            b.insert(id.clone());
        });
        let plugin = self.clone();
        let done_id = id.clone();
        let send = create_ext_action(
            self.common.scope,
            move |result: anyhow::Result<ImportReport>| {
                plugin.vsx.busy.update(|b| {
                    b.remove(&done_id);
                });
                plugin.vsx.refresh_installed();
                match result {
                    Ok(rep) => {
                        plugin
                            .common
                            .internal_command
                            .send(InternalCommand::ReloadConfig);
                        let mut msg = format!(
                            "Installed {} {} ({} themes, {} icon themes, {} grammars, {} snippets)",
                            rep.id,
                            rep.version,
                            rep.color_themes.len(),
                            rep.icon_themes.len(),
                            rep.grammars.len(),
                            rep.snippet_count
                        );
                        if rep.has_code {
                            msg.push_str(". Its code is not loaded.");
                        }
                        plugin.vsx.status.set(msg);
                    }
                    Err(e) => {
                        plugin.vsx_message(
                            MessageType::ERROR,
                            format!("{done_id}: {e:#}"),
                        );
                        plugin.vsx.status.set(format!("{done_id}: {e:#}"));
                    }
                }
            },
        );
        std::thread::spawn(move || {
            let result = match Directory::vsx_directory() {
                Some(root) => Client::open_vsx().and_then(|c| c.install(&id, &root)),
                None => Err(anyhow::anyhow!("cannot locate the data directory")),
            };
            send(result);
        });
    }

    pub fn vsx_uninstall(&self, id: String) {
        let Some(root) = Directory::vsx_directory() else {
            return;
        };
        match openvsx::uninstall(&id, &root) {
            Ok(_) => {
                self.vsx.status.set(format!("Removed {id}"));
                self.common
                    .internal_command
                    .send(InternalCommand::ReloadConfig);
            }
            Err(e) => {
                self.vsx_message(MessageType::ERROR, format!("{id}: {e:#}"));
            }
        }
        self.vsx.refresh_installed();
    }
}

fn button(
    config: floem::reactive::ReadSignal<Arc<LapceConfig>>,
    text: impl Fn() -> String + 'static,
    disabled: impl Fn() -> bool + 'static,
    on_click: impl Fn() + 'static,
) -> impl View {
    label(text)
        .disabled(disabled)
        .on_click_stop(move |_| on_click())
        .style(move |s| {
            let config = config.get();
            s.color(config.color(LapceColor::LAPCE_BUTTON_PRIMARY_FOREGROUND))
                .background(
                    config.color(LapceColor::LAPCE_BUTTON_PRIMARY_BACKGROUND),
                )
                .margin_left(6.0)
                .padding_horiz(6.0)
                .border_radius(6.0)
                .selectable(false)
                .hover(|s| {
                    s.cursor(CursorStyle::Pointer).background(
                        config
                            .color(LapceColor::LAPCE_BUTTON_PRIMARY_BACKGROUND)
                            .multiply_alpha(0.8),
                    )
                })
                .disabled(|s| s.background(config.color(LapceColor::EDITOR_DIM)))
        })
}

fn list_container(inner: impl View + 'static) -> impl View {
    container(scroll(inner).style(|s| s.absolute().size_pct(100.0, 100.0))).style(
        |s| {
            s.width_pct(100.0)
                .line_height(1.6)
                .flex_grow(1.0f32)
                .flex_basis(0.0)
        },
    )
}

fn row_style(
    plugin: &PluginData,
) -> impl use<> + Fn(floem::style::Style) -> floem::style::Style + 'static {
    let config = plugin.common.config;
    move |s| {
        s.flex_col()
            .width_pct(100.0)
            .padding_horiz(10.0)
            .padding_vert(5.0)
            .hover(|s| {
                s.background(
                    config.get().color(LapceColor::PANEL_HOVERED_BACKGROUND),
                )
            })
    }
}

/// Imported extensions with an uninstall button.
pub fn vsx_installed_view(plugin: PluginData) -> impl View {
    let installed = plugin.vsx.installed;
    list_container(
        dyn_stack(
            move || installed.get(),
            |e| (e.id.clone(), e.version.clone()),
            move |e| {
                let uninstall_id = e.id.clone();
                let p = plugin.clone();
                let busy_plugin = plugin.clone();
                let detail = format!(
                    "v{}  themes {}, icon themes {}, grammars {}, snippets {}",
                    e.version, e.color_themes, e.icon_themes, e.grammars, e.snippets
                );
                let code = e.has_code;
                stack((
                    label(move || {
                        if e.display_name.is_empty() {
                            e.id.clone()
                        } else {
                            format!("{} ({})", e.display_name, e.id)
                        }
                    })
                    .style(|s| s.font_bold().text_ellipsis().min_width(0.0)),
                    label(move || detail.clone())
                        .style(|s| s.text_ellipsis().min_width(0.0)),
                    stack((
                        label(move || {
                            if code {
                                "Contains code, not loaded".to_string()
                            } else {
                                "Declarative content only".to_string()
                            }
                        })
                        .style(|s| {
                            s.flex_grow(1.0f32).flex_basis(0.0).min_width(0.0)
                        }),
                        button(
                            busy_plugin.common.config,
                            || "Uninstall".to_string(),
                            move || false,
                            move || p.vsx_uninstall(uninstall_id.clone()),
                        ),
                    ))
                    .style(|s| s.width_pct(100.0).items_center()),
                ))
                .style(row_style(&busy_plugin))
            },
        )
        .style(|s| s.flex_col().width_pct(100.0)),
    )
}

/// Search results from Open VSX with install buttons. The search box is the
/// one above the other plugin lists.
pub fn vsx_search_view(plugin: PluginData) -> impl View {
    let results = plugin.vsx.results;
    let status = plugin.vsx.status;
    let list_plugin = plugin.clone();
    let config = plugin.common.config;
    stack((
        label(move || status.get()).style(move |s| {
            s.padding_horiz(10.0)
                .padding_vert(4.0)
                .width_pct(100.0)
                .color(config.get().color(LapceColor::EDITOR_DIM))
        }),
        label(|| {
            "Imports themes, icon themes, grammars and snippets. Extension code is never run."
                .to_string()
        })
        .style(move |s| {
            s.padding_horiz(10.0)
                .padding_bottom(4.0)
                .width_pct(100.0)
                .color(config.get().color(LapceColor::EDITOR_DIM))
        }),
        list_container(
            dyn_stack(
                move || results.get(),
                |e| (e.id(), e.version.clone()),
                move |e| {
                    let plugin = list_plugin.clone();
                    let id = e.id();
                    let installed = plugin.vsx.installed;
                    let busy = plugin.vsx.busy;
                    let (id_a, id_b, id_c) = (id.clone(), id.clone(), id.clone());
                    let click_plugin = plugin.clone();
                    let detail = format!(
                        "{}  v{}  {} downloads{}",
                        e.namespace,
                        e.version,
                        e.download_count,
                        if e.deprecated { "  (deprecated)" } else { "" }
                    );
                    let title = e.title().to_owned();
                    let description = e.description.clone().unwrap_or_default();
                    stack((
                        label(move || title.clone())
                            .style(|s| s.font_bold().text_ellipsis().min_width(0.0)),
                        label(move || description.clone())
                            .style(|s| s.text_ellipsis().min_width(0.0)),
                        stack((
                            label(move || detail.clone()).style(|s| {
                                s.text_ellipsis()
                                    .min_width(0.0)
                                    .flex_grow(1.0f32)
                                    .flex_basis(0.0)
                            }),
                            button(
                                plugin.common.config,
                                move || {
                                    if installed.with(|l| is_installed(l, &id_a)) {
                                        "Installed".to_string()
                                    } else if busy.with(|b| b.contains(&id_a)) {
                                        "Installing".to_string()
                                    } else {
                                        "Install".to_string()
                                    }
                                },
                                move || {
                                    installed.with(|l| is_installed(l, &id_b))
                                        || busy.with(|b| b.contains(&id_b))
                                },
                                move || click_plugin.vsx_install(id_c.clone()),
                            ),
                        ))
                        .style(|s| s.width_pct(100.0).items_center()),
                        empty().style(|s| s.height(0.0)),
                    ))
                    .style(row_style(&plugin))
                },
            )
            .style(|s| s.flex_col().width_pct(100.0)),
        ),
    ))
    .style(|s| {
        s.width_pct(100.0)
            .flex_grow(1.0f32)
            .flex_basis(0.0)
            .flex_col()
    })
}
