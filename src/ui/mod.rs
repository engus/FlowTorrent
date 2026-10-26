pub mod i18n;
pub mod models;
pub mod components {
    slint::include_modules!();
}
mod about;
mod controller;
pub use controller::UiController;

pub mod view_state;
