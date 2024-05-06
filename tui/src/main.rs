use std::io::stdout;
use crossterm::{execute, terminal::*, ExecutableCommand};
use tui::app::App;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    init_panic_hooks()?;

    let mut app = App::new()?;
    
    // Take control of the terminal.
    execute!(stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    
    match app.run().await {
        Ok(_) => {}
        Err(e) => eprint!("{}", e),
    }

    // Return control of the terminal.
    execute!(stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;

    Ok(())
}

// This breaks the terminal after loading torrent for now.
fn init_panic_hooks() -> color_eyre::Result<()> {

    let hook_builder = color_eyre::config::HookBuilder::default();
    let (panic_hook, eyre_hook) = hook_builder.into_hooks();

    let panic_hook = panic_hook.into_panic_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        stdout().execute(LeaveAlternateScreen).unwrap();
        disable_raw_mode().unwrap();
        println!("{:?}", panic_info);
        panic_hook(panic_info);
    }));

    let eyre_hook = eyre_hook.into_eyre_hook();
    color_eyre::eyre::set_hook(Box::new(move |error| {
        stdout().execute(LeaveAlternateScreen).unwrap();
        disable_raw_mode().unwrap();
        println!("{:?}", error);
        eyre_hook(error)
    }))?;

    Ok(())
}