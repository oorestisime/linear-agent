use anyhow::{Context, Result};
use clap::Parser;
use colored::Colorize;
use dotenv::dotenv;
use std::path::PathBuf;
use std::fs;
use crate::models::Ticket;

mod config;
mod linear;
mod anthropic;
mod models;
mod ui;

/// Linear Agent - Implementation Plan Generator
/// 
/// A CLI tool that fetches tickets from Linear, enriches them with detailed information,
/// and uses Claude to generate comprehensive implementation plans.
#[derive(Parser, Debug)]
#[clap(
    author = "Bold Inc.", 
    version, 
    about,
    after_help = "Example usage:\n  linear-agent --setup                       # Run initial setup\n  linear-agent --user \"John Doe\"              # Get John's tickets (no plans)\n  linear-agent --user \"John Doe\" --plan      # Generate plans for John's tickets\n  linear-agent -u \"John Doe\" -s \"Open\"        # Only analyze open tickets\n  linear-agent -e ~/.linear-agent/.env       # Use custom .env file\n  linear-agent --ticket path/to/ticket.md --plan # Generate plan from saved ticket file\n  linear-agent --ticket-id ABC-123            # Fetch and save a specific ticket by ID"
)]
struct Args {
    /// Path to .env file containing Linear and Anthropic API keys
    /// 
    /// The .env file should contain the following variables:
    /// LINEAR_API_KEY, ANTHROPIC_API_KEY, LINEAR_TEAM_NAME,
    /// LINEAR_AGENT_USER, LINEAR_AGENT_STATES, ANTHROPIC_MODEL
    #[clap(short, long)]
    env: Option<PathBuf>,

    /// Linear user to analyze tickets for (e.g. "Jane Smith")
    #[clap(short, long)]
    user: Option<String>,

    /// Linear team name (defaults to "Engineering" if not specified)
    #[clap(short, long)]
    team: Option<String>,

    /// Comma-separated list of ticket states to analyze
    /// 
    /// Example: "Open,In Progress,Blocked"
    #[clap(short, long)]
    states: Option<String>,

    /// Anthropic model to use for implementation plan generation
    /// 
    /// Supported models: "claude-3-7-sonnet-20250219", "claude-3-5-sonnet-20240620",
    /// "claude-3-haiku-20240307", "claude-3-opus-20240229"
    #[clap(short, long)]
    model: Option<String>,

    /// Run setup wizard to configure credentials and preferences
    /// 
    /// This will guide you through setting up Linear and Anthropic API keys,
    /// as well as default settings for the tool.
    #[clap(long)]
    setup: bool,

    /// Output directory for implementation plans
    /// 
    /// The implementation plans will be saved as Markdown files in this directory.
    #[clap(short, long, default_value = "implementation_plans")]
    output: PathBuf,

    /// Directory for saving ticket information
    /// 
    /// Ticket information will be saved as Markdown files in this directory.
    #[clap(long, default_value = "tickets")]
    tickets_dir: PathBuf,

    /// Path to a previously saved ticket markdown file to process
    /// 
    /// Use this to generate an implementation plan from a ticket file
    /// that was previously saved using this tool.
    #[clap(long)]
    ticket: Option<PathBuf>,

    /// Linear ticket ID to fetch and save
    /// 
    /// Fetches a specific ticket from Linear by ID and saves it as Markdown.
    /// The file will be saved in the tickets directory with the format: ticketId-title.md
    #[clap(long)]
    ticket_id: Option<String>,

    // We've removed the non-interactive mode to avoid accidental high costs
    
    /// Generate implementation plans for tickets
    /// 
    /// If not provided, the tool will only fetch and display ticket information
    /// without generating implementation plans.
    #[clap(long)]
    plan: bool,
    
    /// Enable verbose output with debug information
    /// 
    /// Shows additional details like API responses and debug messages.
    #[clap(long)]
    verbose: bool,
    
