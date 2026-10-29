//! The editor's theme: the schnelle-umlaute palette, as a custom iced theme.
//!
//! The colours are the ones of the original Qt editor's "schnelle-umlaute"
//! palette (addon/palette/Palettes.qml of Maik-0000FF/schnelle-umlaute):
//! a near-black background, slightly lifted surface cards, a purple accent
//! and a green brand, with muted secondary text. iced derives the rest of
//! its palette (hover/pressed states, scrollbars, disabled looks) from the
//! few colours given here; the widgets that need more than that (buttons,
//! inputs, rows) are styled explicitly in the `style` functions below.
//!
//! Only the editor uses this theme; the overlay is unchanged.

use iced::widget::{button, checkbox, container, rule, scrollable, text_input};
use iced::{color, Border, Color, Shadow, Theme};

/// The page background: `#08060f`.
pub const BACKGROUND: Color = color!(0x08060f);
/// The card surface: `#12101d`.
pub const SURFACE: Color = color!(0x12101d);
/// The hovered card/row surface: `#1a1728`.
pub const SURFACE_HOVER: Color = color!(0x1a1728);
/// The resting border: `#2a2640`.
pub const BORDER: Color = color!(0x2a2640);
/// The focused border: `#4a3f70`.
pub const BORDER_FOCUS: Color = color!(0x4a3f70);
/// The accent (purple): `#a855f7`.
pub const ACCENT: Color = color!(0xa855f7);
/// The hovered accent: `#c084fc`.
pub const ACCENT_HOVER: Color = color!(0xc084fc);
/// The accent at 20% opacity, for selected pills: `#a855f733`.
pub const ACCENT_SOFT: Color = color!(0xa855f7, 0.2);
/// The brand (green): `#4ade80`.
pub const BRAND: Color = color!(0x4ade80);
/// The hovered brand: `#86efac`.
pub const BRAND_HOVER: Color = color!(0x86efac);
/// The brand at 20% opacity: `#4ade8033`.
pub const BRAND_SOFT: Color = color!(0x4ade80, 0.2);
/// The primary text: `#f0fdf4`.
pub const TEXT: Color = color!(0xf0fdf4);
/// The muted secondary text: `#6b7280`.
pub const TEXT_MUTED: Color = color!(0x6b7280);
/// Success: `#4ade80` (the brand green).
pub const SUCCESS: Color = color!(0x4ade80);
/// Warning: `#fbbf24`.
pub const WARNING: Color = color!(0xfbbf24);
/// Danger (Remove, Delete, errors): `#f87171`.
pub const DANGER: Color = color!(0xf87171);
/// The hovered danger: `#ef4444`.
pub const DANGER_HOVER: Color = color!(0xef4444);
/// Text on an accent-filled surface: `#ffffff`.
pub const ACCENT_TEXT: Color = color!(0xffffff);

/// The corner radius of cards and inputs, in pixels.
pub const RADIUS: f32 = 8.0;

/// The editor's theme.
pub fn theme() -> Theme {
    Theme::custom(
        "schnelle-umlaute",
        iced::theme::Palette {
            background: BACKGROUND,
            text: TEXT,
            primary: ACCENT,
            success: SUCCESS,
            warning: WARNING,
            danger: DANGER,
        },
    )
}

/// A rounded border in `color`.
fn border(color: Color) -> Border {
    Border {
        color,
        width: 1.0,
        radius: RADIUS.into(),
    }
}

/// The card surface: cards and list rows.
pub fn card(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT),
        background: Some(SURFACE.into()),
        border: border(BORDER),
        shadow: Shadow::default(),
        snap: false,
    }
}

/// A flat surface: the page behind the cards and the segmented tab bar.
pub fn page(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT),
        background: Some(BACKGROUND.into()),
        border: Border::default(),
        shadow: Shadow::default(),
        snap: false,
    }
}

