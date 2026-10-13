use std::{
    collections::HashSet,
    rc::Rc,
    sync::{Arc, atomic::AtomicU64},
};

use anyhow::Result;
use floem::{
    IntoView, View,
    action::show_context_menu,
    ext_event::create_ext_action,
    keyboard::Modifiers,
    kurbo::Rect,
    menu::{Menu, MenuItem},
    reactive::{
        RwSignal, Scope, SignalGet, SignalUpdate, SignalWith, create_effect,
        create_memo, create_rw_signal, use_context,
    },
    style::CursorStyle,
    views::{
        Decorators, container, dyn_container, dyn_stack, empty, img, label,
        rich_text, scroll, stack, svg, text,
    },
};
use indexmap::IndexMap;
use lapce_core::{command::EditCommand, directory::Directory, mode::Mode};
use lapce_proxy::plugin::{download_volt, find_all_volts, volt_icon};
use lapce_rpc::{
    core::{CoreNotification, CoreRpcHandler},
    plugin::{VoltID, VoltInfo, VoltMetadata},
};
use lsp_types::MessageType;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    command::{CommandExecuted, CommandKind, InternalCommand},
    config::{LapceConfig, color::LapceColor},
    db::LapceDb,
    editor::EditorData,
    keypress::{KeyPressFocus, condition::Condition},
    main_split::Editors,
    markdown::{MarkdownContent, parse_markdown},
    panel::plugin_view::VOLT_DEFAULT_PNG,
    web_link::web_link,
    window_tab::CommonData,
};

type PluginInfo = Option<(
    Option<VoltMetadata>,
    VoltInfo,
    Option<VoltIcon>,
    Option<VoltInfo>,
    Option<RwSignal<bool>>,
)>;

#[derive(Clone, PartialEq, Eq)]
pub enum VoltIcon {
    Svg(String),
    Img(Vec<u8>),
}

impl VoltIcon {
    pub fn from_bytes(buf: &[u8]) -> Result<Self> {
        if let Ok(s) = std::str::from_utf8(buf) {
            Ok(VoltIcon::Svg(s.to_string()))
        } else {
            Ok(VoltIcon::Img(buf.to_vec()))
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct VoltsInfo {
    pub plugins: Vec<VoltInfo>,
    pub total: usize,
}

#[derive(Clone)]
pub struct InstalledVoltData {
    pub meta: RwSignal<VoltMetadata>,
    pub icon: RwSignal<Option<VoltIcon>>,
    pub latest: RwSignal<VoltInfo>,
}

#[derive(Clone, PartialEq)]
pub struct AvailableVoltData {
    pub info: RwSignal<VoltInfo>,
    pub icon: RwSignal<Option<VoltIcon>>,
    pub installing: RwSignal<bool>,
}

#[derive(Clone, Debug)]
pub struct AvailableVoltList {
    pub loading: RwSignal<bool>,
    pub query_id: RwSignal<usize>,
    pub query_editor: EditorData,
    pub volts: RwSignal<IndexMap<VoltID, AvailableVoltData>>,
    pub total: RwSignal<usize>,
}

#[derive(Clone)]
pub struct PluginData {
    pub core_rpc: CoreRpcHandler,
    pub installed: RwSignal<IndexMap<VoltID, InstalledVoltData>>,
    pub available: AvailableVoltList,
    pub all: RwSignal<im::HashMap<VoltID, AvailableVoltData>>,
    pub disabled: RwSignal<HashSet<VoltID>>,
    pub workspace_disabled: RwSignal<HashSet<VoltID>>,
    pub vsx: crate::vsx_market::VsxMarket,
    pub common: Rc<CommonData>,
}

impl std::fmt::Debug for PluginData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginData").finish_non_exhaustive()
    }
}

impl KeyPressFocus for PluginData {
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
                #[allow(clippy::single_match)]
                match command.kind {
                    CommandKind::Edit(EditCommand::InsertNewLine) => {
                        return CommandExecuted::Yes;
                    }
                    _ => {}
                }

