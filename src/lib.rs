//! KeyGlow: keyboard-effects domain logic.
//!
//! KeyGlow knows *that* a key was pressed — never *what character* it produces. Events
//! carry a physical key code only. Nothing about typed text exists in this crate, which
//! is what makes it impossible to turn into a keylogger by accident.
//!
//! The OS-level hook (evdev / hidraw / Raw Input / CGEventTap) lives in the Orbit host
//! and is gated behind the explicit `keyboard_listen` grant. This crate only consumes
//! the abstract events the host feeds it.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KeyGlowError {
    #[error("rolling window must be at least 1 second, got {0}")]
    BadWindow(u64),
}

/// A physical key press. `code` is a platform key code (e.g. evdev code, virtual-key
/// code) — an identifier, not a character. Never carries text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyPress {
    pub code: u16,
    pub at_ms: u64,
}

/// Rolling statistics over a sliding time window.
#[derive(Debug, Clone)]
pub struct TypingStats {
    window_ms: u64,
    events: VecDeque<KeyPress>,
}

impl TypingStats {
    pub fn new(window_ms: u64) -> Result<Self, KeyGlowError> {
        if window_ms == 0 {
            return Err(KeyGlowError::BadWindow(0));
        }
        Ok(Self { window_ms, events: VecDeque::new() })
    }

    pub fn record(&mut self, code: u16, at_ms: u64) {
        self.events.push_back(KeyPress { code, at_ms });
        self.prune(at_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        while let Some(front) = self.events.front() {
            if now_ms.saturating_sub(front.at_ms) > self.window_ms {
                self.events.pop_front();
            } else {
                break;
            }
        }
    }

    /// Words per minute over the window (5 keypresses ≈ 1 word, rounded down).
    pub fn wpm(&self, now_ms: u64) -> u32 {
        let n = self.keys_in_window(now_ms) as u64;
        (n * 60_000 / (5 * self.window_ms)) as u32
    }

    /// Key presses inside the window at `now_ms`.
    pub fn keys_in_window(&self, now_ms: u64) -> usize {
        self.events
            .iter()
            .filter(|p| now_ms.saturating_sub(p.at_ms) <= self.window_ms)
            .count()
    }

    /// Per-key press counts inside the window, sorted by code.
    pub fn counts_by_code(&self, now_ms: u64) -> Vec<(u16, u32)> {
        let mut counts: std::collections::BTreeMap<u16, u32> = Default::default();
        for p in self.events.iter().filter(|p| now_ms.saturating_sub(p.at_ms) <= self.window_ms) {
            *counts.entry(p.code).or_default() += 1;
        }
        counts.into_iter().collect()
    }
}

/// Animation families KeyGlow can drive (executed by the platform renderer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationKind {
    /// Scale pulse on press (breathing key).
    Pulse,
    /// Expanding ring from the pressed key.
    Ripple,
    /// Sustained glow easing out.
    Glow,
    /// Discrete stepped color change.
    Step,
}

/// A per-application (or global) lighting profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyProfile {
    pub id: String,
    pub name: String,
    /// Hex accent, e.g. "#6c8cff". Validated by the caller (Orbit validates settings).
    pub accent: String,
    pub animation: AnimationKind,
    /// Peak brightness 0..=255.
    pub brightness: u8,
    /// Idle-dim after N ms without presses (0 = never dim).
    pub idle_dim_ms: u64,
    /// Optional per-app activation (desktop-file id / exe name). None = global default.
    pub activates_on: Option<String>,
}

/// Sound feedback configuration. Sound is opt-in per profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoundProfile {
    pub enabled: bool,
    /// Volume 0..=100.
    pub volume: u8,
    /// Pack id, e.g. "soft-click". The pack itself is not distributed by default.
    pub pack: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_window_rejected() {
        assert_eq!(TypingStats::new(0).unwrap_err(), KeyGlowError::BadWindow(0));
    }

    #[test]
    fn wpm_counts_presses_in_window() {
        let mut stats = TypingStats::new(60_000).unwrap();
        for i in 0..50 {
            stats.record(30, i * 1000); // 50 presses over 50s
        }
        // 50 keys / 5 = 10 words per minute in a 60s window
        assert_eq!(stats.wpm(50_000), 10);
    }

    #[test]
    fn old_events_expire_out_of_window() {
        let mut stats = TypingStats::new(1_000).unwrap();
        stats.record(30, 0);
        stats.record(48, 500);
        stats.record(30, 5_000); // window cutoff at 4000ms
        assert_eq!(stats.keys_in_window(5_000), 1);
    }

    #[test]
    fn counts_by_code_groups_presses() {
        let mut stats = TypingStats::new(10_000).unwrap();
        stats.record(30, 0);
        stats.record(30, 100);
        stats.record(48, 200);
        assert_eq!(stats.counts_by_code(200), vec![(30, 2), (48, 1)]);
    }

    #[test]
    fn key_press_serialization_carries_no_text() {
        let press = KeyPress { code: 30, at_ms: 1 };
        let json = serde_json::to_string(&press).unwrap();
        assert!(json.contains("\"code\":30"));
        assert!(!json.contains('a') || json.len() < 40, "unexpected payload: {json}");
    }

    #[test]
    fn animation_kind_roundtrip() {
        let kinds = [AnimationKind::Pulse, AnimationKind::Ripple, AnimationKind::Glow, AnimationKind::Step];
        for k in kinds {
            let s = serde_json::to_string(&k).unwrap();
            let back: AnimationKind = serde_json::from_str(&s).unwrap();
            assert_eq!(back, k);
        }
    }

    #[test]
    fn default_sound_is_opt_in() {
        let p = SoundProfile { enabled: false, volume: 30, pack: "soft-click".into() };
        assert!(!p.enabled);
    }
}
