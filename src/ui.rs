use anyhow::{Context, Result};
use colored::Colorize;
use dialoguer::{Input, Select, MultiSelect, Confirm};
use std::path::PathBuf;

use crate::models::Ticket;
use crate::config::AppConfig;

/// Display a list of tickets in a user-friendly format
pub fn display_tickets(tickets: &[Ticket]) {
    println!("\n{}", "=".repeat(80));
    println!("Found {} tickets", tickets.len());
    println!("{}", "=".repeat(80));
    
    for (i, ticket) in tickets.iter().enumerate() {
        let priority_str = match ticket.priority {
            3..=4 => "⚠️ High".red(),
            2 => "Medium".yellow(),
            _ => "Low".green(),
        };
        
        let estimate_str = match ticket.estimate {
            Some(est) => format!("{} points", est),
            None => "Not estimated".to_string(),
        };
        
        let labels_str = if ticket.labels.is_empty() {
            "None".to_string()
        } else {
            ticket.labels.join(", ")
        };
        
        println!("{}. [{}] {}", i + 1, ticket.state.blue(), ticket.title.bright_white());
        println!("   Priority: {} | Estimate: {} | Labels: {}", priority_str, estimate_str, labels_str);
        println!("   URL: {}", ticket.url);
        
        // Description is now hidden in the listing to simplify output
        
        println!("{}", "-".repeat(80));
    }
}

/// Get user selection of tickets to process
pub fn get_user_selection(tickets: &[Ticket], generate_plans: bool) -> Result<Vec<usize>> {
    // Different prompt based on whether we're generating plans or just fetching info
    let prompt = if generate_plans {
        "Select tickets to generate implementation plans for:".blue()
    } else {
        "Select tickets to analyze:".blue()
    };
    
    println!("\n{}", prompt);
    
    let selections = MultiSelect::new()
        .items(&tickets.iter().map(|t| &t.title).collect::<Vec<_>>())
        .defaults(&vec![false; tickets.len()])
        .interact()?;
    
    if selections.is_empty() {
        let confirm_message = if generate_plans {
            "No tickets selected. Do you want to generate plans for all tickets?"
        } else {
            "No tickets selected. Do you want to process all tickets?"
        };
        
        if Confirm::new()
            .with_prompt(confirm_message)
            .interact()?
        {
            return Ok((0..tickets.len()).collect());
        }
    }
    
    Ok(selections)
}

/// Run the setup wizard to configure API keys and settings
pub async fn setup_wizard() -> Result<AppConfig> {
    println!("\n{}", "📝 Linear Agent Setup".bright_green());
    println!("{}", "Let's set up your configuration.".blue());
    
    // Start with default config
    let mut config = AppConfig::default();
    
    // Ask for API keys
    config.linear_api_key = Input::new()
        .with_prompt("Linear API Key")
        .allow_empty(false)
        .interact_text()?;
    
    let anthropic_key: String = Input::new()
        .with_prompt("Anthropic API Key (leave empty to skip if not using plan generation)")
        .allow_empty(true)
        .interact_text()?;
        
    config.anthropic_api_key = if anthropic_key.trim().is_empty() {
        None
    } else {
        Some(anthropic_key)
    };
    
    // Ask for Linear settings
    config.linear_team_name = Input::new()
        .with_prompt("Linear Team Name")
        .default("Engineering".to_string())
        .interact_text()?;
    
    config.linear_agent_user = Input::new()
        .with_prompt("Linear User Name (whose tickets to analyze)")
        .allow_empty(false)
        .interact_text()?;
    
    // Ask for states
    let states_input: String = Input::new()
        .with_prompt("Linear States to analyze (comma-separated)")
        .default("Open,In Progress".to_string())
        .interact_text()?;
    
    config.linear_agent_states = states_input
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();
    
    // Ask for Anthropic model
    let models = vec![
        "claude-3-7-sonnet-20250219",
        "claude-3-5-sonnet-20240620",
        "claude-3-haiku-20240307",
        "claude-3-opus-20240229"
    ];
    
    let model_index = Select::new()
        .with_prompt("Select Anthropic Model")
        .default(0)
        .items(&models)
        .interact()?;
    
    config.anthropic_model = models[model_index].to_string();
    
    // Ask to save configuration
    if Confirm::new()
        .with_prompt("Save this configuration for future use?")
        .interact()?
    {
        let home = dirs::home_dir().context("Failed to find home directory")?;
        let default_config_dir = home.join(".linear-agent");
        
        // Convert PathBuf to String for interact_text
        let default_path_str = default_config_dir
            .join(".env")
            .to_string_lossy()
            .to_string();
        
        let config_path_str: String = Input::new()
            .with_prompt("Config file path")
            .default(default_path_str)
            .interact_text()?;
            
        // Convert back to PathBuf
        let config_path = PathBuf::from(config_path_str);
        
        config.save(Some(&config_path))?;
        println!("{}", format!("Configuration saved to {}", config_path.display()).green());
    }
    
    Ok(config)
}