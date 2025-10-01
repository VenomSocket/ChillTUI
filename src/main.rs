mod api;
mod config;
mod models;
mod ui;

use config::Config;
use ui::setup::run_setup_wizard;
use ui::App;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let debug = args.contains(&"--debug".to_string()) || args.contains(&"--logging".to_string());

    if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        print_help();
        return Ok(());
    }

    if args.contains(&"--version".to_string()) || args.contains(&"-v".to_string()) {
        println!("chilltui v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Load or create config
    let mut config = Config::load().unwrap_or_else(|e| {
        if debug {
            eprintln!("[DEBUG] Could not load config: {}", e);
        }
        Config::default()
    });

    // Check if setup is needed
    if config.needs_setup() || args.contains(&"--setup".to_string()) {
        config = run_setup_wizard()?;
    }

    if debug {
        eprintln!("[DEBUG] Starting ChillTUI");
        eprintln!("[DEBUG] Config: {:?}", config);
    }

    // Run app
    let mut app = App::new(config, debug);
    app.run()?;

    Ok(())
}

fn print_help() {
    println!("chilltui v{}", env!("CARGO_PKG_VERSION"));
    println!("Fast terminal UI for torrent search via chill.institute and Put.io integration\n");
    println!("USAGE:");
    println!("    chilltui [OPTIONS]\n");
    println!("OPTIONS:");
    println!("    -h, --help       Print help information");
    println!("    -v, --version    Print version information");
    println!("    --setup          Run setup wizard");
    println!("    --debug          Enable debug logging to stderr");
    println!("    --logging        Same as --debug\n");
    println!("CONTROLS:");
    println!("    Type            Search torrents");
    println!("    Enter           Execute search / Send to Put.io");
    println!("    ↑↓              Navigate results");
    println!("    Space           Select/deselect result");
    println!("    Tab             Switch focus (search/results)");
    println!("    ESC             Clear search");
    println!("    ESC×2           Quit application\n");
    println!("FIRST RUN:");
    println!("    Run without arguments to start setup wizard");
    println!("    You'll need:");
    println!("      - Chill.institute API key (email: chill-institute@proton.me or x.com/chill_institute)");
    println!("      - Put.io account for OAuth authentication\n");
    println!("CONFIG:");
    println!("    Config stored at: ~/.config/chilltui/config.json");
}
