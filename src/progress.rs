use console::{Term, style};

use std::io;
use std::sync::mpsc::Receiver;
use std::thread;
use std::thread::JoinHandle;

pub fn create_writer(
    receiver: Receiver<String>,
    max_value: usize,
) -> io::Result<JoinHandle<io::Result<()>>> {
    Ok(thread::spawn(move || -> Result<(), std::io::Error> {
        let term = Term::stdout();
        term.write_line(
            format!(
                "   {} {} repositories",
                style("Synchronising").green().bold(),
                max_value
            )
            .as_str(),
        )?;

        term.hide_cursor()?;

        for message in receiver {
            term.clear_line()?;
            term.write_line(
                format!(
                    "    {} repository {}",
                    style("Synchronising").green().bold(),
                    message
                )
                .as_str(),
            )?;
            term.move_cursor_up(1)?;
        }

        term.show_cursor()?;
        term.clear_line()?;
        term.write_line(
            format!("    {} synchronising", style("Finished").green().bold()).as_str(),
        )?;
        Ok(())
    }))
}
