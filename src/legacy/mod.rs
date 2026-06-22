//! Legacy modules — kept for reference, not compiled by default.
//! Enable with: `cargo build --features legacy`

#[cfg(feature = "legacy")]
pub mod bot_detector;
#[cfg(feature = "legacy")]
pub mod ca;
#[cfg(feature = "legacy")]
pub mod proxy;
#[cfg(feature = "legacy")]
pub mod webview_login;
