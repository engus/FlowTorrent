//! Release-only resource probe; see docs/performance.md.
#[path = "resources/mod.rs"]
mod resources;
fn main() -> anyhow::Result<()> {
    resources::run(std::env::args_os().skip(1).collect())
}
