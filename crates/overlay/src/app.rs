//! Overlay state and view: the variants of the held key, with the current
//! one highlighted.

use accents_rapides_core::OverlayMessage;
use iced::widget::{container, row, text, Row};
use iced::{border, Color, Element, Length, Theme};
use iced_layershell::to_layer_message;

/// Colour of the variant that the next leader press commits.
pub const ACTIVE_COLOR: Color = Color::from_rgb(1.0, 0.8, 0.2);

/// What the overlay shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overlay {
    /// Variants of the held key, in cycle order.
    pub variants: Vec<String>,
    /// Index of the current variant in `variants`.
    pub current: Option<usize>,
}

impl Overlay {
    /// Show what the input method says.
    pub fn apply(&mut self, message: OverlayMessage) {
        *self = match message {
            OverlayMessage::Show { variants, index } => Self {
                variants,
                current: Some(index),
            },
            OverlayMessage::Hide => Self::default(),
        };
    }

    /// Each variant with whether it is the current one.
    pub fn cells(&self) -> impl Iterator<Item = (&str, bool)> {
        self.variants
            .iter()
            .enumerate()
            .map(|(i, variant)| (variant.as_str(), Some(i) == self.current))
    }
}

/// Messages of the overlay; the layer-shell actions are added by the macro.
#[to_layer_message]
#[derive(Debug, Clone)]
pub enum Message {
    /// From the input method, over the overlay socket.
    Feed(OverlayMessage),
}

/// The overlay: a rounded panel with one glyph per variant, or nothing when
/// hidden.
pub fn view(overlay: &Overlay) -> Element<'_, Message> {
    if overlay.variants.is_empty() {
        return Row::new().into();
    }
    let cells = overlay.cells().map(|(variant, active)| {
        let glyph = text(variant).size(32);
        if active {
            glyph.color(ACTIVE_COLOR).into()
        } else {
            glyph.into()
        }
    });
    container(
        container(row(cells).spacing(10))
            .padding([10, 15])
            .style(panel_style),
    )
    .center_x(Length::Fill)
    .into()
}

/// Translucent dark panel with rounded corners.
pub fn panel_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.7).into()),
        border: border::rounded(8),
        text_color: Some(Color::WHITE),
        ..container::Style::default()
    }
}
