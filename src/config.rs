use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use dirs::home_dir;
use std::env;
use std::fs;
use std::io::Write;

use crate::Args;

const DEFAULT_ENV_FILENAME: &str = ".env";
const DEFAULT_CONFIG_DIR: &str = ".linear-agent";

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppConfig {
    pub linear_api_key: String,
    pub anthropic_api_key: Option<String>,
    pub linear_team_name: String,
    pub linear_agent_user: String,
    pub linear_agent_states: Vec<String>,
    pub anthropic_model: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            linear_api_key: String::new(),
            anthropic_api_key: None,
            linear_team_name: "Engineering".to_string(),
            linear_agent_user: String::new(),
            linear_agent_states: vec!["Open".to_string(), "In Progress".to_string()],
            anthropic_model: "claude-3-7-sonnet-20250219".to_string(),
        }
    }
}

impl AppConfig {
    /// Loads configuration from environment variables
    pub async fn load(_unused: Option<&Path>, args: &Args) -> Result<Self> {
        // Start with default config
        let mut config = Self::default();
        
        // Load environment variables (they should already be loaded in main.rs)
        
        // Get config from environment variables
        if let Ok(key) = env::var("LINEAR_API_KEY") {
            config.linear_api_key = key;
        }
        
        // Make Anthropic API key optional
        if let Ok(key) = env::var("ANTHROPIC_API_KEY") {
            config.anthropic_api_key = Some(key);
        }
        
        if let Ok(team) = env::var("LINEAR_TEAM_NAME") {
            config.linear_team_name = team;
        }
        
        if let Ok(user) = env::var("LINEAR_AGENT_USER") {
            config.linear_agent_user = user;
        }
        
        if let Ok(states) = env::var("LINEAR_AGENT_STATES") {
            config.linear_agent_states = states
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
        }
        
        if let Ok(model) = env::var("ANTHROPIC_MODEL") {
            config.anthropic_model = model;
        }
        
        // Override with command line arguments
        if let Some(user) = &args.user {
            config.linear_agent_user = user.clone();
        }
        
        if let Some(team) = &args.team {
            config.linear_team_name = team.clone();
        }
        
        if let Some(states) = &args.states {
            config.linear_agent_states = states
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
        }
        
        if let Some(model) = &args.model {
            config.anthropic_model = model.clone();
        }
        
        Ok(config)
    }
    
    /// Save configuration to a .env file
    pub fn save(&self, path: Option<&Path>) -> Result<PathBuf> {
        let env_path = if let Some(p) = path {
            p.to_path_buf()
        } else {
            // Create the default config directory in home
            let home = home_dir().context("Failed to find home directory")?;
            let config_dir = home.join(DEFAULT_CONFIG_DIR);
            if !config_dir.exists() {
                fs::create_dir_all(&config_dir).context("Failed to create config directory")?;
            }
            config_dir.join(DEFAULT_ENV_FILENAME)
        };
        
        // Create the .env file content
        let mut content = String::new();
        content.push_str(&format!("LINEAR_API_KEY={}\n", self.linear_api_key));
        // Include Anthropic API key if available
        if let Some(api_key) = &self.anthropic_api_key {
            content.push_str(&format!("ANTHROPIC_API_KEY={}\n", api_key));
        }
        content.push_str(&format!("LINEAR_TEAM_NAME={}\n", self.linear_team_name));
        content.push_str(&format!("LINEAR_AGENT_USER={}\n", self.linear_agent_user));
        content.push_str(&format!("LINEAR_AGENT_STATES={}\n", self.linear_agent_states.join(",")));
        content.push_str(&format!("ANTHROPIC_MODEL={}\n", self.anthropic_model));
        
        // Write to file
        let mut file = fs::File::create(&env_path)
            .context(format!("Failed to create .env file at {}", env_path.display()))?;
        file.write_all(content.as_bytes())
            .context("Failed to write to .env file")?;
            
        Ok(env_path)
    }
    
    /// Get standard locations for .env file
    pub fn get_env_locations() -> Vec<PathBuf> {
        let mut locations = Vec::new();
        
        // Current directory
        locations.push(PathBuf::from(DEFAULT_ENV_FILENAME));
        
        // Home directory
        if let Some(home) = home_dir() {
            // ~/.linear-agent/.env
            locations.push(home.join(DEFAULT_CONFIG_DIR).join(DEFAULT_ENV_FILENAME));
        }
        
        locations
    }
}