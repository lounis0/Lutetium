#[derive(Debug)]
pub struct BackendError(pub String);
impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for BackendError {}

pub mod gdi {
    //! Initial native implementation. It consumes public backend-neutral
    //! frames produced by the core renderer.
}
pub mod native_window {
    //! Native window/event-loop implementation, isolated from widgets.
}
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::run;
#[cfg(not(windows))]
pub fn run(_: crate::Application) -> Result<(), BackendError> {
    Err(BackendError(
        "ION's first native backend currently supports Windows only".into(),
    ))
}
