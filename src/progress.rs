use console::{Term, style};
use crossbeam_channel::Receiver;
use std::io;
use std::thread::{self, JoinHandle};

/// Progress events emitted by worker threads. Each worker owns a `slot`, so the
/// writer can keep one live line per worker.
pub enum Update {
    /// A worker slot started processing a repo.
    Start {
        slot: usize,
        label: &'static str,
        repo: String,
    },
    /// A worker slot finished its current repo, optionally with an error.
    Finish { slot: usize, error: Option<String> },
}

/// Spawn a writer that renders a summary line plus one live line per worker.
/// Failures are printed as permanent lines above the live block.
pub fn create_writer(
    receiver: Receiver<Update>,
    total: usize,
    workers: usize,
    action: &'static str,
) -> JoinHandle<io::Result<()>> {
    thread::spawn(move || -> io::Result<()> {
        let term = Term::stdout();
        term.hide_cursor()?;

        // Per-slot (label, repo) of the repo a worker is currently handling.
        let mut slots: Vec<Option<(&'static str, String)>> = vec![None; workers];
        let mut done = 0usize;
        let mut failed = 0usize;

        let mut height = draw(&term, action, total, done, &slots)?;

        for update in receiver {
            let mut failure = None;

            match update {
                Update::Start { slot, label, repo } => {
                    if let Some(s) = slots.get_mut(slot) {
                        *s = Some((label, repo));
                    }
                }
                Update::Finish { slot, error } => {
                    done += 1;
                    if let Some(err) = error {
                        failed += 1;
                        let repo = slots
                            .get(slot)
                            .and_then(|s| s.as_ref())
                            .map(|(_, repo)| repo.as_str())
                            .unwrap_or("");
                        failure = Some(format!(
                            "    {} {}: {}",
                            style("Failed").red().bold(),
                            repo,
                            err
                        ));
                    }
                    if let Some(s) = slots.get_mut(slot) {
                        *s = None;
                    }
                }
            }

            // Clear the live block so a permanent failure line lands above it.
            term.clear_last_lines(height)?;
            if let Some(line) = failure {
                term.write_line(&line)?;
            }
            height = draw(&term, action, total, done, &slots)?;
        }

        term.clear_last_lines(height)?;
        term.show_cursor()?;

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
        term.write_line(&summary)?;
        Ok(())
    })
}

/// Render the summary line plus one line per worker slot; returns the block height.
fn draw(
    term: &Term,
    action: &str,
    total: usize,
    done: usize,
    slots: &[Option<(&'static str, String)>],
) -> io::Result<usize> {
    term.write_line(
        format!(
            "{} [{}/{}] repositories",
            style(action).green().bold(),
            done,
            total
        )
        .as_str(),
    )?;

    let mut active = 0usize;
    for (label, repo) in slots.iter().flatten() {
        term.write_line(format!("   {} {}", style(label).green().bold(), repo).as_str())?;
        active += 1;
    }

    Ok(active + 1)
}
