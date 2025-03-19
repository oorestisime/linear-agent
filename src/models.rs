use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticket {
    pub id: String,
    pub title: String,
    pub description: String,
    pub priority: i32,
    pub estimate: Option<f64>,
    pub labels: Vec<String>,
    pub url: String,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub assignee: Option<String>,
    pub comments: Vec<Comment>,
    pub parent: Option<RelatedTicket>,
    pub children: Vec<RelatedTicket>,
    pub related_tickets: Vec<RelatedTicket>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedTicket {
    pub id: String,
    pub title: String,
    pub state: String,
    pub assignee: Option<String>,
}

impl Ticket {
    pub fn new(
        id: String,
        title: String,
        description: String,
        priority: i32,
        estimate: Option<f64>,
        labels: Vec<String>,
        url: String,
        state: String,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        assignee: Option<String>,
    ) -> Self {
        Self {
            id,
            title,
            description,
            priority,
            estimate,
            labels,
            url,
            state,
            created_at,
            updated_at,
            assignee,
            comments: Vec::new(),
            parent: None,
            children: Vec::new(),
            related_tickets: Vec::new(),
        }
    }

    /// Parse a ticket from a markdown file that was saved by this tool
    pub fn from_markdown(content: &str) -> Result<Self, anyhow::Error> {
        // First line should be the title
        let mut lines = content.lines();
        let title_line = lines
            .next()
            .ok_or_else(|| anyhow::anyhow!("Missing title line"))?;
        let title = title_line.trim_start_matches("# Ticket: ").to_string();

        // Initialize fields with defaults
        let mut id = String::new();
        let mut description = String::new();
        let mut priority = 0;
        let mut estimate = None;
        let mut labels = Vec::new();
        let mut url = String::new();
        let mut state = String::new();
        let mut in_description_section = false;
        let mut comment_section_start = false;
        let mut comments = Vec::new();
        let mut current_comment = String::new();
        let mut comment_user = None;
        let mut comment_date = None;
        let mut related_tickets = Vec::new();
        let mut children = Vec::new();

        // Parse the rest of the file
        for line in lines {
            // Parse metadata
            if line.starts_with("**Ticket ID:**") {
                id = line.trim_start_matches("**Ticket ID:**").trim().to_string();
            } else if line.starts_with("**State:**") {
                state = line.trim_start_matches("**State:**").trim().to_string();
            } else if line.starts_with("**Priority:**") {
                let priority_str = line.trim_start_matches("**Priority:**").trim();
                priority = priority_str.parse().unwrap_or(0);
            } else if line.starts_with("**Estimate:**") {
                let estimate_str = line.trim_start_matches("**Estimate:**").trim();
                if !estimate_str.contains("Not estimated") {
                    estimate = estimate_str.parse().ok();
                }
            } else if line.starts_with("**URL:**") {
                url = line.trim_start_matches("**URL:**").trim().to_string();
            } else if line.starts_with("**Labels:**") {
                let labels_str = line.trim_start_matches("**Labels:**").trim();
                if labels_str != "None" {
                    labels = labels_str.split(", ").map(|s| s.to_string()).collect();
                }
            }
            // Handle description section
            else if line.contains("## Description") {
                in_description_section = true;
                continue;
            } else if line.contains("## Comments") {
                in_description_section = false;
                comment_section_start = true;
                continue;
            } else if line.contains("## Related Tickets") {
                comment_section_start = false;
                continue;
            } else if line.contains("## Child Tickets") {
                continue;
            }
            // Process description content
            else if in_description_section {
                if !description.is_empty() {
                    description.push_str("\n");
                }
                description.push_str(line);
            }
            // Process comments
            else if comment_section_start
                && line.starts_with("- ")
                && line.contains("(")
                && line.contains("): ")
            {
                // If we were already building a comment, save it
                if !current_comment.is_empty() && comment_user.is_some() && comment_date.is_some() {
                    comments.push(Comment {
                        id: format!("from_file_{}", comments.len()),
                        body: current_comment.trim().to_string(),
                        created_at: chrono::DateTime::parse_from_str(
                            comment_date.unwrap(),
                            "%Y-%m-%d",
                        )
                        .unwrap_or_else(|_| {
                            chrono::DateTime::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap()
                        })
                        .with_timezone(&chrono::Utc),
                        user: comment_user.clone(),
                    });
                    current_comment = String::new();
                }

                // Parse comment header
                let parts: Vec<&str> = line.splitn(2, ": ").collect();
                if parts.len() == 2 {
                    let header = parts[0];
                    let content = parts[1];

                    // Extract user and date
                    let header_parts: Vec<&str> =
                        header.trim_start_matches("- ").split(" (").collect();
                    if header_parts.len() == 2 {
                        comment_user = Some(header_parts[0].to_string());
                        comment_date = Some(header_parts[1].trim_end_matches(")"));
                        current_comment = content.to_string();
                    }
                }
            }
            // Process related tickets or children
            else if line.starts_with("- ") && line.contains("(State: ") {
                // This is either a related ticket or a child ticket
                let parts: Vec<&str> = line.trim_start_matches("- ").split(" (State: ").collect();
                if parts.len() == 2 {
                    let ticket_title = parts[0].to_string();
                    let ticket_state = parts[1].trim_end_matches(")").to_string();

                    // Based on the current section, add to related or children
                    if line.contains("## Related Tickets") {
                        related_tickets.push(RelatedTicket {
                            id: format!("placeholder_{}", related_tickets.len()),
                            title: ticket_title,
                            state: ticket_state,
                            assignee: None,
                        });
                    } else {
                        children.push(RelatedTicket {
                            id: format!("placeholder_{}", children.len()),
                            title: ticket_title,
                            state: ticket_state,
                            assignee: None,
                        });
                    }
                }
            }
        }

        // Add the last comment if any
        if !current_comment.is_empty() && comment_user.is_some() && comment_date.is_some() {
            comments.push(Comment {
                id: format!("from_file_{}", comments.len()),
                body: current_comment.trim().to_string(),
                created_at: chrono::DateTime::parse_from_str(comment_date.unwrap(), "%Y-%m-%d")
                    .unwrap_or_else(|_| {
                        chrono::DateTime::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap()
                    })
                    .with_timezone(&chrono::Utc),
                user: comment_user.clone(),
            });
        }

        // Create the ticket with parsed information
        Ok(Self {
            id,
            title,
            description,
            priority,
            estimate,
            labels,
            url,
            state,
            // Use current time for created/updated
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            assignee: None,
            comments,
            parent: None,
            children,
            related_tickets,
        })
    }
}

// Linear GraphQL response types

#[derive(Debug, Deserialize)]
pub struct LinearResponse<T> {
    pub data: T,
}

// Using an empty struct for simplicity since we don't actually use the content
#[derive(Debug, Deserialize)]
pub struct LinearErrorResponse {}

// Using an empty struct for simplicity since we don't actually use the content
#[derive(Debug, Deserialize)]
pub struct LinearError {}

#[derive(Debug, Deserialize)]
pub struct LinearViewerResponse {
    pub viewer: LinearViewer,
}

#[derive(Debug, Deserialize)]
pub struct LinearViewer {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct LinearUsersResponse {
    pub users: LinearNodesContainer<LinearUser>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinearUser {
    // These fields are required for the API but not directly referenced in our code
    // pub id: String,
    // pub name: String,
    pub assigned_issues: LinearNodesContainer<LinearIssue>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinearNodesContainer<T> {
    pub nodes: Vec<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinearIssue {
    pub identifier: String,
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<i32>,
    pub estimate: Option<f64>,
    pub url: String,
    pub state: LinearState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // Note: Fields 'assignee' and 'labels' from the API response are intentionally omitted
    // as they are not used directly. Instead, we fetch these separately in enrich_ticket.
}

#[derive(Debug, Deserialize)]
pub struct LinearState {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct LinearLabel {
    // This struct is used as a placeholder for deserialization
    // The actual label name is extracted in fetch_ticket_labels
    // using a local Label struct
}

// Anthropic types

#[derive(Debug, Serialize)]
pub struct AnthropicRequest {
    pub model: String,
    pub max_tokens: u32,
    pub messages: Vec<AnthropicMessage>,
}

#[derive(Debug, Serialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicResponse {
    pub content: Vec<AnthropicContent>,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicContent {
    pub text: String,
}
