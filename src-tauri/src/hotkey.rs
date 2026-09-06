//! Parsing and validation for the user-configurable global shortcut.

use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

use crate::error::{Error, Result};

/// Parses a Clipdeck shortcut and rejects combinations that would capture
/// ordinary typing globally. OS-level conflicts are detected when registration
/// is attempted by the global-shortcut plugin.
pub fn parse(combo: &str) -> Result<Shortcut> {
    let parts: Vec<&str> = combo.split('+').map(str::trim).collect();
    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        return Err(Error::Other(
            "global shortcut contains an empty key segment".into(),
        ));
    }
    let mut modifiers = Modifiers::empty();
    let mut code = None;

    for part in parts {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => insert_modifier(&mut modifiers, Modifiers::CONTROL, part)?,
            "shift" => insert_modifier(&mut modifiers, Modifiers::SHIFT, part)?,
            "alt" => insert_modifier(&mut modifiers, Modifiers::ALT, part)?,
            "super" | "win" | "meta" | "cmd" | "command" => {
                insert_modifier(&mut modifiers, Modifiers::SUPER, part)?
            }
            key => {
                if code.is_some() {
                    return Err(Error::Other(
                        "global shortcut must contain exactly one non-modifier key".into(),
                    ));
                }
                code =
                    Some(key_name_to_code(key).ok_or_else(|| {
                        Error::Other(format!("unsupported shortcut key: {part}"))
                    })?);
            }
        }
    }

    if !modifiers.intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER) {
        return Err(Error::Other(
            "global shortcut must include Ctrl, Alt, or Command/Win".into(),
        ));
    }
    let code =
        code.ok_or_else(|| Error::Other("global shortcut must include a non-modifier key".into()))?;
    Ok(Shortcut::new(Some(modifiers), code))
}

fn insert_modifier(modifiers: &mut Modifiers, value: Modifiers, token: &str) -> Result<()> {
    if modifiers.contains(value) {
        return Err(Error::Other(format!(
            "duplicate shortcut modifier: {token}"
        )));
    }
    modifiers.insert(value);
    Ok(())
}

fn key_name_to_code(name: &str) -> Option<Code> {
    use Code::*;
    let normalized = name.to_ascii_uppercase();
    Some(match normalized.as_str() {
        "A" => KeyA,
        "B" => KeyB,
        "C" => KeyC,
        "D" => KeyD,
        "E" => KeyE,
        "F" => KeyF,
        "G" => KeyG,
        "H" => KeyH,
        "I" => KeyI,
        "J" => KeyJ,
        "K" => KeyK,
        "L" => KeyL,
        "M" => KeyM,
        "N" => KeyN,
        "O" => KeyO,
        "P" => KeyP,
        "Q" => KeyQ,
        "R" => KeyR,
        "S" => KeyS,
        "T" => KeyT,
        "U" => KeyU,
        "V" => KeyV,
        "W" => KeyW,
        "X" => KeyX,
        "Y" => KeyY,
        "Z" => KeyZ,
        "0" => Digit0,
        "1" => Digit1,
        "2" => Digit2,
        "3" => Digit3,
        "4" => Digit4,
        "5" => Digit5,
        "6" => Digit6,
        "7" => Digit7,
        "8" => Digit8,
        "9" => Digit9,
        "F1" => F1,
        "F2" => F2,
        "F3" => F3,
        "F4" => F4,
        "F5" => F5,
        "F6" => F6,
        "F7" => F7,
        "F8" => F8,
        "F9" => F9,
        "F10" => F10,
        "F11" => F11,
        "F12" => F12,
        "SPACE" => Space,
        "ENTER" => Enter,
        "TAB" => Tab,
        "ESC" | "ESCAPE" => Escape,
        "INSERT" => Insert,
        "DELETE" | "DEL" => Delete,
        "HOME" => Home,
        "END" => End,
        "PAGEUP" | "PGUP" => PageUp,
        "PAGEDOWN" | "PGDN" => PageDown,
        "LEFT" | "ARROWLEFT" => ArrowLeft,
        "RIGHT" | "ARROWRIGHT" => ArrowRight,
        "UP" | "ARROWUP" => ArrowUp,
        "DOWN" | "ARROWDOWN" => ArrowDown,
        "BACKSLASH" => Backslash,
        "SLASH" => Slash,
        "COMMA" => Comma,
        "PERIOD" => Period,
        "SEMICOLON" => Semicolon,
        "QUOTE" => Quote,
        "BACKQUOTE" | "`" => Backquote,
        "MINUS" | "-" => Minus,
        "EQUALS" | "=" => Equal,
        "[" | "BRACKETLEFT" => BracketLeft,
        "]" | "BRACKETRIGHT" => BracketRight,
        _ => return None,
    })
}

