use serde::{Deserialize, Serialize};

/// Hotkey action types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HotkeyAction {
    ToggleRecording,
    SaveReplay,
    ToggleReplayBuffer,
    MarkHighlight,
}

impl HotkeyAction {
    pub fn all() -> &'static [HotkeyAction] {
        &[
            HotkeyAction::ToggleRecording,
            HotkeyAction::SaveReplay,
            HotkeyAction::ToggleReplayBuffer,
            HotkeyAction::MarkHighlight,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            HotkeyAction::ToggleRecording => "Toggle Recording",
            HotkeyAction::SaveReplay => "Save Replay",
            HotkeyAction::ToggleReplayBuffer => "Toggle Replay Buffer",
            HotkeyAction::MarkHighlight => "Mark Highlight",
        }
    }
}
