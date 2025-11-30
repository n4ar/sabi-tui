//! First-run onboarding flow

use crate::config::{Config, Provider};
use std::io::{self, Write};

pub fn run_onboarding() -> io::Result<Config> {
    println!("\n🚀 Welcome to Sabi-TUI!\n");
    println!("Let's set up your AI provider.\n");

    // Select provider
    println!("Select provider:");
    println!("  1) Gemini (Google AI)");
    println!("  2) OpenAI");
    println!("  3) OpenAI-compatible (Ollama, Groq, Together, etc.)");
    print!("\nChoice [1]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let choice = input.trim();

    let (provider, base_url, default_model): (Provider, Option<String>, String) = match choice {
        "2" => (Provider::OpenAI, None, "gpt-4o-mini".into()),
        "3" => {
            print!("Base URL (e.g., http://localhost:11434/v1): ");
            io::stdout().flush()?;
            input.clear();
            io::stdin().read_line(&mut input)?;
            let url = input.trim().to_string();

            print!("Model name: ");
            io::stdout().flush()?;
            input.clear();
            io::stdin().read_line(&mut input)?;
            let model = input.trim().to_string();

            (Provider::OpenAI, Some(url), model)
        }
        _ => (Provider::Gemini, None, "gemini-2.5-flash".into()),
    };

    // Get API key
    let api_key_prompt = match (&provider, &base_url) {
        (Provider::Gemini, _) => "Gemini API key (https://aistudio.google.com/apikey): ",
        (Provider::OpenAI, Some(_)) => "API key (leave empty if not required): ",
        (Provider::OpenAI, None) => "OpenAI API key: ",
    };

    print!("{}", api_key_prompt);
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;
    let api_key = input.trim().to_string();

    // Model selection for non-custom providers
    let model = if base_url.is_none() {
        print!("Model [{}]: ", default_model);
        io::stdout().flush()?;
        input.clear();
        io::stdin().read_line(&mut input)?;
        let m = input.trim();
        if m.is_empty() {
            default_model
        } else {
            m.to_string()
        }
    } else {
        default_model
    };

    let config = Config {
        provider,
        api_key,
        base_url,
        model,
        ..Config::default()
    };

    // Save config
    config
        .save()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    println!("\n✓ Configuration saved to ~/.sabi/config.toml");
    println!("  Run `sabi` to start!\n");

    Ok(config)
}
