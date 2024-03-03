use std::io::stdout;
use crossterm::{execute, terminal::*};


#[tokio::main]
async fn main() -> anyhow::Result<()> {

    let mut app = tui::app::App::new()?;
    
    execute!(stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    
    app.run().await?;

    execute!(stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;

    Ok(())
}