/// A pressed-in well, e.g. the segmented tab bar holding the tabs.
pub fn well(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT),
        background: Some(BACKGROUND.into()),
        border: border(BORDER),
        shadow: Shadow::default(),
        snap: false,
    }
}

/// The primary button: accent-filled (Add, Use).
pub fn primary_button(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => ACCENT_HOVER,
        button::Status::Pressed => ACCENT,
        button::Status::Active | button::Status::Disabled => ACCENT,
    };
    button::Style {
        background: Some(background.into()),
        text_color: ACCENT_TEXT,
        border: Border {
            radius: RADIUS.into(),
            ..Border::default()
        },
        shadow: Shadow::default(),
        snap: false,
    }
}

/// A neutral button on a card: surface with a border (Remove, Delete, +/−).
pub fn secondary_button(_theme: &Theme, status: button::Status) -> button::Style {
    let (background, border_color) = match status {
        button::Status::Hovered => (SURFACE_HOVER, BORDER_FOCUS),
        button::Status::Pressed => (SURFACE, BORDER_FOCUS),
        button::Status::Active => (SURFACE, BORDER),
        button::Status::Disabled => (SURFACE, BORDER),
    };
    button::Style {
        background: Some(background.into()),
        text_color: TEXT,
        border: border(border_color),
        shadow: Shadow::default(),
        snap: false,
    }
}

/// A destructive button (Remove, Delete): danger text, danger border on hover.
pub fn danger_button(_theme: &Theme, status: button::Status) -> button::Style {
    let (background, border_color) = match status {
        button::Status::Hovered => (color!(0xf87171, 0.15), DANGER_HOVER),
        button::Status::Pressed => (color!(0xf87171, 0.10), DANGER),
        button::Status::Active => (Color::TRANSPARENT, DANGER),
        button::Status::Disabled => (Color::TRANSPARENT, BORDER),
    };
    button::Style {
        background: Some(background.into()),
        text_color: if status == button::Status::Disabled {
            TEXT_MUTED
        } else {
            DANGER
        },
        border: border(border_color),
        shadow: Shadow::default(),
        snap: false,
    }
}

/// A selected choice (the active tab, the chosen leader): accent-soft pill
/// with accent text.
pub fn selected_button(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => color!(0xa855f7, 0.3),
        button::Status::Pressed | button::Status::Active | button::Status::Disabled => ACCENT_SOFT,
    };
    button::Style {
        background: Some(background.into()),
        text_color: ACCENT_HOVER,
        border: border(ACCENT),
        shadow: Shadow::default(),
        snap: false,
    }
}

/// A text input on a card.
pub fn input(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        text_input::Status::Hovered => BORDER_FOCUS,
        text_input::Status::Focused { .. } => ACCENT,
        text_input::Status::Active | text_input::Status::Disabled => BORDER,
    };
    text_input::Style {
        background: BACKGROUND.into(),
        border: border(border_color),
        icon: TEXT_MUTED,
        placeholder: TEXT_MUTED,
        value: TEXT,
        selection: ACCENT_SOFT,
    }
}

/// A checkbox (Reverse, the toggles): accent box when checked.
pub fn checkbox_style(_theme: &Theme, status: checkbox::Status) -> checkbox::Style {
    let (background, border_color) = match status {
        checkbox::Status::Hovered { is_checked } => {
            if is_checked {
                (ACCENT_HOVER, ACCENT_HOVER)
            } else {
                (BACKGROUND, BORDER_FOCUS)
            }
        }
        checkbox::Status::Active { is_checked } => {
            if is_checked {
                (ACCENT, ACCENT)
            } else {
                (BACKGROUND, BORDER)
            }
        }
        checkbox::Status::Disabled { is_checked } => {
            if is_checked {
                (color!(0xa855f7, 0.4), color!(0xa855f7, 0.4))
            } else {
                (BACKGROUND, BORDER)
            }
        }
    };
    checkbox::Style {
        background: background.into(),
        icon_color: ACCENT_TEXT,
        border: border(border_color),
        text_color: Some(TEXT),
    }
}

