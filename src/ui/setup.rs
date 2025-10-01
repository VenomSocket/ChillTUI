use std::io::{self, Write};
use crate::config::Config;
use crate::api::PutioClient;

pub fn run_setup_wizard() -> Result<Config, Box<dyn std::error::Error>> {
    let mut config = Config::load().unwrap_or_default();

    println!("═══════════════════════════════════════════════════════════");
    println!("            ChillTUI First-Time Setup");
    println!("═══════════════════════════════════════════════════════════\n");

    // Chill.institute API key
    if config.chill_api_key.is_none() {
        println!("Step 1: Chill.institute API Key");
        println!("Request an API key by emailing: chill-institute@proton.me");
        println!("Or via X: x.com/chill_institute");

        loop {
            print!("\nEnter your Chill API key: ");
            io::stdout().flush()?;

            let mut api_key = String::new();
            io::stdin().read_line(&mut api_key)?;
            let api_key = api_key.trim();

            if api_key.is_empty() {
                println!("✗ API key cannot be empty. Please try again.");
                continue;
            }

            if api_key.len() < 10 {
                println!("✗ API key seems too short. Please check and try again.");
                continue;
            }

            config.chill_api_key = Some(api_key.to_string());
            println!("✓ API key accepted");
            println!();
            break;
        }
    }

    // Put.io OAuth setup
    if config.putio_oauth_token.is_none() {
        println!("Step 2: Put.io Authentication");
        println!("1. Go to: https://app.put.io/oauth");
        println!("2. Click 'Create App' and fill in:");
        println!("   - Name: ChillTUI (or any name)");
        println!("   - Description: Personal torrent client");
        println!("   - Website: http://localhost");
        println!("   - Callback URL: http://localhost");
        println!("3. After saving, click the key icon (🔑) next to your app");
        println!("4. Copy the OAuth Token");

        loop {
            print!("\nEnter your Put.io OAuth token: ");
            io::stdout().flush()?;

            let mut token = String::new();
            io::stdin().read_line(&mut token)?;
            let token = token.trim();

            if token.is_empty() {
                println!("✗ OAuth token cannot be empty. Please try again.");
                continue;
            }

            if token.len() < 20 {
                println!("✗ OAuth token seems too short. Please check and try again.");
                continue;
            }

            // Test connection
            let client = PutioClient::new(token.to_string());
            match client.test_connection() {
                Ok(username) => {
                    config.putio_oauth_token = Some(token.to_string());
                    println!("✓ Connected as: {}\n", username);
                    break;
                }
                Err(e) => {
                    println!("✗ Failed to connect to Put.io: {}", e);
                    println!("  Please check your token and try again.");
                    continue;
                }
            }
        }
    }

    // Folder setup
    if config.putio_folder_id.is_none() {
        println!("Step 3: Put.io Folder Setup");
        println!("Choose where to save downloads on Put.io:");
        println!("  1. Use default folder: /ChillTUI/");
        println!("  2. Create custom folder");
        print!("\nChoice [1]: ");
        io::stdout().flush()?;

        let mut choice = String::new();
        io::stdin().read_line(&mut choice)?;
        let choice = choice.trim();

        if choice == "2" {
            print!("Enter folder name: ");
            io::stdout().flush()?;
            let mut folder_name = String::new();
            io::stdin().read_line(&mut folder_name)?;
            config.putio_folder_name = folder_name.trim().to_string();
        } else {
            config.putio_folder_name = "ChillTUI".to_string();
        }

        // Create folder
        let client = PutioClient::new(config.putio_oauth_token.as_ref().unwrap().clone());
        let folder_id = client.find_or_create_folder(&config.putio_folder_name)?;
        config.putio_folder_id = Some(folder_id);
        println!("✓ Folder created: /{}/\n", config.putio_folder_name);
    }

    // Save config
    config.save()?;
    println!("═══════════════════════════════════════════════════════════");
    println!("✓ Setup complete! Configuration saved.");
    println!("═══════════════════════════════════════════════════════════\n");

    Ok(config)
}