//! Editor state, messages and view.
//!
//! Every change to the configuration or the mappings is saved right away
//! and the status line reports the result; the Fcitx5 addon picks the files
//! up when a window next gains focus.

use accents_rapides_core::{
    join_outputs, mappings_path, split_outputs, AppFilterMode, Config, CoreError, KeyMapping,
    LeaderKey, MappingKey, MappingManager, MappingProfile,
};
use accents_rapides_ui::font_for;
use iced::widget::{button, checkbox, column, row, scrollable, text, text_input, Column};
use iced::{Element, Length};
use std::ops::RangeInclusive;

/// Shown where a tab needs a current profile and there is none.
pub const NO_PROFILE: &str = "No profile selected";

/// Status line after a successful save.
pub const SAVED: &str = "Saved";

/// Bounds of the time window, in milliseconds.
pub const TIME_WINDOW_MS: RangeInclusive<u32> = 50..=2000;

/// How much the time window buttons change it, in milliseconds.
const TIME_WINDOW_STEP_MS: u32 = 50;

/// Ids of the text inputs, for tests to find them.
pub mod input_id {
    pub const PROFILE: &str = "new-profile";
    pub const KEY: &str = "new-key";
    pub const OUTPUT: &str = "new-output";
    pub const APP: &str = "new-app";
    pub const CUSTOM_LEADER: &str = "custom-leader";
}

/// The leader keys offered as buttons; the custom one has its own input.
const LEADERS: [(&str, LeaderKey); 7] = [
    ("Space", LeaderKey::Space),
    ("←", LeaderKey::LeftArrow),
    ("→", LeaderKey::RightArrow),
    ("↑", LeaderKey::UpArrow),
    ("↓", LeaderKey::DownArrow),
    ("Alt", LeaderKey::Alt),
    ("AltGr", LeaderKey::AltGr),
];

/// The editor tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Profiles,
    Mappings,
    Settings,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Profiles, Tab::Mappings, Tab::Settings];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Profiles => "Profiles",
            Tab::Mappings => "Mappings",
            Tab::Settings => "Settings",
        }
    }
}

/// Mapping and application rows are addressed by their index in the list.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    SelectTab(Tab),
    NewProfileName(String),
    AddProfile,
    UseProfile(String),
    DeleteProfile(String),
    NewKey(String),
    NewOutput(String),
    AddMapping,
    /// The output spec of a mapping, as typed.
    Output(usize, String),
    Reverse(usize, bool),
    RemoveMapping(usize),
    Leader(LeaderKey),
    TimeWindow(u32),
    ShowOverlay(bool),
    SortByUsage(bool),
    FilterMode(AppFilterMode),
    NewApp(String),
    AddApp,
    RemoveApp(usize),
}

/// The editor: the loaded configuration and mapping profiles, plus what is
/// being typed.
#[derive(Debug, Clone)]
pub struct Editor {
    pub config: Config,
    pub tab: Tab,
    /// Result of the last save.
    pub status: String,
    mappings: MappingManager,
    /// The output spec of each mapping of the current profile, as typed:
    /// `é,` must stay as typed rather than be rebuilt from the variants
    /// parsed from it.
    outputs: Vec<String>,
    new_profile: String,
    new_key: String,
    new_output: String,
    new_app: String,
}

impl Editor {
    pub fn new(config: Config, mappings: MappingManager) -> Self {
        let mut editor = Self {
            config,
            tab: Tab::default(),
            status: String::new(),
            mappings,
            outputs: Vec::new(),
            new_profile: String::new(),
            new_key: String::new(),
            new_output: String::new(),
            new_app: String::new(),
        };
        editor.reload_outputs();
        editor
    }

    pub fn mappings(&self) -> &MappingManager {
        &self.mappings
    }

    /// Apply `message`, saving if it changed the configuration or the
    /// mappings.
    pub fn update(&mut self, message: Message) {
        if self.apply(message) {
            self.save();
        }
    }

