pub mod i18n;
pub mod models;
pub mod components {
    slint::include_modules!();
}
mod controller;
pub use controller::UiController;