                return self
                    .available
                    .query_editor
                    .run_command(command, count, mods);
            }
            CommandKind::MotionMode(_) => {}
        }
        CommandExecuted::No
    }

    fn receive_char(&self, c: &str) {
        self.available.query_editor.receive_char(c);
    }
}

impl PluginData {
    pub fn new(
        cx: Scope,
        disabled: HashSet<VoltID>,
        workspace_disabled: HashSet<VoltID>,
        editors: Editors,
        common: Rc<CommonData>,
        core_rpc: CoreRpcHandler,
    ) -> Self {
        let installed = cx.create_rw_signal(IndexMap::new());
        let available = AvailableVoltList {
            loading: cx.create_rw_signal(false),
            volts: cx.create_rw_signal(IndexMap::new()),
            total: cx.create_rw_signal(0),
            query_id: cx.create_rw_signal(0),
            query_editor: editors.make_local(cx, common.clone()),
        };
        let disabled = cx.create_rw_signal(disabled);
        let workspace_disabled = cx.create_rw_signal(workspace_disabled);

        let plugin = Self {
            core_rpc: core_rpc.clone(),
            installed,
            available,
            all: cx.create_rw_signal(im::HashMap::new()),
            disabled,
            workspace_disabled,
            vsx: crate::vsx_market::VsxMarket::new(cx),
            common,
        };

        plugin.load_available_volts("", 0, core_rpc.clone());
        plugin.vsx_search("");

        {
            let plugin = plugin.clone();
            let extra_plugin_paths =
                plugin.common.window_common.extra_plugin_paths.clone();

            let send = create_ext_action(
                cx,
                move |volts: Vec<(Option<Vec<u8>>, VoltMetadata)>| {
                    for (icon, meta) in volts {
                        plugin.volt_installed(&meta, &icon);
                    }
                },
            );
            std::thread::Builder::new()
                .name("FindAllVolts".to_owned())
                .spawn(move || {
                    let volts = find_all_volts(&extra_plugin_paths);
                    let volts = volts
                        .into_iter()
                        .map(|meta| (volt_icon(&meta), meta))
                        .collect::<Vec<_>>();
                    send(volts);
                })
                .unwrap();
        }

        {
            let plugin = plugin.clone();
            cx.create_effect(move |s| {
                let query = plugin
                    .available
                    .query_editor
                    .doc_signal()
                    .get()
                    .buffer
                    .with(|buffer| buffer.to_string());
                if s.as_ref() == Some(&query) {
                    return query;
                }
                plugin.available.query_id.update(|id| *id += 1);
                plugin.available.loading.set(false);
                plugin.available.volts.update(|v| v.clear());
                plugin.load_available_volts(&query, 0, core_rpc.clone());
                plugin.vsx_search(&query);
                query
            });
        }

        plugin
    }

    pub fn volt_installed(&self, volt: &VoltMetadata, icon: &Option<Vec<u8>>) {
        let volt_id = volt.id();
        let (existing, is_latest, volt_data) = self
            .installed
            .try_update(|installed| {
                if let Some(v) = installed.get(&volt_id) {
                    (true, true, v.to_owned())
                } else {
                    let (info, is_latest) = if let Some(volt) = self
                        .available
                        .volts
                        .with_untracked(|all| all.get(&volt_id).cloned())
                    {
                        (volt.info.get_untracked(), true)
                    } else {
                        (volt.info(), false)
                    };

                    let latest = self.common.scope.create_rw_signal(info);
                    let data = InstalledVoltData {
                        meta: self.common.scope.create_rw_signal(volt.clone()),
                        icon: self.common.scope.create_rw_signal(
                            icon.as_ref()
                                .and_then(|icon| VoltIcon::from_bytes(icon).ok()),
                        ),
                        latest,
                    };
                    installed.insert(volt_id, data.clone());

                    (false, is_latest, data)
                }
            })
            .unwrap();

        if existing {
            volt_data.meta.set(volt.clone());
            volt_data.icon.set(
                icon.as_ref()
                    .and_then(|icon| VoltIcon::from_bytes(icon).ok()),
            );
        }

        let latest = volt_data.latest;
        if !is_latest {
            let url = format!(
                "https://plugins.lapce.dev/api/v1/plugins/{}/{}/latest",
                volt.author, volt.name
            );
            let send = create_ext_action(
                self.common.scope,
                move |info: Option<VoltInfo>| {
                    if let Some(info) = info {
                        latest.set(info);
                    }
                },
            );
            std::thread::spawn(move || {
                let info: Option<VoltInfo> = lapce_proxy::get_url(url, None)
                    .ok()
                    .and_then(|r| r.json().ok());
                send(info);
            });
        }
    }