    /// Apply `message`; whether it changed the configuration or the
    /// mappings. Stale indices and names change nothing.
    fn apply(&mut self, message: Message) -> bool {
        match message {
            Message::SelectTab(tab) => {
                self.tab = tab;
                false
            }
            Message::NewProfileName(name) => {
                self.new_profile = name;
                false
            }
            Message::AddProfile => self.add_profile(),
            Message::UseProfile(name) => self.use_profile(&name),
            Message::DeleteProfile(name) => self.delete_profile(&name),
            Message::NewKey(key) => {
                self.new_key = key;
                false
            }
            Message::NewOutput(output) => {
                self.new_output = output;
                false
            }
            Message::AddMapping => self.add_mapping(),
            Message::Output(index, output) => self.set_output(index, output),
            Message::Reverse(index, reverse) => self
                .mapping_mut(index)
                .map(|mapping| mapping.reverse_cycle = reverse)
                .is_some(),
            Message::RemoveMapping(index) => self.remove_mapping(index),
            Message::Leader(leader) => {
                self.config.leader_key = leader;
                true
            }
            Message::TimeWindow(ms) => {
                self.config.time_window_ms =
                    ms.clamp(*TIME_WINDOW_MS.start(), *TIME_WINDOW_MS.end());
                true
            }
            Message::ShowOverlay(show) => {
                self.config.show_overlay = show;
                true
            }
            Message::SortByUsage(sort) => {
                self.config.sort_by_usage = sort;
                true
            }
            Message::FilterMode(mode) => {
                self.config.app_filter_mode = mode;
                true
            }
            Message::NewApp(app) => {
                self.new_app = app;
                false
            }
            Message::AddApp => self.add_app(),
            Message::RemoveApp(index) => {
                let listed = index < self.config.filtered_apps.len();
                if listed {
                    self.config.filtered_apps.remove(index);
                }
                listed
            }
        }
    }

    /// Save the configuration and the mappings next to it, reporting the
    /// result in the status line.
    fn save(&mut self) {
        self.status = match self.try_save() {
            Ok(()) => SAVED.to_string(),
            Err(error) => error.to_string(),
        };
    }

    fn try_save(&self) -> Result<(), CoreError> {
        let config_path = self
            .config
            .config_path
            .as_deref()
            .ok_or(CoreError::NoConfigPath)?;
        self.config.save_to_file(config_path)?;
        self.mappings.save_to_file(&mappings_path(config_path))
    }

    /// Rebuild the output drafts from the current profile.
    fn reload_outputs(&mut self) {
        self.outputs = self
            .mappings
            .current_profile()
            .map(|profile| {
                profile
                    .mappings
                    .values()
                    .map(|mapping| join_outputs(&mapping.characters))
                    .collect()
            })
            .unwrap_or_default();
    }

    /// The profile name typed in the add row, if it is new.
    fn new_profile_name(&self) -> Option<&str> {
        let name = self.new_profile.trim();
        (!name.is_empty() && !self.mappings.profiles.contains_key(name)).then_some(name)
    }

    fn add_profile(&mut self) -> bool {
        let Some(name) = self.new_profile_name().map(str::to_string) else {
            return false;
        };
        self.mappings.add_profile(MappingProfile::new(name));
        self.new_profile.clear();
        true
    }

    fn use_profile(&mut self, name: &str) -> bool {
        let used = self.mappings.set_current_profile(name);
        if used {
            self.reload_outputs();
        }
        used
    }

    /// The current profile cannot be deleted.
    fn delete_profile(&mut self, name: &str) -> bool {
        if self.mappings.current_profile.as_deref() == Some(name) {
            return false;
        }
        self.mappings.profiles.shift_remove(name).is_some()
    }

    /// The key typed in the add row, if it parses and is not mapped yet in
    /// the current profile.
    fn new_mapping_key(&self) -> Option<MappingKey> {
        let key: MappingKey = self.new_key.trim().parse().ok()?;
        let profile = self.mappings.current_profile()?;
        (!profile.mappings.contains_key(&key)).then_some(key)
    }