/// The two independently registered global actions.
///
/// `AppState` used to hold a single `active_hotkey`, which made it impossible
/// to bind the quick palette and the full application window at the same time
/// without one silently overwriting the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HotkeyAction {
    /// Toggles the frameless quick clipboard palette.
    QuickPalette,
    /// Opens the decorated full application window.
    FullWindow,
}

impl HotkeyAction {
    /// Human-readable label used in validation errors shown in Settings.
    pub fn label(self) -> &'static str {
        match self {
            HotkeyAction::QuickPalette => "Quick clipboard",
            HotkeyAction::FullWindow => "Open full Clipmo",
        }
    }
}

/// Rejects a save where both actions would be bound to the same accelerator.
///
/// Two identical registrations cannot both win, so the second would silently
/// steal the first action's shortcut. Failing loudly keeps Settings truthful.
pub fn validate_distinct(quick: &str, full: &str) -> Result<(Shortcut, Shortcut)> {
    let quick_shortcut = parse(quick).map_err(|error| {
        Error::Other(format!("{}: {error}", HotkeyAction::QuickPalette.label()))
    })?;
    let full_shortcut = parse(full)
        .map_err(|error| Error::Other(format!("{}: {error}", HotkeyAction::FullWindow.label())))?;
    if quick_shortcut == full_shortcut {
        return Err(Error::Other(format!(
            "\u{201c}{}\u{201d} and \u{201c}{}\u{201d} cannot use the same shortcut",
            HotkeyAction::QuickPalette.label(),
            HotkeyAction::FullWindow.label()
        )));
    }
    Ok((quick_shortcut, full_shortcut))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_supported_shortcuts_with_a_primary_modifier() {
        assert_eq!(
            parse("Cmd+Shift+V").unwrap(),
            parse("Super+Shift+V").unwrap()
        );
        let defaults = crate::models::Settings::default();
        assert!(validate_distinct(&defaults.hotkey, &defaults.full_window_hotkey).is_ok());
        let shortcut = parse("Ctrl+Shift+V").unwrap();
        assert!(shortcut.mods.contains(Modifiers::CONTROL));
        assert!(shortcut.mods.contains(Modifiers::SHIFT));
        assert_eq!(shortcut.key, Code::KeyV);

        assert_eq!(parse("Ctrl+ArrowLeft").unwrap().key, Code::ArrowLeft);
        assert_eq!(parse("Ctrl+BracketLeft").unwrap().key, Code::BracketLeft);
    }

    #[test]
    fn rejects_shortcuts_that_would_capture_normal_typing() {
        assert!(parse("V").is_err());
        assert!(parse("Shift+V").is_err());
    }

    #[test]
    fn rejects_unsupported_duplicate_and_multi_key_shortcuts() {
        assert!(parse("Ctrl+VolumeUp").is_err());
        assert!(parse("Ctrl+Ctrl+V").is_err());
        assert!(parse("Ctrl+V+C").is_err());
        assert!(parse("Ctrl++V").is_err());
        assert!(parse("Ctrl+V+").is_err());
    }

    #[test]
    fn rejects_two_actions_bound_to_the_same_accelerator() {
        let error = validate_distinct("Ctrl+Shift+V", "Ctrl+Shift+V").unwrap_err();
        assert!(error.to_string().contains("cannot use the same shortcut"));
    }

    #[test]
    fn accepts_the_shipped_defaults_for_both_actions() {
        let (quick, full) = validate_distinct("Ctrl+Shift+V", "Ctrl+Alt+Shift+V").unwrap();
        assert_ne!(quick, full);
        assert_eq!(quick.key, Code::KeyV);
        assert_eq!(full.key, Code::KeyV);
        assert!(full.mods.contains(Modifiers::ALT));
        assert!(!quick.mods.contains(Modifiers::ALT));
    }

    #[test]
    fn names_the_offending_action_when_one_shortcut_is_invalid() {
        let error = validate_distinct("Ctrl+Shift+V", "Shift+V").unwrap_err();
        assert!(error.to_string().contains("Open full Clipmo"));
    }
}