    /// Check for updates
    /// 
    /// Checks if a new version of linear-agent is available.
    #[clap(long)]
    check_update: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();
    
    // Check for updates if requested
    if args.check_update {
        check_for_updates().await?;
        return Ok(());
    }

    // Print welcome message
    println!("{}", "🔍 Linear Agent: Interactive Implementation Plan Generator".bright_green());

    // If using --ticket option, we'll process a local ticket file
    if let Some(ticket_path) = &args.ticket {
        if !args.plan {
            println!("{}", "Note: Using --ticket without --plan will only display the ticket details".yellow());
        }
        
        if !ticket_path.exists() {
            println!("{}", format!("❌ Error: Ticket file not found: {}", ticket_path.display()).red());
            return Ok(());
        }
        
        // Load the ticket from the markdown file
        println!("{}", format!("Loading ticket from {}", ticket_path.display()).blue());
        let ticket_content = fs::read_to_string(ticket_path)
            .context(format!("Failed to read ticket file: {}", ticket_path.display()))?;
        
        let ticket = models::Ticket::from_markdown(&ticket_content)
            .context("Failed to parse ticket from markdown file")?;
        
        println!("{}", "Ticket loaded successfully:".green());
        println!("Title: {}", ticket.title);
        println!("ID: {}", ticket.id);
        println!("State: {}", ticket.state);
        
        // If --plan flag is provided, generate an implementation plan
        if args.plan {
            // Load environment variables for Anthropic API
            if let Some(env_path) = &args.env {
                dotenv::from_path(env_path).context("Failed to load .env file")?;
            } else {
                // Try to load from default locations
                let env_locations = config::AppConfig::get_env_locations();
                let mut loaded = false;
                
                for location in env_locations {
                    if location.exists() {
                        dotenv::from_path(&location).context(format!("Failed to load .env from {}", location.display()))?;
                        println!("Loaded configuration from {}", location.display());
                        loaded = true;
                        break;
                    }
                }
                
                if !loaded {
                    // If no .env file found, try loading from default location just in case
                    dotenv().ok();
                }
            }
            
            // Load configuration
            let app_config = config::AppConfig::load(None, &args).await?;
            
            // Test Anthropic API connection
            println!("\n{}", "Testing Anthropic API connection...".blue());
            let anthropic_client = anthropic::AnthropicClient::new(&app_config.anthropic_api_key);
            
            let anthropic_test = anthropic_client.test_connection().await;
            if anthropic_test.is_err() {
                println!("\n{}", "❌ Error: Anthropic API connection failed. Please check your API key and try again.".red());
                return Ok(());
            }
            
            println!("\n{}", "✅ Anthropic API connection successful".green());
            
            // Create output directory
            std::fs::create_dir_all(&args.output).context("Failed to create output directory")?;
            
            // Generate implementation plan
            println!("\n{}", format!("Generating implementation plan for: {}", ticket.title).blue());
            
            let implementation_plan = anthropic_client
                .generate_implementation_plan(&ticket, &app_config.anthropic_model)
                .await?;
            
            // Create safe filename with format ticketId-title.md
            let safe_title = ticket.title.chars()
                .map(|c| if c.is_alphanumeric() { c } else { '_' })
                .collect::<String>();
                
            let filename = format!("{}-{}.md", ticket.id, &safe_title[..std::cmp::min(50, safe_title.len())]);
            let file_path = args.output.join(filename);
            
            // Create the file content with implementation plan
            let file_content = format!(
                "# Implementation Plan: {}\n\n\
                 **Ticket ID:** {}\n\
                 **State:** {}\n\
                 **Priority:** {}\n\
                 **Estimate:** {}\n\
                 **URL:** {}\n\n\
                 ---\n\n\
                 {}",
                ticket.title,
                ticket.id,
                ticket.state,
                ticket.priority,
                ticket.estimate.map_or("Not estimated".to_string(), |e| e.to_string()),
                ticket.url,
                implementation_plan
            );
            
            // Write to file
            std::fs::write(&file_path, file_content).context("Failed to write implementation plan to file")?;
            
            // Get absolute path
            let abs_path = std::fs::canonicalize(&file_path)
                .unwrap_or_else(|_| file_path.clone());
            println!("{}", format!("✅ Implementation plan saved to {}", abs_path.display()).green());
        }
        
        return Ok(());
    }
    