    fn add_mapping(&mut self) -> bool {
        let Some((key, profile)) = self
            .new_mapping_key()
            .zip(self.mappings.current_profile_mut())
        else {
            return false;
        };
        profile
            .mappings
            .insert(key, KeyMapping::new(split_outputs(&self.new_output)));
        self.outputs.push(std::mem::take(&mut self.new_output));
        self.new_key.clear();
        true
    }

    /// The `index`th mapping of the current profile.
    fn mapping_mut(&mut self, index: usize) -> Option<&mut KeyMapping> {
        self.mappings
            .current_profile_mut()?
            .mappings
            .get_index_mut(index)
            .map(|(_, mapping)| mapping)
    }

    fn set_output(&mut self, index: usize, output: String) -> bool {
        let Some(mapping) = self.mapping_mut(index) else {
            return false;
        };
        mapping.characters = split_outputs(&output);
        mapping.cycle_position = 0;
        self.outputs[index] = output;
        true
    }

    fn remove_mapping(&mut self, index: usize) -> bool {
        let removed = self
            .mappings
            .current_profile_mut()
            .and_then(|profile| profile.mappings.shift_remove_index(index))
            .is_some();
        if removed {
            self.outputs.remove(index);
        }
        removed
    }

    /// The application typed in the add row, if it is not listed yet.
    fn new_app_name(&self) -> Option<&str> {
        let app = self.new_app.trim();
        (!app.is_empty() && !self.config.filtered_apps.iter().any(|listed| listed == app))
            .then_some(app)
    }

    fn add_app(&mut self) -> bool {
        let Some(app) = self.new_app_name().map(str::to_string) else {
            return false;
        };
        self.config.filtered_apps.push(app);
        self.new_app.clear();
        true
    }

    pub fn view(&self) -> Element<'_, Message> {
        let tabs =
            row(Tab::ALL.map(|tab| choice(tab.label(), tab == self.tab, Message::SelectTab(tab))))
                .spacing(8);

        let content = match self.tab {
            Tab::Profiles => self.profiles(),
            Tab::Mappings => self.mapping_list(),
            Tab::Settings => self.settings(),
        };