    pub fn volt_removed(&self, volt: &VoltInfo) {
        let id = volt.id();
        self.installed.update(|installed| {
            installed.swap_remove(&id);
        });

        if self.disabled.with_untracked(|d| d.contains(&id)) {
            self.disabled.update(|d| {
                d.remove(&id);
            });
            let db: Arc<LapceDb> = use_context().unwrap();
            db.save_disabled_volts(
                self.disabled.get_untracked().into_iter().collect(),
            );
        }

        if self.workspace_disabled.with_untracked(|d| d.contains(&id)) {
            self.workspace_disabled.update(|d| {
                d.remove(&id);
            });
            let db: Arc<LapceDb> = use_context().unwrap();
            db.save_workspace_disabled_volts(
                self.common.workspace.clone(),
                self.workspace_disabled
                    .get_untracked()
                    .into_iter()
                    .collect(),
            );
        }
    }

    fn load_available_volts(
        &self,
        query: &str,
        _offset: usize,
        core_rpc: CoreRpcHandler,
    ) {
        if self.available.loading.get_untracked() {
            return;
        }
        self.available.loading.set(true);
        let plugin = self.clone();
        let generation = self.available.query_id.get_untracked();
        let send = create_ext_action(
            self.common.scope,
            move |result: Result<VoltsInfo>| {
                if plugin.available.query_id.get_untracked() != generation {
                    return;
                }
                plugin.available.loading.set(false);
                match result {
                    Ok(result) => {
                        plugin.available.total.set(result.plugins.len());
                        let mut available = IndexMap::new();
                        for info in result.plugins {
                            let data = AvailableVoltData {
                                info: plugin
                                    .common
                                    .scope
                                    .create_rw_signal(info.clone()),
                                icon: plugin.common.scope.create_rw_signal(None),
                                installing: plugin
                                    .common
                                    .scope
                                    .create_rw_signal(false),
                            };
                            let icon = data.icon;
                            let icon_info = info.clone();
                            let icon_send = create_ext_action(
                                plugin.common.scope,
                                move |result: Result<VoltIcon>| {
                                    if let Ok(value) = result {
                                        icon.set(Some(value));
                                    }
                                },
                            );
                            std::thread::spawn(move || {
                                icon_send(Self::load_icon(&icon_info))
                            });
                            available.insert(info.id(), data);
                        }
                        plugin.all.update(|all| {
                            for (id, data) in &available {
                                all.insert(id.clone(), data.clone());
                            }
                        });
                        plugin.available.volts.set(available);
                    }
                    Err(error) => {
                        core_rpc.notification(CoreNotification::ShowMessage {
                            title: "Themes & Icons".to_string(),
                            message: lsp_types::ShowMessageParams {
                                typ: MessageType::ERROR,
                                message: error.to_string(),
                            },
                        })
                    }
                }
            },
        );
        let query = query.to_owned();
        std::thread::spawn(move || send(Self::query_volts(&query, 0)));
    }