    // If using --ticket-id option, we'll fetch and save that specific ticket
    if let Some(ticket_id) = &args.ticket_id {
        // Load environment variables
        if let Some(env_path) = &args.env {
            dotenv::from_path(env_path).context("Failed to load .env file")?;
        } else {
            // Try to load from default locations
            let env_locations = config::AppConfig::get_env_locations();
            let mut loaded = false;
            
            for location in env_locations {
                if location.exists() {
                    dotenv::from_path(&location).context(format!("Failed to load .env from {}", location.display()))?;
                    println!("Loaded configuration from {}", location.display());
                    loaded = true;
                    break;
                }
            }
            
            if !loaded {
                // If no .env file found, try loading from default location just in case
                dotenv().ok();
            }
        }
        
        // Load configuration
        let app_config = config::AppConfig::load(None, &args).await?;
        
        // Create Linear client
        println!("\n{}", "Testing Linear API connection...".blue());
        let linear_client = linear::LinearClient::new(&app_config.linear_api_key);
        
        let linear_test = linear_client.test_connection(args.verbose).await;
        if linear_test.is_err() {
            println!("\n{}", "❌ Error: Linear API connection failed. Please check your API key and try again.".red());
            return Ok(());
        }
        
        println!("\n{}", "✅ Linear API connection successful".green());
        
        // Fetch ticket by ID
        println!("\n{}", format!("Fetching ticket with ID: {}...", ticket_id).blue());
        let ticket = linear_client.fetch_ticket_by_id(ticket_id, args.verbose).await
            .context(format!("Failed to fetch ticket with ID: {}", ticket_id))?;
        
        // Enrich ticket with additional information
        println!("\n{}", "Gathering additional information about the ticket...".blue());
        let skip_labels = !args.plan;
        let enriched_ticket = linear_client.enrich_ticket(&ticket, args.verbose, skip_labels).await?;
        
        // Create tickets directory
        std::fs::create_dir_all(&args.tickets_dir).context("Failed to create tickets directory")?;
        
        // Create safe filename with format ticketId-title.md
        let safe_title = enriched_ticket.title.chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>();
            
        let filename = format!("{}-{}.md", enriched_ticket.id, &safe_title[..std::cmp::min(50, safe_title.len())]);
        
        // Save ticket to tickets directory
        let ticket_file_path = args.tickets_dir.join(&filename);
        
        // Create labels string
        let labels_str = if enriched_ticket.labels.is_empty() {
            "None".to_string()
        } else {
            enriched_ticket.labels.join(", ")
        };
        
        // Create related tickets string
        let related_tickets_str = if enriched_ticket.related_tickets.is_empty() {
            "None".to_string()
        } else {
            enriched_ticket.related_tickets.iter()
                .map(|rt| format!("- {} (State: {})", rt.title, rt.state))
                .collect::<Vec<String>>()
                .join("\n")
        };
        
        // Create children tickets string
        let children_str = if enriched_ticket.children.is_empty() {
            "None".to_string()
        } else {
            enriched_ticket.children.iter()
                .map(|child| format!("- {} (State: {})", child.title, child.state))
                .collect::<Vec<String>>()
                .join("\n")
        };
        
        // Create comments string
        let comments_str = if enriched_ticket.comments.is_empty() {
            "None".to_string()
        } else {
            enriched_ticket.comments.iter()
                .map(|comment| {
                    let user = comment.user.as_deref().unwrap_or("Unknown");
                    format!("- {} ({}): {}", user, comment.created_at.format("%Y-%m-%d"), comment.body)
                })
                .collect::<Vec<String>>()
                .join("\n")
        };
        
        // Create the file content with ticket information
        let ticket_file_content = format!(
            "# Ticket: {}\n\n\
             **Ticket ID:** {}\n\
             **State:** {}\n\
             **Priority:** {}\n\
             **Estimate:** {}\n\
             **URL:** {}\n\
             **Labels:** {}\n\n\
             ## Description\n\n{}\n\n\
             ## Comments\n\n{}\n\n\
             ## Related Tickets\n\n{}\n\n\
             ## Child Tickets\n\n{}\n\n",
            enriched_ticket.title,
            enriched_ticket.id,
            enriched_ticket.state,
            enriched_ticket.priority,
            enriched_ticket.estimate.map_or("Not estimated".to_string(), |e| e.to_string()),
            enriched_ticket.url,
            labels_str,
            enriched_ticket.description,
            comments_str,
            related_tickets_str,
            children_str
        );
        
        // Write the ticket information to the tickets directory
        std::fs::write(&ticket_file_path, &ticket_file_content)
            .context("Failed to write ticket information to file")?;
        
        // Get absolute path
        let abs_path = std::fs::canonicalize(&ticket_file_path)
            .unwrap_or_else(|_| ticket_file_path.clone());
        println!("{}", format!("✅ Ticket information saved to {}", abs_path.display()).green());
        
        // If --plan flag is provided, also generate an implementation plan
        if args.plan {
            // We need to test the Anthropic API connection first
            println!("\n{}", "Testing Anthropic API connection...".blue());
            let anthropic_client = anthropic::AnthropicClient::new(&app_config.anthropic_api_key);
            
            let anthropic_test = anthropic_client.test_connection().await;
            if anthropic_test.is_err() {
                println!("\n{}", "❌ Error: Anthropic API connection failed. Please check your API key and try again.".red());
                return Ok(());
            }
            
            println!("\n{}", "✅ Anthropic API connection successful".green());
            
            // Create output directory
            std::fs::create_dir_all(&args.output).context("Failed to create implementation plans directory")?;
            
            // Generate implementation plan
            println!("\n{}", format!("Generating implementation plan for: {}", enriched_ticket.title).blue());
            
            let implementation_plan = anthropic_client
                .generate_implementation_plan(&enriched_ticket, &app_config.anthropic_model)
                .await?;
            
            // Path for the implementation plan (using the same filename format for consistency)
            let plan_file_path = args.output.join(&filename);
            
            // Create the file content with implementation plan
            let plan_file_content = format!(
                "# Implementation Plan: {}\n\n\
                 **Ticket ID:** {}\n\
                 **State:** {}\n\
                 **Priority:** {}\n\
                 **Estimate:** {}\n\
                 **URL:** {}\n\n\
                 ---\n\n\
                 {}",
                enriched_ticket.title,
                enriched_ticket.id,
                enriched_ticket.state,
                enriched_ticket.priority,
                enriched_ticket.estimate.map_or("Not estimated".to_string(), |e| e.to_string()),
                enriched_ticket.url,
                implementation_plan
            );
            
            // Write the implementation plan to the output directory
            std::fs::write(&plan_file_path, plan_file_content)
                .context("Failed to write implementation plan to file")?;
            
            // Get absolute path
            let abs_path = std::fs::canonicalize(&plan_file_path)
                .unwrap_or_else(|_| plan_file_path.clone());
            println!("{}", format!("✅ Implementation plan saved to {}", abs_path.display()).green());
        }
        
        return Ok(());
    }

