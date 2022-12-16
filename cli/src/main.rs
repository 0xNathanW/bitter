use clap::Parser;
use anyhow::Result;
use crossterm::{execute, terminal::EnterAlternateScreen, terminal::LeaveAlternateScreen};
use tui::{
    backend::CrosstermBackend, 
    Terminal,
    widgets::{Block, Borders},
};

mod app;

// #[derive(Parser)]
// struct Args {
//     #[arg(short, long, help = "Path to torrent file")]
//     torrent: String,

//     #[arg(short, long, help = "Verbose output")]
//     verbose: bool,
// }

fn main() -> Result<()> {

    // let args = Args::parse();
    
    // Setup terminal
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;


    terminal.draw(|f| {
        let size = f.size();
        let block = Block::default()
            .title("Block")
            .borders(Borders::ALL);
        f.render_widget(block, size);
    })?;

    use std::{thread, time::Duration};
    thread::sleep(Duration::from_millis(5000));




    // Restore terminal
    crossterm::terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
