use console::{Term, style};

use crossbeam_channel::Receiver;
use std::io;
use std::thread;
use std::thread::JoinHandle;

/// A single repository's result, reported once its work has finished.
pub struct Update {
    pub repo: String,
    pub error: Option<String>,
}

pub fn create_writer(
    receiver: Receiver<Update>,
    total: usize,
    action: &'static str,
) -> io::Result<JoinHandle<io::Result<()>>> {
    Ok(thread::spawn(move || -> Result<(), std::io::Error> {
        let term = Term::stdout();
        term.write_line(
            format!(
                "   {} {} repositories",
                style(action).green().bold(),
                total
            )
            .as_str(),
        )?;

        term.hide_cursor()?;

        let mut done = 0usize;
        let mut failed = 0usize;

        for update in receiver {
            done += 1;

            // Failures become permanent lines above the live progress line.
            if let Some(err) = &update.error {
                failed += 1;
                term.clear_line()?;
                term.write_line(
                    format!(
                        "    {} {}: {}",
                        style("Failed").red().bold(),
                        update.repo,
                        err
                    )
                    .as_str(),
                )?;
            }

            term.clear_line()?;
            term.write_line(
                format!(
                    "   {} [{}/{}] {}",
                    style(action).green().bold(),
                    done,
                    total,
                    update.repo
                )
                .as_str(),
            )?;
            term.move_cursor_up(1)?;
        }

        term.show_cursor()?;
        term.clear_line()?;

        let summary = if failed == 0 {
            format!(
                "    {} {} repositories",
                style("Finished").green().bold(),
                total
            )
        } else {
            format!(
                "    {} {}/{} repositories ({} failed)",
                style("Finished").yellow().bold(),
                done - failed,
                total,
                failed
            )
        };
        term.write_line(summary.as_str())?;
        Ok(())
    }))
}