    // Regular mode: fetch tickets from Linear
    // Load environment variables from .env file if specified
    if let Some(env_path) = &args.env {
        dotenv::from_path(env_path).context("Failed to load .env file")?;
    } else {
        // Try to load from default locations
        let env_locations = config::AppConfig::get_env_locations();
        let mut loaded = false;
        
        for location in env_locations {
            if location.exists() {
                dotenv::from_path(&location).context(format!("Failed to load .env from {}", location.display()))?;
                println!("Loaded configuration from {}", location.display());
                loaded = true;
                break;
            }
        }
        
        if !loaded {
            // If no .env file found, try loading from default location just in case
            dotenv().ok();
        }
    }

    // Load or create configuration
    let app_config = if args.setup {
        ui::setup_wizard().await?
    } else {
        config::AppConfig::load(None, &args).await?
    };

    // Test API connections
    println!("\n{}", "Testing API connections...".blue());
    let linear_client = linear::LinearClient::new(&app_config.linear_api_key);
    let anthropic_client = anthropic::AnthropicClient::new(&app_config.anthropic_api_key);

    let linear_test = linear_client.test_connection(args.verbose).await;
    let anthropic_test = anthropic_client.test_connection().await;

    if linear_test.is_err() || anthropic_test.is_err() {
        println!("\n{}", "❌ Error: One or more API connections failed. Please check your API keys and try again.".red());
        return Ok(());
    }