    fn load_icon(volt: &VoltInfo) -> Result<VoltIcon> {
        let url = format!(
            "https://plugins.lapce.dev/api/v1/plugins/{}/{}/{}/icon?id={}",
            volt.author, volt.name, volt.version, volt.updated_at_ts
        );

        let cache_file_path = Directory::cache_directory().map(|cache_dir| {
            let mut hasher = Sha256::new();
            hasher.update(url.as_bytes());
            let filename = format!("{:x}", hasher.finalize());
            cache_dir.join(filename)
        });

        let cache_content =
            cache_file_path.as_ref().and_then(|p| std::fs::read(p).ok());

        let content = match cache_content {
            Some(content) => content,
            None => {
                let resp = lapce_proxy::get_url(&url, None)?;
                if !resp.status().is_success() {
                    return Err(anyhow::anyhow!("can't download icon"));
                }
                let buf = resp.bytes()?.to_vec();

                if let Some(path) = cache_file_path.as_ref() {
                    if let Err(err) = std::fs::write(path, &buf) {
                        tracing::error!("{:?}", err);
                    }
                }

                buf
            }
        };

        VoltIcon::from_bytes(&content)
    }

    fn download_readme(
        volt: &VoltInfo,
        config: &LapceConfig,
    ) -> Result<Vec<MarkdownContent>> {
        let url = format!(
            "https://plugins.lapce.dev/api/v1/plugins/{}/{}/{}/readme",
            volt.author, volt.name, volt.version
        );
        let resp = lapce_proxy::get_url(&url, None)?;
        if resp.status() != 200 {
            let text = parse_markdown("Plugin doesn't have a README", 2.0, config);
            return Ok(text);
        }
        let text = resp.text()?;
        let text = parse_markdown(&text, 2.0, config);
        Ok(text)
    }

    fn query_volts(query: &str, _offset: usize) -> Result<VoltsInfo> {
        // Consume server pagination before filtering: counting visible entries as
        // the next server offset would repeat/skip entries and never finish.
        let mut plugins = Vec::new();
        let mut offset = 0usize;
        for _ in 0..100 {
            let mut url =
                url::Url::parse("https://plugins.lapce.dev/api/v1/plugins")?;
            url.query_pairs_mut()
                .append_pair("q", query)
                .append_pair("offset", &offset.to_string());
            let page: VoltsInfo = lapce_proxy::get_url(url, None)?
                .error_for_status()?
                .json()?;
            let count = page.plugins.len();
            offset += count;
            plugins.extend(page.plugins);
            if count == 0 || offset >= page.total {
                return Ok(VoltsInfo {
                    total: plugins.len(),
                    plugins,
                });
            }
        }
        anyhow::bail!("Theme catalog exceeds pagination limit")
    }

    fn all_loaded(&self) -> bool {
        self.available.volts.with_untracked(|v| v.len())
            >= self.available.total.get_untracked()
    }

    pub fn load_more_available(&self, core_rpc: CoreRpcHandler) {
        if self.all_loaded() {
            return;
        }

        let query = self
            .available
            .query_editor
            .doc()
            .buffer
            .with_untracked(|buffer| buffer.to_string());
        let offset = self.available.volts.with_untracked(|v| v.len());
        self.load_available_volts(&query, offset, core_rpc);
    }

    pub fn install_volt(&self, info: VoltInfo) {
        let installing = self
            .all
            .with_untracked(|all| all.get(&info.id()).map(|data| data.installing));
        if let Some(signal) = installing {
            if signal.get_untracked() {
                return;
            }
            signal.set(true);
        }
        let plugin = self.clone();
        let send = create_ext_action(
            self.common.scope,
            move |result: Result<(VoltMetadata, Option<Vec<u8>>)>| {
                if let Some(signal) = installing {
                    signal.set(false);
                }
                match result {
                    Ok((meta, icon)) => {
                        plugin.volt_installed(&meta, &icon);
                        plugin
                            .common
                            .internal_command
                            .send(InternalCommand::ReloadConfig);
                    }
                    Err(error) => {
                        plugin.core_rpc.notification(CoreNotification::ShowMessage {
                            title: "Themes & Icons".to_string(),
                            message: lsp_types::ShowMessageParams {
                                typ: MessageType::ERROR,
                                message: error.to_string(),
                            },
                        })
                    }
                }
            },
        );
        if info.wasm {
            // The proxy downloads, installs and starts the plugin, then
            // reports back with CoreNotification::VoltInstalled.
            if let Some(signal) = installing {
                signal.set(false);
            }
            self.common.proxy.install_volt(info);
            return;
        }
        std::thread::spawn(move || {
            let result = download_volt(&info).map(|meta| {
                let icon = volt_icon(&meta);
                (meta, icon)
            });
            send(result);
        });
    }

