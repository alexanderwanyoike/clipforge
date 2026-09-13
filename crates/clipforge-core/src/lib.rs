#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable
    )
)]
pub mod audio;
pub mod capture;
pub mod config;
pub mod doctor;
pub mod encode;
pub mod error;
pub mod export;
pub mod hotkeys;
pub mod library;
pub mod process;
pub mod recording;
pub mod replay;