    println!("\n{}", "✅ API connections successful".green());

    // Fetch tickets assigned to the user
    println!("\n{}", format!("Fetching tickets assigned to {}...", app_config.linear_agent_user).blue());
    let tickets = linear_client.fetch_user_tickets(
        &app_config.linear_team_name,
        &app_config.linear_agent_user,
        &app_config.linear_agent_states,
        args.verbose,
    ).await?;

    if tickets.is_empty() {
        println!("\n{}", format!("⚠️ No tickets found for user '{}'", app_config.linear_agent_user).yellow());
        println!("{}", format!("Please check if the user exists in Linear and has tickets assigned in the states: {}", 
                          app_config.linear_agent_states.join(", ")).yellow());
        return Ok(());
    }

    // Display tickets
    ui::display_tickets(&tickets);

    // Always interactive - get user's selection of tickets to process
    let selected_indices = ui::get_user_selection(&tickets, args.plan)?;
    if selected_indices.is_empty() {
        println!("\n{}", "No tickets selected. Exiting.".yellow());
        return Ok(());
    }
    
    let selected_tickets: Vec<Ticket> = selected_indices.into_iter()
        .map(|i| tickets[i].clone())
        .collect();

    let message = if args.plan {
        format!("Selected {} tickets for implementation plan generation:", selected_tickets.len())
    } else {
        format!("Selected {} tickets for analysis:", selected_tickets.len())
    };
    println!("\n{}", message.blue());
    for (i, ticket) in selected_tickets.iter().enumerate() {
        println!("{}. {}", i + 1, ticket.title);
    }

    // Enrich selected tickets with additional information
    println!("\n{}", "Gathering additional information about selected tickets...".blue());
    let mut enriched_tickets = Vec::new();
    
    let progress_bar = indicatif::ProgressBar::new(selected_tickets.len() as u64);
    for ticket in &selected_tickets {
        progress_bar.println(format!("Enriching ticket: {}", ticket.title));
        // Skip fetching labels if not needed unless we're generating plans
        let skip_labels = !args.plan;
        let enriched = linear_client.enrich_ticket(ticket, args.verbose, skip_labels).await?;
        enriched_tickets.push(enriched);
        progress_bar.inc(1);
    }
    progress_bar.finish_with_message("All tickets enriched");

    // Always create the tickets directory to store ticket information
    std::fs::create_dir_all(&args.tickets_dir).context("Failed to create tickets directory")?;
    
    // If generating plans, create the output directory too
    if args.plan {
        std::fs::create_dir_all(&args.output).context("Failed to create implementation plans directory")?;
    }