    pub fn plugin_disabled(&self, id: &VoltID) -> bool {
        self.disabled.with_untracked(|d| d.contains(id))
            || self.workspace_disabled.with_untracked(|d| d.contains(id))
    }

    pub fn enable_volt(&self, volt: VoltInfo) {
        let id = volt.id();
        self.disabled.update(|d| {
            d.remove(&id);
        });
        if !self.plugin_disabled(&id) {
            self.common.proxy.enable_volt(volt.clone());
        }
        let db: Arc<LapceDb> = use_context().unwrap();
        db.save_disabled_volts(self.disabled.get_untracked().into_iter().collect());
        self.common
            .internal_command
            .send(InternalCommand::ReloadConfig);
    }

    pub fn disable_volt(&self, volt: VoltInfo) {
        let id = volt.id();
        self.disabled.update(|d| {
            d.insert(id);
        });
        self.common.proxy.disable_volt(volt.clone());
        let db: Arc<LapceDb> = use_context().unwrap();
        db.save_disabled_volts(self.disabled.get_untracked().into_iter().collect());
        self.common
            .internal_command
            .send(InternalCommand::ReloadConfig);
    }

    pub fn enable_volt_for_ws(&self, volt: VoltInfo) {
        let id = volt.id();
        self.workspace_disabled.update(|d| {
            d.remove(&id);
        });
        let db: Arc<LapceDb> = use_context().unwrap();
        db.save_workspace_disabled_volts(
            self.common.workspace.clone(),
            self.workspace_disabled
                .get_untracked()
                .into_iter()
                .collect(),
        );
        self.common
            .internal_command
            .send(InternalCommand::ReloadConfig);
    }

    pub fn disable_volt_for_ws(&self, volt: VoltInfo) {
        let id = volt.id();
        self.workspace_disabled.update(|d| {
            d.insert(id);
        });
        let db: Arc<LapceDb> = use_context().unwrap();
        db.save_workspace_disabled_volts(
            self.common.workspace.clone(),
            self.workspace_disabled
                .get_untracked()
                .into_iter()
                .collect(),
        );
        self.common
            .internal_command
            .send(InternalCommand::ReloadConfig);
    }

