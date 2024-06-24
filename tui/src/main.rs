use std::{fs::File, io::stdout};
use crossterm::{execute, terminal::*, ExecutableCommand};
use tui::app::App;
use simplelog::*;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {

    // Setup logging
    let log_file = File::create("tui.log")?;
    WriteLogger::init(LevelFilter::Info, Config::default(), log_file)?;
    // console_subscriber::init();

    // init_panic_hooks()?;

    let mut app = App::new()?;
    
    // Take control of the terminal.
    log::info!("entering alternate screen mode.");
    execute!(stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    log::info!("entered alternate screen mode.");

    let r = app.run().await;
    if let Err(e) = r {
        log::error!("{:?}", e);
    }
    app.shutdown().await;

    // Return control of the terminal.
    log::info!("leaving alternate screen mode.");
    execute!(stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    log::info!("left alternate screen mode.");


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