    // Process each enriched ticket
    for (i, ticket) in enriched_tickets.iter().enumerate() {
        let safe_title = ticket.title.chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>();
            
        // Use the new filename format: ticketId-title.md
        let filename = format!("{}-{}.md", ticket.id, &safe_title[..std::cmp::min(50, safe_title.len())]);
        
        // Always save the ticket information to the tickets directory
        let ticket_file_path = args.tickets_dir.join(&filename);
        
        // Create labels string
        let labels_str = if ticket.labels.is_empty() {
            "None".to_string()
        } else {
            ticket.labels.join(", ")
        };
        
        // Create related tickets string
        let related_tickets_str = if ticket.related_tickets.is_empty() {
            "None".to_string()
        } else {
            ticket.related_tickets.iter()
                .map(|rt| format!("- {} (State: {})", rt.title, rt.state))
                .collect::<Vec<String>>()
                .join("\n")
        };
        
        // Create children tickets string
        let children_str = if ticket.children.is_empty() {
            "None".to_string()
        } else {
            ticket.children.iter()
                .map(|child| format!("- {} (State: {})", child.title, child.state))
                .collect::<Vec<String>>()
                .join("\n")
        };
        
        // Create comments string
        let comments_str = if ticket.comments.is_empty() {
            "None".to_string()
        } else {
            ticket.comments.iter()
                .map(|comment| {
                    let user = comment.user.as_deref().unwrap_or("Unknown");
                    format!("- {} ({}): {}", user, comment.created_at.format("%Y-%m-%d"), comment.body)
                })
                .collect::<Vec<String>>()
                .join("\n")
        };
        
        // Create the file content with ticket information
        let ticket_file_content = format!(
            "# Ticket: {}\n\n\
             **Ticket ID:** {}\n\
             **State:** {}\n\
             **Priority:** {}\n\
             **Estimate:** {}\n\
             **URL:** {}\n\
             **Labels:** {}\n\n\
             ## Description\n\n{}\n\n\
             ## Comments\n\n{}\n\n\
             ## Related Tickets\n\n{}\n\n\
             ## Child Tickets\n\n{}\n\n\
             ",
            ticket.title,
            ticket.id,
            ticket.state,
            ticket.priority,
            ticket.estimate.map_or("Not estimated".to_string(), |e| e.to_string()),
            ticket.url,
            labels_str,
            ticket.description,
            comments_str,
            related_tickets_str,
            children_str
        );
        
        // Always write the ticket information to the tickets directory
        println!("\n{}", format!("[{}/{}] Saving ticket information: {}", 
                         i + 1, enriched_tickets.len(), ticket.title).blue());
        
        std::fs::write(&ticket_file_path, &ticket_file_content)
            .context("Failed to write ticket information to file")?;
        
        // Get absolute path
        let abs_path = std::fs::canonicalize(&ticket_file_path)
            .unwrap_or_else(|_| ticket_file_path.clone());
        println!("{}", format!("✅ Ticket information saved to {}", abs_path.display()).green());
        
        // If --plan flag is provided, also generate an implementation plan
        if args.plan {
            println!("\n{}", format!("[{}/{}] Generating implementation plan for: {}", 
                              i + 1, enriched_tickets.len(), ticket.title).blue());
            
            // Generate implementation plan
            let implementation_plan = anthropic_client
                .generate_implementation_plan(ticket, &app_config.anthropic_model)
                .await?;
            
            // Path for the implementation plan
            let plan_file_path = args.output.join(&filename);
            
            // Create the file content with implementation plan
            let plan_file_content = format!(
                "# Implementation Plan: {}\n\n\
                 **Ticket ID:** {}\n\
                 **State:** {}\n\
                 **Priority:** {}\n\
                 **Estimate:** {}\n\
                 **URL:** {}\n\n\
                 ---\n\n\
                 {}",
                ticket.title,
                ticket.id,
                ticket.state,
                ticket.priority,
                ticket.estimate.map_or("Not estimated".to_string(), |e| e.to_string()),
                ticket.url,
                implementation_plan
            );
            
            // Write the implementation plan to the output directory
            std::fs::write(&plan_file_path, plan_file_content)
                .context("Failed to write implementation plan to file")?;
            
            // Get absolute path
            let abs_path = std::fs::canonicalize(&plan_file_path)
                .unwrap_or_else(|_| plan_file_path.clone());
            println!("{}", format!("✅ Implementation plan saved to {}", abs_path.display()).green());
        }
    }
    