    pub fn uninstall_volt(&self, volt: VoltMetadata) {
        if volt.wasm.is_some() {
            self.common.proxy.remove_volt(volt);
        } else {
            let plugin = self.clone();
            let info = volt.info();
            let send =
                create_ext_action(self.common.scope, move |result: Result<()>| {
                    match result {
                        Ok(()) => {
                            plugin.volt_removed(&info);
                            plugin
                                .common
                                .internal_command
                                .send(InternalCommand::ReloadConfig);
                        }
                        Err(error) => plugin.core_rpc.notification(
                            CoreNotification::ShowMessage {
                                title: "Themes & Icons".to_string(),
                                message: lsp_types::ShowMessageParams {
                                    typ: MessageType::ERROR,
                                    message: error.to_string(),
                                },
                            },
                        ),
                    }
                });
            std::thread::spawn(move || {
                let uninstall = || -> Result<()> {
                    let path = volt
                        .dir
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("don't have dir"))?;
                    let root = Directory::plugins_directory()
                        .ok_or_else(|| anyhow::anyhow!("No package directory"))?
                        .canonicalize()?;
                    let path = path.canonicalize()?;
                    anyhow::ensure!(
                        path.parent() == Some(root.as_path())
                            && path.file_name().and_then(|s| s.to_str())
                                == Some(volt.id().to_string().as_str()),
                        "Cannot uninstall a development theme outside the package directory"
                    );
                    std::fs::remove_dir_all(path)?;
                    Ok(())
                };
                send(uninstall());
            });
        }
    }

    pub fn reload_volt(&self, _volt: VoltMetadata) {
        self.common
            .internal_command
            .send(InternalCommand::ReloadConfig);
    }

    pub fn plugin_controls(&self, meta: VoltMetadata, latest: VoltInfo) -> Menu {
        let volt_id = meta.id();
        let mut menu = Menu::new("");
        if meta.version != latest.version {
            menu = menu
                .entry(MenuItem::new("Upgrade Theme").action({
                    let plugin = self.clone();
                    let info = latest.clone();
                    move || {
                        plugin.install_volt(info.clone());
                    }
                }))
                .separator();
        }
        menu = menu
            .entry(MenuItem::new("Reload Themes").action({
                let plugin = self.clone();
                let meta = meta.clone();
                move || {
                    plugin.reload_volt(meta.clone());
                }
            }))
            .separator()
            .entry(
                MenuItem::new("Enable")
                    .enabled(
                        self.disabled
                            .with_untracked(|disabled| disabled.contains(&volt_id)),
                    )
                    .action({
                        let plugin = self.clone();
                        let volt = meta.info();
                        move || {
                            plugin.enable_volt(volt.clone());
                        }
                    }),
            )
            .entry(
                MenuItem::new("Disable")
                    .enabled(
                        self.disabled
                            .with_untracked(|disabled| !disabled.contains(&volt_id)),
                    )
                    .action({
                        let plugin = self.clone();
                        let volt = meta.info();
                        move || {
                            plugin.disable_volt(volt.clone());
                        }
                    }),
            )
            .separator()
            .entry(
                MenuItem::new("Enable For Workspace")
                    .enabled(
                        self.workspace_disabled
                            .with_untracked(|disabled| disabled.contains(&volt_id)),
                    )
                    .action({
                        let plugin = self.clone();
                        let volt = meta.info();
                        move || {
                            plugin.enable_volt_for_ws(volt.clone());
                        }
                    }),
            )
            .entry(
                MenuItem::new("Disable For Workspace")
                    .enabled(
                        self.workspace_disabled
                            .with_untracked(|disabled| !disabled.contains(&volt_id)),
                    )
                    .action({
                        let plugin = self.clone();
                        let volt = meta.info();
                        move || {
                            plugin.disable_volt_for_ws(volt.clone());
                        }
                    }),
            )
            .separator()
            .entry(MenuItem::new("Uninstall").action({
                let plugin = self.clone();
                move || {
                    plugin.uninstall_volt(meta.clone());
                }
            }));
        menu
    }
}

