//! What the editor and the overlay share: the font to show a text with.
//!
//! iced (cosmic-text underneath) consults its font fallback list only for
//! the characters its current font lacks, and that list tries the text fonts
//! before the colour emoji font. Many emoji exist as monochrome glyphs in
//! those text fonts (DejaVu Sans, Noto Sans Symbols 2), so they come out
//! grey; only the others (flags, skin tones, ZWJ sequences) in colour. Emacs
//! has no such problem because it gives every character presented as emoji
//! by default the emoji font outright (`charscript.el`, `fontset.el`); this
//! does the same, per text.

use iced::advanced::graphics::text::cosmic_text::fontdb;
use iced::advanced::graphics::text::font_system;
use iced::font::{Family, Font};
use std::sync::OnceLock;
use unicode_properties::emoji::EmojiStatus;
use unicode_properties::UnicodeEmoji;

/// Colour emoji fonts, in the order fontconfig prefers them for its `emoji`
/// family (60-generic.conf); the first installed one is used.
const EMOJI_FAMILIES: [&str; 9] = [
    "Noto Color Emoji",
    "Apple Color Emoji",
    "Segoe UI Emoji",
    "Twitter Color Emoji",
    "EmojiOne Mozilla",
    "Emoji Two",
    "JoyPixels",
    "Emoji One",
    "OpenMoji Color",
];

/// The font to show `text` with: the colour emoji font when `text` has
/// emoji and one is installed, the default font otherwise.
pub fn font_for(text: &str) -> Font {
    if has_emoji(text) {
        emoji_font().unwrap_or(Font::DEFAULT)
    } else {
        Font::DEFAULT
    }
}

/// Whether `text` has a character presented as emoji by default, or the
/// emoji presentation selector (U+FE0F) asking it of one presented as text
/// by default (❤️, 1️⃣). Digits, © or ↔ alone do not count, though emoji
/// fonts have glyphs for them.
fn has_emoji(text: &str) -> bool {
    text.chars().any(|c| {
        c == '\u{FE0F}'
            || matches!(
                c.emoji_status(),
                EmojiStatus::EmojiPresentation
                    | EmojiStatus::EmojiPresentationAndModifierBase
                    | EmojiStatus::EmojiPresentationAndEmojiComponent
                    | EmojiStatus::EmojiPresentationAndModifierAndEmojiComponent
            )
    })
}

/// The first family of [`EMOJI_FAMILIES`] that `installed` accepts.
fn emoji_family(mut installed: impl FnMut(&str) -> bool) -> Option<&'static str> {
    EMOJI_FAMILIES.into_iter().find(|family| installed(family))
}

/// The colour emoji font, looked up once in iced's font system.
fn emoji_font() -> Option<Font> {
    static FONT: OnceLock<Option<Font>> = OnceLock::new();
    *FONT.get_or_init(|| {
        let mut system = font_system().write().expect("iced's font system is usable");
        let db = system.raw().db();
        emoji_family(|family| {
            let query = fontdb::Query {
                families: &[fontdb::Family::Name(family)],
                ..fontdb::Query::default()
            };
            db.query(&query).is_some()
        })
        .map(|family| Font {
            family: Family::Name(family),
            ..Font::DEFAULT
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emoji_presentation_characters_count_as_emoji() {
        // One of each Emoji_Presentation status: a face, a hand (takes a
        // skin tone), a regional indicator and a skin tone.
        for text in ["😀", "👍", "🇫", "🏽", "é,😀", "👨‍👩‍👧", "🇫🇷"] {
            assert!(has_emoji(text), "{text}");
        }
        // Presented as text by default, until the selector asks otherwise.
        assert!(!has_emoji("❤"));
        assert!(has_emoji("❤️"));
        assert!(has_emoji("1️⃣"));
    }

    #[test]
    fn text_does_not_count_as_emoji() {
        for text in [
            "",
            "a",
            "é,è,ê,ë,€",
            "1",
            "#",
            "©",
            "®",
            "™",
            "↔",
            "ß",
            "中",
        ] {
            assert!(!has_emoji(text), "{text}");
        }
    }

    #[test]
    fn the_first_installed_family_is_used() {
        assert_eq!(
            emoji_family(|family| family == "JoyPixels" || family == "Twitter Color Emoji"),
            Some("Twitter Color Emoji")
        );
        assert_eq!(emoji_family(|_| false), None);
    }

    /// Needs a colour emoji font installed (Noto Color Emoji, say).
    #[test]
    fn emoji_get_the_emoji_font_and_text_the_default_one() {
        let emoji = font_for("😀");
        assert_ne!(emoji, Font::DEFAULT);
        assert!(matches!(emoji.family, Family::Name(name) if EMOJI_FAMILIES.contains(&name)));
        assert_eq!(font_for("é,è"), Font::DEFAULT);
    }
}