/// The scrollable: no container chrome, a thin muted scrollbar.
pub fn scrollable_style(_theme: &Theme, _status: scrollable::Status) -> scrollable::Style {
    let rail = |scroller: Color| scrollable::Rail {
        background: Some(Color::TRANSPARENT.into()),
        border: Border::default(),
        scroller: scrollable::Scroller {
            background: scroller.into(),
            border: Border {
                radius: 4.0.into(),
                ..Border::default()
            },
        },
    };
    scrollable::Style {
        container: container::Style {
            text_color: Some(TEXT),
            background: None,
            border: Border::default(),
            shadow: Shadow::default(),
            snap: false,
        },
        vertical_rail: rail(BORDER_FOCUS),
        horizontal_rail: rail(BORDER_FOCUS),
        gap: None,
        auto_scroll: scrollable::AutoScroll {
            background: SURFACE.into(),
            border: border(BORDER),
            shadow: Shadow::default(),
            icon: TEXT,
        },
    }
}

/// A key chip: an accent-soft pill with accent text, monospace in the view.
pub fn key_chip(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(ACCENT_HOVER),
        background: Some(ACCENT_SOFT.into()),
        border: border(ACCENT),
        shadow: Shadow::default(),
        snap: false,
    }
}

/// The empty status line: invisible, keeps the layout steady.
pub fn status_idle(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT_MUTED),
        background: None,
        border: Border::default(),
        shadow: Shadow::default(),
        snap: false,
    }
}

/// The status pill after a successful save: brand-soft with brand text.
pub fn status_saved(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(BRAND_HOVER),
        background: Some(BRAND_SOFT.into()),
        border: border(BRAND),
        shadow: Shadow::default(),
        snap: false,
    }
}

/// The status pill when saving failed: danger-soft with danger text.
pub fn status_error(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(DANGER_HOVER),
        background: Some(color!(0xf87171, 0.15).into()),
        border: border(DANGER),
        shadow: Shadow::default(),
        snap: false,
    }
}