pub fn plugin_info_view(plugin: PluginData, volt: VoltID) -> impl View {
    let config = plugin.common.config;
    let header_rect = create_rw_signal(Rect::ZERO);
    let scroll_width: RwSignal<f64> = create_rw_signal(0.0);
    let internal_command = plugin.common.internal_command;
    let local_plugin = plugin.clone();
    let plugin_info = create_memo(move |_| {
        plugin
            .installed
            .with(|volts| {
                volts.get(&volt).map(|v| {
                    (
                        Some(v.meta.get()),
                        v.meta.get().info(),
                        v.icon.get(),
                        Some(v.latest.get()),
                        None,
                    )
                })
            })
            .or_else(|| {
                plugin.all.with(|volts| {
                    volts.get(&volt).map(|v| {
                        (None, v.info.get(), v.icon.get(), None, Some(v.installing))
                    })
                })
            })
    });

    let version_view = move |plugin: PluginData, plugin_info: PluginInfo| {
        let version_info = plugin_info.as_ref().map(|(_, volt, _, latest, _)| {
            (
                volt.version.clone(),
                latest.as_ref().map(|i| i.version.clone()),
            )
        });
        let installing = plugin_info
            .as_ref()
            .and_then(|(_, _, _, _, installing)| *installing);
        let local_version_info = version_info.clone();
        let control = {
            move |version_info: Option<(String, Option<String>)>| match version_info
                .as_ref()
                .map(|(v, l)| match l {
                    Some(l) => (true, l == v),
                    None => (false, false),
                }) {
                Some((true, true)) => "Installed ▼",
                Some((true, false)) => "Upgrade ▼",
                _ => {
                    if installing.map(|i| i.get()).unwrap_or(false) {
                        "Installing"
                    } else {
                        "Install"
                    }
                }
            }
        };
        let local_plugin_info = plugin_info.clone();
        let local_plugin = plugin.clone();
        stack((
            text(
                version_info
                    .as_ref()
                    .map(|(v, _)| format!("v{v}"))
                    .unwrap_or_default(),
            ),
            label(move || control(local_version_info.clone()))
                .style(move |s| {
                    let config = config.get();
                    s.margin_left(10)
                        .padding_horiz(10)
                        .border_radius(6.0)
                        .color(
                            config
                                .color(LapceColor::LAPCE_BUTTON_PRIMARY_FOREGROUND),
                        )
                        .background(
                            config
                                .color(LapceColor::LAPCE_BUTTON_PRIMARY_BACKGROUND),
                        )
                        .hover(|s| {
                            s.cursor(CursorStyle::Pointer).background(
                                config
                                    .color(
                                        LapceColor::LAPCE_BUTTON_PRIMARY_BACKGROUND,
                                    )
                                    .multiply_alpha(0.8),
                            )
                        })
                        .active(|s| {
                            s.background(
                                config
                                    .color(
                                        LapceColor::LAPCE_BUTTON_PRIMARY_BACKGROUND,
                                    )
                                    .multiply_alpha(0.6),
                            )
                        })
                        .disabled(|s| {
                            s.background(config.color(LapceColor::EDITOR_DIM))
                        })
                        .selectable(false)
                })
                .disabled(move || installing.map(|i| i.get()).unwrap_or(false))
                .on_click_stop(move |_| {
                    if let Some((meta, info, _, latest, _)) =
                        local_plugin_info.as_ref()
                    {
                        if let Some(meta) = meta {
                            let menu = local_plugin.plugin_controls(
                                meta.to_owned(),
                                latest.clone().unwrap_or_else(|| info.to_owned()),
                            );
                            show_context_menu(menu, None);
                        } else {
                            local_plugin.install_volt(info.to_owned());
                        }
                    }
                }),
        ))
    };

    scroll(
        dyn_container(
            move || plugin_info.get(),
            move |plugin_info| {
                stack((
                    stack((
                        match plugin_info
                            .as_ref()
                            .and_then(|(_, _, icon, _, _)| icon.clone())
                        {
                            None => container(
                                img(move || VOLT_DEFAULT_PNG.to_vec())
                                    .style(|s| s.size_full()),
                            ),
                            Some(VoltIcon::Svg(svg_str)) => container(
                                svg(move || svg_str.clone())
                                    .style(|s| s.size_full()),
                            ),
                            Some(VoltIcon::Img(buf)) => container(
                                img(move || buf.clone()).style(|s| s.size_full()),
                            ),
                        }
                        .style(|s| {
                            s.min_size(150.0, 150.0).size(150.0, 150.0).padding(20)
                        }),
                        stack((
                            text(
                                plugin_info
                                    .as_ref()
                                    .map(|(_, volt, _, _, _)| {
                                        volt.display_name.as_str()
                                    })
                                    .unwrap_or(""),
                            )
                            .style(move |s| {
                                s.font_bold().font_size(
                                    (config.get().ui.font_size() as f32 * 1.6)
                                        .round(),
                                )
                            }),
                            text(
                                plugin_info
                                    .as_ref()
                                    .map(|(_, volt, _, _, _)| {
                                        volt.description.as_str()
                                    })
                                    .unwrap_or(""),
                            )
                            .style(move |s| {
                                let scroll_width = scroll_width.get();
                                s.max_width(
                                    scroll_width
                                        .clamp(200.0 + 60.0 * 2.0 + 200.0, 800.0)
                                        - 60.0 * 2.0
                                        - 200.0,
                                )
                            }),
                            {
                                let repo = plugin_info
                                    .as_ref()
                                    .and_then(|(_, volt, _, _, _)| {
                                        volt.repository.as_deref()
                                    })
                                    .unwrap_or("")
                                    .to_string();
                                let local_repo = repo.clone();
                                stack((
                                    text("Repository: "),
                                    web_link(
                                        move || repo.clone(),
                                        move || local_repo.clone(),
                                        move || {
                                            config
                                                .get()
                                                .color(LapceColor::EDITOR_LINK)
                                        },
                                        internal_command,
                                    ),
                                ))
                            },
                            text(
                                plugin_info
                                    .as_ref()
                                    .map(|(_, volt, _, _, _)| volt.author.as_str())
                                    .unwrap_or(""),
                            )
                            .style(move |s| {
                                s.color(config.get().color(LapceColor::EDITOR_DIM))
                            }),
                            version_view(local_plugin.clone(), plugin_info.clone()),
                        ))
                        .style(|s| s.flex_col().line_height(1.6)),
                    ))
                    .style(|s| s.absolute())
                    .on_resize(move |rect| {
                        if header_rect.get_untracked() != rect {
                            header_rect.set(rect);
                        }
                    }),
                    empty().style(move |s| {
                        let rect = header_rect.get();
                        s.size(rect.width(), rect.height()).pointer_events_none()
                    }),
                    empty().style(move |s| {
                        s.margin_vert(6)
                            .height(1)
                            .width_full()
                            .background(config.get().color(LapceColor::LAPCE_BORDER))
                    }),
                    {
                        let readme = create_rw_signal(None);
                        let info = plugin_info
                            .as_ref()
                            .map(|(_, info, _, _, _)| info.to_owned());
                        create_effect(move |_| {
                            let config = config.get();
                            let info = info.clone();
                            if let Some(info) = info {
                                let cx = Scope::current();
                                let send = create_ext_action(cx, move |result| {
                                    if let Ok(md) = result {
                                        readme.set(Some(md));
                                    }
                                });
                                std::thread::spawn(move || {
                                    let result =
                                        PluginData::download_readme(&info, &config);
                                    send(result);
                                });
                            }
                        });
                        {
                            let id = AtomicU64::new(0);
                            dyn_stack(
                                move || {
                                    readme.get().unwrap_or_else(|| {
                                        parse_markdown(
                                            "Loading README",
                                            2.0,
                                            &config.get(),
                                        )
                                    })
                                },
                                move |_| {
                                    id.fetch_add(
                                        1,
                                        std::sync::atomic::Ordering::Relaxed,
                                    )
                                },
                                move |content| match content {
                                    MarkdownContent::Text(text_layout) => container(
                                        rich_text(move || text_layout.clone())
                                            .style(|s| s.width_full()),
                                    )
                                    .style(|s| s.width_full()),
                                    MarkdownContent::Image { .. } => {
                                        container(empty())
                                    }
                                    MarkdownContent::Separator => {
                                        container(empty().style(move |s| {
                                            s.width_full()
                                                .margin_vert(5.0)
                                                .height(1.0)
                                                .background(
                                                    config.get().color(
                                                        LapceColor::LAPCE_BORDER,
                                                    ),
                                                )
                                        }))
                                    }
                                },
                            )
                            .style(|s| s.flex_col().width_full())
                        }
                    },
                ))
                .style(move |s| {
                    let padding = 60.0;
                    s.flex_col()
                        .width(
                            scroll_width
                                .get()
                                .min(800.0)
                                .max(header_rect.get().width() + padding * 2.0),
                        )
                        .padding(padding)
                })
                .into_any()
            },
        )
        .style(|s| s.min_width_full().justify_center()),
    )
    .on_resize(move |rect| {
        if scroll_width.get_untracked() != rect.width() {
            scroll_width.set(rect.width());
        }
    })
    .style(|s| s.absolute().size_full())
    .debug_name("Plugin Info")
}