    if !enriched_tickets.is_empty() {
        // Always show message about saved tickets
        println!("\n{}", "✅ All ticket information saved successfully".green());
        // Get absolute path
        let tickets_abs_path = std::fs::canonicalize(&args.tickets_dir)
            .unwrap_or_else(|_| args.tickets_dir.clone());
        println!("{}", format!("Ticket information saved to the '{}' directory", tickets_abs_path.display()).blue());
        
        // If plans were generated, show message about that too
        if args.plan {
            println!("\n{}", "✅ All implementation plans generated successfully".green());
            // Get absolute path
            let output_abs_path = std::fs::canonicalize(&args.output)
                .unwrap_or_else(|_| args.output.clone());
            println!("{}", format!("Implementation plans saved to the '{}' directory", output_abs_path.display()).blue());
        }
    }
    
    Ok(())
}

/// Check for updates by comparing the current version with the latest release on GitHub
async fn check_for_updates() -> Result<()> {
    println!("{}", "Checking for updates...".blue());
    
    // Get current version from Cargo.toml
    let current_version = env!("CARGO_PKG_VERSION");
    println!("Current version: {}", current_version);
    
    // GitHub API endpoint for the latest release
    let github_url = "https://api.github.com/repos/yourusername/linear-agent/releases/latest";
    
    // Create a client with proper user-agent header (required by GitHub API)
    let client = reqwest::Client::builder()
        .user_agent("linear-agent-updater")
        .build()?;
    
    // Make the request to the GitHub API
    let response = client.get(github_url).send().await;
    
    match response {
        Ok(response) => {
            if response.status().is_success() {
                // Parse the JSON response
                let release: serde_json::Value = response.json().await?;
                
                // Extract the latest version (without 'v' prefix)
                if let Some(tag_name) = release["tag_name"].as_str() {
                    let latest_version = tag_name.trim_start_matches('v');
                    println!("Latest version: {}", latest_version);
                    
                    // Compare versions
                    if latest_version != current_version {
                        println!("{}", "A new version is available!".green());
                        println!("To update, run one of the following commands based on your platform:");
                        
                        println!("\nLinux (x86_64):");
                        println!("curl -L https://github.com/yourusername/linear-agent/releases/latest/download/linear-agent-linux-amd64 -o linear-agent && chmod +x linear-agent && sudo mv linear-agent /usr/local/bin/");
                        
                        println!("\nmacOS (Intel):");
                        println!("curl -L https://github.com/yourusername/linear-agent/releases/latest/download/linear-agent-macos-amd64 -o linear-agent && chmod +x linear-agent && sudo mv linear-agent /usr/local/bin/");
                        
                        println!("\nmacOS (Apple Silicon):");
                        println!("curl -L https://github.com/yourusername/linear-agent/releases/latest/download/linear-agent-macos-arm64 -o linear-agent && chmod +x linear-agent && sudo mv linear-agent /usr/local/bin/");
                        
                        println!("\nOr download directly from: {}", release["html_url"].as_str().unwrap_or("https://github.com/yourusername/linear-agent/releases"));
                    } else {
                        println!("{}", "You are using the latest version!".green());
                    }
                } else {
                    println!("{}", "Failed to extract version from the latest release.".red());
                }
            } else {
                println!("{}", format!("Failed to check for updates: HTTP {}", response.status()).red());
            }
        }
        Err(err) => {
            println!("{}", format!("Failed to check for updates: {}", err).red());
        }
    }
    
    Ok(())
}