/// A hairline separator.
pub fn rule_style(_theme: &Theme) -> rule::Style {
    rule::Style {
        color: BORDER,
        radius: 0.0.into(),
        fill_mode: rule::FillMode::Full,
        snap: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_has_the_schnelle_umlaute_palette() {
        let theme = theme();
        assert_eq!(theme.to_string(), "schnelle-umlaute");
        let palette = theme.palette();
        assert_eq!(palette.background, BACKGROUND);
        assert_eq!(palette.text, TEXT);
        assert_eq!(palette.primary, ACCENT);
        assert_eq!(palette.success, SUCCESS);
        assert_eq!(palette.warning, WARNING);
        assert_eq!(palette.danger, DANGER);
        assert!(theme.extended_palette().is_dark);
    }

    #[test]
    fn cards_are_surfaces_with_a_border() {
        let style = card(&theme());
        assert_eq!(style.background, Some(SURFACE.into()));
        assert_eq!(style.text_color, Some(TEXT));
        assert_eq!(style.border.color, BORDER);
        assert_eq!(style.border.radius, RADIUS.into());
    }

    #[test]
    fn page_and_well_are_flat() {
        let style = page(&theme());
        assert_eq!(style.background, Some(BACKGROUND.into()));
        assert_eq!(style.border.color, Color::TRANSPARENT);
        let style = well(&theme());
        assert_eq!(style.background, Some(BACKGROUND.into()));
        assert_eq!(style.border.color, BORDER);
    }

    #[test]
    fn primary_button_is_accent_filled_in_every_status() {
        for status in [
            button::Status::Active,
            button::Status::Hovered,
            button::Status::Pressed,
            button::Status::Disabled,
        ] {
            let style = primary_button(&theme(), status);
            assert_eq!(style.text_color, ACCENT_TEXT);
            let expected = match status {
                button::Status::Hovered => ACCENT_HOVER,
                _ => ACCENT,
            };
            assert_eq!(style.background, Some(expected.into()), "{status:?}");
        }
    }

    #[test]
    fn secondary_button_uses_surface_and_border() {
        let style = secondary_button(&theme(), button::Status::Active);
        assert_eq!(style.background, Some(SURFACE.into()));
        assert_eq!(style.text_color, TEXT);
        assert_eq!(style.border.color, BORDER);
        let style = secondary_button(&theme(), button::Status::Hovered);
        assert_eq!(style.background, Some(SURFACE_HOVER.into()));
        assert_eq!(style.border.color, BORDER_FOCUS);
        let style = secondary_button(&theme(), button::Status::Pressed);
        assert_eq!(style.background, Some(SURFACE.into()));
        assert_eq!(style.border.color, BORDER_FOCUS);
        let style = secondary_button(&theme(), button::Status::Disabled);
        assert_eq!(style.background, Some(SURFACE.into()));
        assert_eq!(style.border.color, BORDER);
    }

    #[test]
    fn danger_button_is_danger_text_with_a_danger_border_on_hover() {
        let style = danger_button(&theme(), button::Status::Active);
        assert_eq!(style.text_color, DANGER);
        assert_eq!(style.background, Some(Color::TRANSPARENT.into()));
        assert_eq!(style.border.color, DANGER);
        let style = danger_button(&theme(), button::Status::Hovered);
        assert_eq!(style.text_color, DANGER);
        assert_eq!(style.background, Some(color!(0xf87171, 0.15).into()));
        assert_eq!(style.border.color, DANGER_HOVER);
        let style = danger_button(&theme(), button::Status::Pressed);
        assert_eq!(style.background, Some(color!(0xf87171, 0.10).into()));
        assert_eq!(style.border.color, DANGER);
        let style = danger_button(&theme(), button::Status::Disabled);
        assert_eq!(style.text_color, TEXT_MUTED);
        assert_eq!(style.border.color, BORDER);
    }

    #[test]
    fn selected_button_is_an_accent_soft_pill() {
        for status in [
            button::Status::Active,
            button::Status::Hovered,
            button::Status::Pressed,
            button::Status::Disabled,
        ] {
            let style = selected_button(&theme(), status);
            assert_eq!(style.text_color, ACCENT_HOVER);
            assert_eq!(style.border.color, ACCENT);
            let expected = match status {
                button::Status::Hovered => color!(0xa855f7, 0.3),
                _ => ACCENT_SOFT,
            };
            assert_eq!(style.background, Some(expected.into()), "{status:?}");
        }
    }

    #[test]
    fn input_is_focused_with_the_accent_border() {
        let style = input(&theme(), text_input::Status::Active);
        assert_eq!(style.background, BACKGROUND.into());
        assert_eq!(style.border.color, BORDER);
        assert_eq!(style.placeholder, TEXT_MUTED);
        assert_eq!(style.value, TEXT);
        assert_eq!(style.selection, ACCENT_SOFT);
        let style = input(&theme(), text_input::Status::Hovered);
        assert_eq!(style.border.color, BORDER_FOCUS);
        let style = input(&theme(), text_input::Status::Focused { is_hovered: false });
        assert_eq!(style.border.color, ACCENT);
        let style = input(&theme(), text_input::Status::Focused { is_hovered: true });
        assert_eq!(style.border.color, ACCENT);
        let style = input(&theme(), text_input::Status::Disabled);
        assert_eq!(style.border.color, BORDER);
    }

    #[test]
    fn checkbox_is_accent_when_checked() {
        let style = checkbox_style(&theme(), checkbox::Status::Active { is_checked: true });
        assert_eq!(style.background, ACCENT.into());
        assert_eq!(style.icon_color, ACCENT_TEXT);
        assert_eq!(style.border.color, ACCENT);
        assert_eq!(style.text_color, Some(TEXT));
        let style = checkbox_style(&theme(), checkbox::Status::Active { is_checked: false });
        assert_eq!(style.background, BACKGROUND.into());
        assert_eq!(style.border.color, BORDER);
        let style = checkbox_style(&theme(), checkbox::Status::Hovered { is_checked: true });
        assert_eq!(style.background, ACCENT_HOVER.into());
        assert_eq!(style.border.color, ACCENT_HOVER);
        let style = checkbox_style(&theme(), checkbox::Status::Hovered { is_checked: false });
        assert_eq!(style.background, BACKGROUND.into());
        assert_eq!(style.border.color, BORDER_FOCUS);
        let style = checkbox_style(&theme(), checkbox::Status::Disabled { is_checked: true });
        assert_eq!(style.background, color!(0xa855f7, 0.4).into());
        let style = checkbox_style(&theme(), checkbox::Status::Disabled { is_checked: false });
        assert_eq!(style.background, BACKGROUND.into());
        assert_eq!(style.border.color, BORDER);
    }

    #[test]
    fn scrollable_has_a_thin_muted_scrollbar() {
        let style = scrollable_style(
            &theme(),
            scrollable::Status::Active {
                is_horizontal_scrollbar_disabled: true,
                is_vertical_scrollbar_disabled: false,
            },
        );
        assert_eq!(style.container.background, None);
        assert_eq!(style.vertical_rail.scroller.background, BORDER_FOCUS.into());
        assert_eq!(
            style.horizontal_rail.scroller.background,
            BORDER_FOCUS.into()
        );
        assert_eq!(style.gap, None);
        assert_eq!(style.auto_scroll.background, SURFACE.into());
        let style = scrollable_style(
            &theme(),
            scrollable::Status::Hovered {
                is_horizontal_scrollbar_hovered: false,
                is_vertical_scrollbar_hovered: true,
                is_horizontal_scrollbar_disabled: false,
                is_vertical_scrollbar_disabled: false,
            },
        );
        assert_eq!(style.vertical_rail.scroller.background, BORDER_FOCUS.into());
        let style = scrollable_style(
            &theme(),
            scrollable::Status::Dragged {
                is_horizontal_scrollbar_dragged: true,
                is_vertical_scrollbar_dragged: false,
                is_horizontal_scrollbar_disabled: false,
                is_vertical_scrollbar_disabled: true,
            },
        );
        assert_eq!(style.vertical_rail.scroller.background, BORDER_FOCUS.into());
    }

    #[test]
    fn key_chip_is_an_accent_soft_pill() {
        let style = key_chip(&theme());
        assert_eq!(style.background, Some(ACCENT_SOFT.into()));
        assert_eq!(style.text_color, Some(ACCENT_HOVER));
        assert_eq!(style.border.color, ACCENT);
        assert_eq!(style.border.radius, RADIUS.into());
    }

    #[test]
    fn status_pills() {
        let style = status_idle(&theme());
        assert_eq!(style.background, None);
        assert_eq!(style.text_color, Some(TEXT_MUTED));
        assert_eq!(style.border.color, Color::TRANSPARENT);
        let style = status_saved(&theme());
        assert_eq!(style.background, Some(BRAND_SOFT.into()));
        assert_eq!(style.text_color, Some(BRAND_HOVER));
        assert_eq!(style.border.color, BRAND);
        let style = status_error(&theme());
        assert_eq!(style.background, Some(color!(0xf87171, 0.15).into()));
        assert_eq!(style.text_color, Some(DANGER_HOVER));
        assert_eq!(style.border.color, DANGER);
    }

    #[test]
    fn rule_is_a_hairline() {
        let style = rule_style(&theme());
        assert_eq!(style.color, BORDER);
        assert_eq!(style.fill_mode, rule::FillMode::Full);
    }
}