        column![
            text("Accents Rapides Editor").size(28),
            tabs,
            scrollable(content).height(Length::Fill),
            text(&self.status),
        ]
        .spacing(16)
        .padding(20)
        .into()
    }

    /// All profiles, the current one marked; use or delete each, add one.
    fn profiles(&self) -> Element<'_, Message> {
        let current = self.mappings.current_profile.as_deref();
        let rows = self.mappings.profiles.keys().map(|name| {
            let is_current = Some(name.as_str()) == current;
            let label = if is_current {
                format!("{name} (current)")
            } else {
                name.clone()
            };
            row![
                text(label).width(Length::Fill),
                button("Use")
                    .on_press_maybe((!is_current).then(|| Message::UseProfile(name.clone()))),
                button("Delete")
                    .on_press_maybe((!is_current).then(|| Message::DeleteProfile(name.clone()))),
            ]
            .spacing(8)
            .into()
        });
        let add = row![
            text_input("New profile name", &self.new_profile)
                .id(input_id::PROFILE)
                .on_input(Message::NewProfileName),
            button("Add profile")
                .on_press_maybe(self.new_profile_name().map(|_| Message::AddProfile)),
        ]
        .spacing(8);

        column![
            text("Profiles").size(20),
            Column::with_children(rows).spacing(4),
            add,
        ]
        .spacing(8)
        .into()
    }

    /// The mappings of the current profile: key, output spec, reverse
    /// cycling, remove; and a row to add one.
    fn mapping_list(&self) -> Element<'_, Message> {
        let Some(profile) = self.mappings.current_profile() else {
            return text(NO_PROFILE).into();
        };
        let rows = profile.mappings.iter().zip(&self.outputs).enumerate().map(
            |(i, ((key, mapping), output))| {
                row![
                    text(key.to_string()).width(Length::Fixed(100.0)),
                    text_input("Variants, comma-separated", output)
                        .font(font_for(output))
                        .on_input(move |output| Message::Output(i, output)),
                    checkbox(mapping.reverse_cycle)
                        .label("Reverse")
                        .on_toggle(move |reverse| Message::Reverse(i, reverse)),
                    button("Remove").on_press(Message::RemoveMapping(i)),
                ]
                .spacing(8)
                .into()
            },
        );
        let add = row![
            text_input("Key (a, Shift+A, Space)", &self.new_key)
                .id(input_id::KEY)
                .width(Length::Fixed(100.0))
                .on_input(Message::NewKey),
            text_input("Variants, comma-separated", &self.new_output)
                .id(input_id::OUTPUT)
                .font(font_for(&self.new_output))
                .on_input(Message::NewOutput),
            button("Add mapping")
                .on_press_maybe(self.new_mapping_key().map(|_| Message::AddMapping)),
        ]
        .spacing(8);

        column![
            text(format!("Mappings of {}", profile.name)).size(20),
            Column::with_children(rows).spacing(4),
            add,
        ]
        .spacing(8)
        .into()
    }

    fn settings(&self) -> Element<'_, Message> {
        let config = &self.config;

        let custom = match &config.leader_key {
            LeaderKey::Custom(key) => Some(key.as_str()),
            _ => None,
        };
        let leaders = row(LEADERS.iter().map(|(label, leader)| {
            choice(
                label,
                config.leader_key == *leader,
                Message::Leader(leader.clone()),
            )
        }))
        .push(choice(
            "Custom",
            custom.is_some(),
            Message::Leader(LeaderKey::Custom(custom.unwrap_or_default().to_string())),
        ))
        .push(
            text_input("Custom key name", custom.unwrap_or_default())
                .id(input_id::CUSTOM_LEADER)
                .on_input_maybe(custom.map(|_| |key| Message::Leader(LeaderKey::Custom(key)))),
        )
        .spacing(8);

        let window = row![
            text(format!("Time window: {} ms", config.time_window_ms)),
            button("−")
                .on_press_maybe(step_window(config.time_window_ms, false).map(Message::TimeWindow)),
            button("+")
                .on_press_maybe(step_window(config.time_window_ms, true).map(Message::TimeWindow)),
        ]
        .spacing(8);

        let filter = row![
            choice(
                "Blacklist",
                config.app_filter_mode == AppFilterMode::Blacklist,
                Message::FilterMode(AppFilterMode::Blacklist),
            ),
            choice(
                "Whitelist",
                config.app_filter_mode == AppFilterMode::Whitelist,
                Message::FilterMode(AppFilterMode::Whitelist),
            ),
        ]
        .spacing(8);
        let apps = config.filtered_apps.iter().enumerate().map(|(i, app)| {
            row![
                text(app).width(Length::Fill),
                button("Remove").on_press(Message::RemoveApp(i)),
            ]
            .spacing(8)
            .into()
        });
        let add_app = row![
            text_input("Application (e.g. keepassxc)", &self.new_app)
                .id(input_id::APP)
                .on_input(Message::NewApp),
            button("Add application").on_press_maybe(self.new_app_name().map(|_| Message::AddApp)),
        ]
        .spacing(8);

        column![
            text("Leader key").size(20),
            leaders,
            window,
            checkbox(config.show_overlay)
                .label("Show the cycle overlay")
                .on_toggle(Message::ShowOverlay),
            checkbox(config.sort_by_usage)
                .label("Sort variants by usage")
                .on_toggle(Message::SortByUsage),
            text("Application filter").size(20),
            filter,
            Column::with_children(apps).spacing(4),
            add_app,
        ]
        .spacing(8)
        .into()
    }
}

/// A button that looks selected when `selected`: tabs and options.
fn choice<'a>(label: &'a str, selected: bool, message: Message) -> Element<'a, Message> {
    let style = if selected {
        button::primary
    } else {
        button::secondary
    };
    button(label).on_press(message).style(style).into()
}

/// The time window one step up or down from `ms`, if that stays in bounds.
fn step_window(ms: u32, up: bool) -> Option<u32> {
    let stepped = if up {
        ms.saturating_add(TIME_WINDOW_STEP_MS)
    } else {
        ms.saturating_sub(TIME_WINDOW_STEP_MS)
    };
    TIME_WINDOW_MS.contains(&stepped).then_some(stepped)
}
