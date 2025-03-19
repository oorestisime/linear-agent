use anyhow::{Context, Result};
use reqwest::Client;

use crate::models::{Ticket, AnthropicRequest, AnthropicResponse, AnthropicMessage};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";

pub struct AnthropicClient {
    client: Client,
    api_key: String,
}

impl AnthropicClient {
    pub fn new(api_key: &str) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.to_string(),
        }
    }
    
    /// Test the connection to the Anthropic API
    pub async fn test_connection(&self) -> Result<String> {
        let response = self.generate_text(
            "claude-3-7-sonnet-20250219",
            "Hello, this is a test message. Please respond with a short greeting.",
        ).await?;
        
        Ok(response)
    }
    
    /// Generate implementation plan for a ticket
    pub async fn generate_implementation_plan(&self, ticket: &Ticket, model: &str) -> Result<String> {
        // Build the prompt
        let prompt = self.build_implementation_plan_prompt(ticket);
        
        // Call the API
        let implementation_plan = self.generate_text(model, &prompt).await?;
        
        Ok(implementation_plan)
    }
    
    /// Build prompt for implementation plan generation
    fn build_implementation_plan_prompt(&self, ticket: &Ticket) -> String {
        let mut prompt = String::from(
            "You are a software engineering expert helping to create implementation plans for software development tickets.\n\n\
             I'm going to provide you with a ticket from our project management system. Based on the ticket details,\n\
             generate a detailed implementation plan. The plan should include:\n\n\
             1. An overview of the task\n\
             2. Technical requirements and considerations\n\
             3. Step-by-step implementation approach\n\
             4. Potential challenges and solutions\n\
             5. Testing strategy\n\
             6. Estimated effort (in hours or story points)\n\n\
             Here's the ticket information:\n\n"
        );
        
        // Add ticket details
        prompt.push_str(&format!("Title: {}\n", ticket.title));
        prompt.push_str(&format!("Description: {}\n", ticket.description));
        prompt.push_str(&format!("Priority: {}\n", ticket.priority));
        prompt.push_str(&format!("Estimate: {}\n", ticket.estimate.map_or("Not estimated".to_string(), |e| e.to_string())));
        prompt.push_str(&format!("State: {}\n", ticket.state));
        prompt.push_str(&format!("Labels: {}\n", if ticket.labels.is_empty() { "None".to_string() } else { ticket.labels.join(", ") }));
        prompt.push_str(&format!("Created: {}\n", ticket.created_at.format("%Y-%m-%d")));
        prompt.push_str(&format!("Updated: {}\n\n", ticket.updated_at.format("%Y-%m-%d")));
        
        // Add comments
        prompt.push_str("Comments:\n");
        if ticket.comments.is_empty() {
            prompt.push_str("No comments\n");
        } else {
            for comment in &ticket.comments {
                let user_str = match &comment.user {
                    Some(u) => u.clone(),
                    None => "Unknown".to_string()
                };
                prompt.push_str(&format!("- {} ({}): {}\n", 
                    user_str, 
                    comment.created_at.format("%Y-%m-%d"), 
                    comment.body
                ));
            }
        }
        prompt.push_str("\n");
        
        // Add parent ticket
        if let Some(parent) = &ticket.parent {
            prompt.push_str(&format!("Parent Ticket: {} (State: {})\n\n", parent.title, parent.state));
        } else {
            prompt.push_str("No parent ticket\n\n");
        }
        
        // Add child tickets
        prompt.push_str("Child Tickets:\n");
        if ticket.children.is_empty() {
            prompt.push_str("No child tickets\n");
        } else {
            for child in &ticket.children {
                prompt.push_str(&format!("- {} (State: {})\n", child.title, child.state));
            }
        }
        prompt.push_str("\n");
        
        // Add related tickets
        prompt.push_str("Related Tickets:\n");
        if ticket.related_tickets.is_empty() {
            prompt.push_str("No related tickets\n");
        } else {
            for related in &ticket.related_tickets {
                let assignee_str = match &related.assignee {
                    Some(a) => a.clone(),
                    None => "Unassigned".to_string()
                };
                prompt.push_str(&format!("- {} (State: {}, Assignee: {})\n", 
                    related.title, 
                    related.state, 
                    assignee_str
                ));
            }
        }
        prompt.push_str("\n");
        
        // Final instruction
        prompt.push_str("Please provide a detailed implementation plan for this ticket.");
        
        prompt
    }
    
    /// Generate text using the Anthropic API
    async fn generate_text(&self, model: &str, prompt: &str) -> Result<String> {
        let request = AnthropicRequest {
            model: model.to_string(),
            max_tokens: 4000,
            messages: vec![
                AnthropicMessage {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                },
            ],
        };
        
        let response = self.client.post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Anthropic API")?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            anyhow::bail!("Anthropic API request failed with status {}: {}", status, error_text);
        }
        
        let anthropic_response: AnthropicResponse = response.json()
            .await
            .context("Failed to deserialize Anthropic API response")?;
            
        // Extract the text from the first content item
        if let Some(content) = anthropic_response.content.first() {
            Ok(content.text.clone())
        } else {
            anyhow::bail!("Anthropic API returned empty response")
        }
    }
}