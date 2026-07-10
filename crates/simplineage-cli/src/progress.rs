//! Progress indicators for long-running CLI operations.

use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

/// Create a spinner for indeterminate work.
#[must_use]
pub fn spinner(message: impl Into<String>) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(message.into());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

/// Create a determinate progress bar (e.g. multi-file import).
#[must_use]
pub fn bar(len: u64, message: impl Into<String>) -> ProgressBar {
    let pb = ProgressBar::new(len.max(1));
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.cyan} {msg} [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("█▉▊▋▌▍▎▏  "),
    );
    pb.set_message(message.into());
    pb
}

/// Finish a spinner/bar with a success message (or clear when quiet).
pub fn finish_ok(pb: ProgressBar, msg: impl Into<String>, quiet: bool) {
    if quiet {
        pb.finish_and_clear();
    } else {
        pb.finish_with_message(msg.into());
    }
}

/// Finish and clear without a trailing line.
pub fn finish_clear(pb: ProgressBar) {
    pb.finish_and_clear();
}
