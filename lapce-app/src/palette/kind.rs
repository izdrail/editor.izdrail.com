use strum_macros::EnumIter;

use crate::command::LapceWorkbenchCommand;

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter)]
pub enum PaletteKind {
    PaletteHelp,
    File,
    FileChooser,
    Line,
    Command,
    Workspace,
    Reference,
    DocumentSymbol,
    WorkspaceSymbol,
    SshHost,
    #[cfg(windows)]
    WslHost,
    RunAndDebug,
    Routes,
    BladeComponents,
    EnvConfigKeys,
    Models,
    Tests,
    Migrations,
    Policies,
    ComponentUsages,
    BladeLayouts,
    FormRequests,
    AppClasses,
    ArtisanCommands,
    ConfigProviders,
    LivewireMembers,
    KeymapPresets,
    Worktrees,
    RemoveWorktree,
    Walkthroughs,
    CssClasses,
    ViewUsages,
    LivewireUsages,
    ImportClass,
    WrapTag,
    RenameTag,
    ColorTheme,
    IconTheme,
    Language,
    LineEnding,
    SCMReferences,
    TerminalProfile,
    DiffFiles,
    HelpAndFile,
}

impl PaletteKind {
    /// The symbol/prefix that is used to signify the behavior of the palette.
    pub fn symbol(&self) -> &'static str {
        match &self {
            PaletteKind::PaletteHelp => "?",
            PaletteKind::Line => "/",
            PaletteKind::DocumentSymbol => "@",
            PaletteKind::WorkspaceSymbol => "#",
            // PaletteKind::GlobalSearch => "?",
            PaletteKind::Workspace => ">",
            PaletteKind::Command => ":",
            PaletteKind::TerminalProfile => "<",
            PaletteKind::File
            | PaletteKind::FileChooser
            | PaletteKind::Reference
            | PaletteKind::SshHost
            | PaletteKind::RunAndDebug
            | PaletteKind::Routes
            | PaletteKind::BladeComponents
            | PaletteKind::EnvConfigKeys
            | PaletteKind::Models
            | PaletteKind::Tests
            | PaletteKind::Migrations
            | PaletteKind::Policies
            | PaletteKind::ComponentUsages
            | PaletteKind::BladeLayouts
            | PaletteKind::FormRequests
            | PaletteKind::AppClasses
            | PaletteKind::ArtisanCommands
            | PaletteKind::ConfigProviders
            | PaletteKind::LivewireMembers
            | PaletteKind::KeymapPresets
            | PaletteKind::Worktrees
            | PaletteKind::RemoveWorktree
            | PaletteKind::Walkthroughs
            | PaletteKind::CssClasses
            | PaletteKind::ViewUsages
            | PaletteKind::LivewireUsages
            | PaletteKind::ImportClass
            | PaletteKind::WrapTag
            | PaletteKind::RenameTag
            | PaletteKind::ColorTheme
            | PaletteKind::IconTheme
            | PaletteKind::Language
            | PaletteKind::LineEnding
            | PaletteKind::SCMReferences
            | PaletteKind::HelpAndFile
            | PaletteKind::DiffFiles => "",
            #[cfg(windows)]
            PaletteKind::WslHost => "",
        }
    }

    /// Extract the palette kind from the input string. This is most often a prefix.
    pub fn from_input(input: &str) -> PaletteKind {
        match input {
            _ if input.starts_with('?') => PaletteKind::PaletteHelp,
            _ if input.starts_with('/') => PaletteKind::Line,
            _ if input.starts_with('@') => PaletteKind::DocumentSymbol,
            _ if input.starts_with('#') => PaletteKind::WorkspaceSymbol,
            _ if input.starts_with('>') => PaletteKind::Workspace,
            _ if input.starts_with(':') => PaletteKind::Command,
            _ if input.starts_with('<') => PaletteKind::TerminalProfile,
            _ => PaletteKind::File,
        }
    }

    /// Get the [`LapceWorkbenchCommand`] that opens this palette kind, if one exists.
    pub fn command(self) -> Option<LapceWorkbenchCommand> {
        match self {
            PaletteKind::PaletteHelp => Some(LapceWorkbenchCommand::PaletteHelp),
            PaletteKind::Line => Some(LapceWorkbenchCommand::PaletteLine),
            PaletteKind::DocumentSymbol => {
                Some(LapceWorkbenchCommand::PaletteSymbol)
            }
            PaletteKind::WorkspaceSymbol => {
                Some(LapceWorkbenchCommand::PaletteWorkspaceSymbol)
            }
            PaletteKind::Workspace => Some(LapceWorkbenchCommand::PaletteWorkspace),
            PaletteKind::Command => Some(LapceWorkbenchCommand::PaletteCommand),
            PaletteKind::File => Some(LapceWorkbenchCommand::Palette),
            PaletteKind::FileChooser => None,
            PaletteKind::HelpAndFile => {
                Some(LapceWorkbenchCommand::PaletteHelpAndFile)
            }
            PaletteKind::Reference => None, // InternalCommand::PaletteReferences
            PaletteKind::SshHost => Some(LapceWorkbenchCommand::ConnectSshHost),
            #[cfg(windows)]
            PaletteKind::WslHost => Some(LapceWorkbenchCommand::ConnectWslHost),
            PaletteKind::RunAndDebug => {
                Some(LapceWorkbenchCommand::PaletteRunAndDebug)
            }
            PaletteKind::Routes => Some(LapceWorkbenchCommand::PaletteRoutes),
            PaletteKind::BladeComponents => {
                Some(LapceWorkbenchCommand::PaletteBladeComponents)
            }
            PaletteKind::EnvConfigKeys => {
                Some(LapceWorkbenchCommand::PaletteEnvConfigKeys)
            }
            PaletteKind::Models => Some(LapceWorkbenchCommand::PaletteModels),
            PaletteKind::Tests => Some(LapceWorkbenchCommand::PaletteTests),
            PaletteKind::Migrations => Some(LapceWorkbenchCommand::PaletteMigrations),
            PaletteKind::Policies => Some(LapceWorkbenchCommand::PalettePolicies),
            PaletteKind::ComponentUsages => {
                Some(LapceWorkbenchCommand::PaletteComponentUsages)
            }
            PaletteKind::BladeLayouts => {
                Some(LapceWorkbenchCommand::PaletteBladeLayouts)
            }
            PaletteKind::FormRequests => {
                Some(LapceWorkbenchCommand::PaletteFormRequests)
            }
            PaletteKind::AppClasses => {
                Some(LapceWorkbenchCommand::PaletteAppClasses)
            }
            PaletteKind::ArtisanCommands => {
                Some(LapceWorkbenchCommand::PaletteArtisanCommands)
            }
            PaletteKind::ConfigProviders => {
                Some(LapceWorkbenchCommand::PaletteConfigProviders)
            }
            PaletteKind::LivewireMembers => {
                Some(LapceWorkbenchCommand::PaletteLivewireMembers)
            }
            PaletteKind::KeymapPresets => {
                Some(LapceWorkbenchCommand::PaletteKeymapPresets)
            }
            PaletteKind::Worktrees => {
                Some(LapceWorkbenchCommand::PaletteWorktrees)
            }
            PaletteKind::RemoveWorktree => {
                Some(LapceWorkbenchCommand::PaletteRemoveWorktree)
            }
            PaletteKind::Walkthroughs => {
                Some(LapceWorkbenchCommand::PaletteWalkthroughs)
            }
            PaletteKind::CssClasses => {
                Some(LapceWorkbenchCommand::PaletteCssClasses)
            }
            PaletteKind::ViewUsages => {
                Some(LapceWorkbenchCommand::PaletteViewUsages)
            }
            PaletteKind::LivewireUsages => {
                Some(LapceWorkbenchCommand::PaletteLivewireUsages)
            }
            PaletteKind::ImportClass => Some(LapceWorkbenchCommand::ImportPhpClass),
            PaletteKind::WrapTag => Some(LapceWorkbenchCommand::WrapSelectionInTag),
            PaletteKind::RenameTag => Some(LapceWorkbenchCommand::RenameTag),
            PaletteKind::ColorTheme => Some(LapceWorkbenchCommand::ChangeColorTheme),
            PaletteKind::IconTheme => Some(LapceWorkbenchCommand::ChangeIconTheme),
            PaletteKind::Language => Some(LapceWorkbenchCommand::ChangeFileLanguage),
            PaletteKind::LineEnding => {
                Some(LapceWorkbenchCommand::ChangeFileLineEnding)
            }
            PaletteKind::SCMReferences => {
                Some(LapceWorkbenchCommand::PaletteSCMReferences)
            }
            PaletteKind::TerminalProfile => None, // InternalCommand::NewTerminal
            PaletteKind::DiffFiles => Some(LapceWorkbenchCommand::DiffFiles),
        }
    }

    // pub fn has_preview(&self) -> bool {
    //     matches!(
    //         self,
    //         PaletteType::Line
    //             | PaletteType::DocumentSymbol
    //             | PaletteType::WorkspaceSymbol
    //             | PaletteType::GlobalSearch
    //             | PaletteType::Reference
    //     )
    // }

    pub fn get_input<'a>(&self, input: &'a str) -> &'a str {
        match self {
            #[cfg(windows)]
            PaletteKind::WslHost => input,
            PaletteKind::File
            | PaletteKind::FileChooser
            | PaletteKind::Reference
            | PaletteKind::SshHost
            | PaletteKind::RunAndDebug
            | PaletteKind::Routes
            | PaletteKind::BladeComponents
            | PaletteKind::EnvConfigKeys
            | PaletteKind::Models
            | PaletteKind::Tests
            | PaletteKind::Migrations
            | PaletteKind::Policies
            | PaletteKind::ComponentUsages
            | PaletteKind::BladeLayouts
            | PaletteKind::FormRequests
            | PaletteKind::AppClasses
            | PaletteKind::ArtisanCommands
            | PaletteKind::ConfigProviders
            | PaletteKind::LivewireMembers
            | PaletteKind::KeymapPresets
            | PaletteKind::Worktrees
            | PaletteKind::RemoveWorktree
            | PaletteKind::Walkthroughs
            | PaletteKind::CssClasses
            | PaletteKind::ViewUsages
            | PaletteKind::LivewireUsages
            | PaletteKind::ImportClass
            | PaletteKind::WrapTag
            | PaletteKind::RenameTag
            | PaletteKind::ColorTheme
            | PaletteKind::IconTheme
            | PaletteKind::Language
            | PaletteKind::LineEnding
            | PaletteKind::SCMReferences | PaletteKind::HelpAndFile
            | PaletteKind::DiffFiles => input,
            PaletteKind::PaletteHelp
            | PaletteKind::Command
            | PaletteKind::Workspace
            | PaletteKind::DocumentSymbol
            | PaletteKind::WorkspaceSymbol
            | PaletteKind::Line
            | PaletteKind::TerminalProfile
            // | PaletteType::GlobalSearch
             => input.get(1..).unwrap_or(""),
        }
    }

    /// Get the palette kind that it should be considered as based on the current
    /// [`PaletteKind`] and the current input.
    pub fn get_palette_kind(&self, input: &str) -> PaletteKind {
        if self == &PaletteKind::HelpAndFile && input.is_empty() {
            return *self;
        }

        if self != &PaletteKind::File
            && self != &PaletteKind::HelpAndFile
            && self.symbol() == ""
        {
            return *self;
        }

        PaletteKind::from_input(input)
    }
}
