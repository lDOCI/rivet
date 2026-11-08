//! Rivet UI and engine. The `rivet` binary picks a backend (desktop window or
//! framebuffer); `ui-snapshot` renders the same UI to PNG for review.

slint::include_modules!();

pub mod engine;
