use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde_json::json;

use crate::models::{
    Comment, LinearResponse, LinearState, LinearUsersResponse, LinearViewerResponse, RelatedTicket,
    Ticket,
};

const LINEAR_API_URL: &str = "https://api.linear.app/graphql";

pub struct LinearClient {
    client: Client,
    api_key: String,
}

impl LinearClient {
    pub fn new(api_key: &str) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.to_string(),
        }
    }

    /// Test the connection to the Linear API
    pub async fn test_connection(&self, verbose: bool) -> Result<String> {
        let query = r#"
        query {
          viewer {
            name
          }
        }
        "#;

        let response: LinearResponse<LinearViewerResponse> =
            self.execute_query(query, json!({}), verbose).await?;
        Ok(response.data.viewer.name)
    }

    /// Fetch a ticket by its ID
    pub async fn fetch_ticket_by_id(&self, ticket_id: &str, verbose: bool) -> Result<Ticket> {
        let query = r#"
        query TicketById($id: String!) {
          issue(id: $id) {
            id
            identifier
            title
            description
            priority
            estimate
            url
            state {
              name
            }
            createdAt
            updatedAt
            assignee {
              name
            }
          }
        }
        "#;

        let variables = json!({
            "id": ticket_id
        });

        #[derive(serde::Deserialize)]
        struct IssueResponse {
            issue: LinearIssue,
        }

        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct LinearIssue {
            identifier: String,
            title: String,
            description: Option<String>,
            priority: Option<i32>,
            estimate: Option<f64>,
            url: String,
            state: LinearState,
            created_at: DateTime<Utc>,
            updated_at: DateTime<Utc>,
            assignee: Option<LinearAssignee>,
        }

        #[derive(serde::Deserialize)]
        struct LinearAssignee {
            name: String,
        }

        let response: LinearResponse<IssueResponse> =
            self.execute_query(query, variables, verbose).await?;

        let issue = &response.data.issue;

        let ticket = Ticket::new(
            issue.identifier.clone(), // Use the identifier field for the ticket ID
            issue.title.clone(),
            issue.description.clone().unwrap_or_default(),
            issue.priority.unwrap_or(0),
            issue.estimate,
            Vec::new(), // Will be populated in enrich_ticket
            issue.url.clone(),
            issue.state.name.clone(),
            issue.created_at,
            issue.updated_at,
            issue.assignee.as_ref().map(|a| a.name.clone()),
        );

        Ok(ticket)
    }

    /// Fetch tickets assigned to a specific user
    pub async fn fetch_user_tickets(
        &self,
        team_name: &str,
        user_name: &str,
        states: &[String],
        verbose: bool,
    ) -> Result<Vec<Ticket>> {
        let query = r#"
        query UserTickets($teamName: String!, $assigneeName: String!, $states: [String!]!) {
          users(filter: { name: { eq: $assigneeName } }) {
            nodes {
              id
              name
              assignedIssues(
                filter: {
                  team: {
                    name: {
                      eq: $teamName
                    }
                  }
                  state: { 
                    name: { 
                      in: $states 
                    } 
                  }
                  assignee: {
                    name: {
                      eq: $assigneeName
                    }
                  }
                }
              ) {
                nodes {
                  id
                  identifier
                  title
                  description
                  priority
                  estimate
                  url
                  state {
                    name
                  }
                  createdAt
                  updatedAt
                }
              }
            }
          }
        }
        "#;

        let variables = json!({
            "teamName": team_name,
            "assigneeName": user_name,
            "states": states
        });

        let response: LinearResponse<LinearUsersResponse> =
            self.execute_query(query, variables, verbose).await?;

        // Check if user exists
        let users = &response.data.users.nodes;
        if users.is_empty() {
            anyhow::bail!("User '{}' not found", user_name);
        }

        // Get assigned issues
        let user = &users[0];
        let issues = &user.assigned_issues.nodes;

        let tickets = issues
            .iter()
            .map(|issue| {
                Ticket::new(
                    issue.identifier.clone(), // Use the identifier field instead of id
                    issue.title.clone(),
                    issue.description.clone().unwrap_or_default(),
                    issue.priority.unwrap_or(0),
                    issue.estimate,
                    Vec::new(), // Will be populated in enrich_ticket
                    issue.url.clone(),
                    issue.state.name.clone(),
                    issue.created_at,
                    issue.updated_at,
                    Some(user_name.to_string()),
                )
            })
            .collect();

        Ok(tickets)
    }

    /// Enrich a ticket with additional information
    pub async fn enrich_ticket(
        &self,
        ticket: &Ticket,
        verbose: bool,
        skip_labels: bool,
    ) -> Result<Ticket> {
        let mut enriched = ticket.clone();

        // Fetch labels only if not skipped
        if !skip_labels {
            enriched.labels = self.fetch_ticket_labels(&ticket.id, verbose).await?;
        }

        // Fetch comments
        enriched.comments = self.fetch_ticket_comments(&ticket.id, verbose).await?;

        // Fetch parent ticket
        enriched.parent = self.fetch_ticket_parent(&ticket.id, verbose).await?;

        // Fetch children tickets
        enriched.children = self.fetch_ticket_children(&ticket.id, verbose).await?;

        // Fetch related tickets
        enriched.related_tickets = self.fetch_related_tickets(&ticket.id, verbose).await?;

        Ok(enriched)
    }

    /// Fetch labels for a ticket
    async fn fetch_ticket_labels(&self, ticket_id: &str, verbose: bool) -> Result<Vec<String>> {
        let query = r#"
        query TicketLabels($issueId: String!) {
          issue(id: $issueId) {
            labels {
              nodes {
                name
              }
            }
          }
        }
        "#;

        let variables = json!({
            "issueId": ticket_id
        });

        #[derive(serde::Deserialize)]
        struct LabelsResponse {
            issue: IssueLabels,
        }

        #[derive(serde::Deserialize)]
        struct IssueLabels {
            labels: LabelsContainer,
        }

        #[derive(serde::Deserialize)]
        struct LabelsContainer {
            nodes: Vec<Label>,
        }

        #[derive(serde::Deserialize)]
        struct Label {
            name: String,
        }

        let response: LinearResponse<LabelsResponse> =
            self.execute_query(query, variables, verbose).await?;

        let labels = response
            .data
            .issue
            .labels
            .nodes
            .iter()
            .map(|label| label.name.clone())
            .collect();

        Ok(labels)
    }

    /// Fetch comments for a ticket
    async fn fetch_ticket_comments(&self, ticket_id: &str, verbose: bool) -> Result<Vec<Comment>> {
        let query = r#"
        query TicketComments($issueId: String!) {
          issue(id: $issueId) {
            comments {
              nodes {
                id
                body
                createdAt
                user {
                  name
                }
              }
            }
          }
        }
        "#;

        let variables = json!({
            "issueId": ticket_id
        });

        #[derive(serde::Deserialize)]
        struct CommentsResponse {
            issue: IssueComments,
        }

        #[derive(serde::Deserialize)]
        struct IssueComments {
            comments: CommentsContainer,
        }

        #[derive(serde::Deserialize)]
        struct CommentsContainer {
            nodes: Vec<CommentNode>,
        }

        #[derive(serde::Deserialize)]
        struct CommentNode {
            id: String,
            body: String,
            #[serde(rename = "createdAt")]
            created_at: chrono::DateTime<Utc>,
            user: Option<User>,
        }

        #[derive(serde::Deserialize)]
        struct User {
            name: String,
        }

        let response: LinearResponse<CommentsResponse> =
            self.execute_query(query, variables, verbose).await?;

        let comments = response
            .data
            .issue
            .comments
            .nodes
            .iter()
            .map(|comment| Comment {
                id: comment.id.clone(),
                body: comment.body.clone(),
                created_at: comment.created_at,
                user: comment.user.as_ref().map(|u| u.name.clone()),
            })
            .collect();

        Ok(comments)
    }

    /// Fetch parent ticket for a ticket
    async fn fetch_ticket_parent(
        &self,
        ticket_id: &str,
        verbose: bool,
    ) -> Result<Option<RelatedTicket>> {
        let query = r#"
        query TicketParent($issueId: String!) {
          issue(id: $issueId) {
            parent {
              id
              identifier
              title
              state {
                name
              }
              assignee {
                name
              }
            }
          }
        }
        "#;

        let variables = json!({
            "issueId": ticket_id
        });

        #[derive(serde::Deserialize)]
        struct ParentResponse {
            issue: IssueParent,
        }

        #[derive(serde::Deserialize)]
        struct IssueParent {
            parent: Option<ParentTicket>,
        }

        #[derive(serde::Deserialize)]
        struct ParentTicket {
            identifier: String,
            title: String,
            state: TicketState,
            assignee: Option<TicketAssignee>,
        }

        #[derive(serde::Deserialize)]
        struct TicketState {
            name: String,
        }

        #[derive(serde::Deserialize)]
        struct TicketAssignee {
            name: String,
        }

        let response: LinearResponse<ParentResponse> =
            self.execute_query(query, variables, verbose).await?;

        let parent = response.data.issue.parent.map(|parent| RelatedTicket {
            id: parent.identifier, // Use identifier instead of id
            title: parent.title,
            state: parent.state.name,
            assignee: parent.assignee.map(|a| a.name),
        });

        Ok(parent)
    }

    /// Fetch children tickets for a ticket
    async fn fetch_ticket_children(
        &self,
        ticket_id: &str,
        verbose: bool,
    ) -> Result<Vec<RelatedTicket>> {
        let query = r#"
        query TicketChildren($issueId: String!) {
          issue(id: $issueId) {
            children {
              nodes {
                id
                identifier
                title
                state {
                  name
                }
                assignee {
                  name
                }
              }
            }
          }
        }
        "#;

        let variables = json!({
            "issueId": ticket_id
        });

        #[derive(serde::Deserialize)]
        struct ChildrenResponse {
            issue: IssueChildren,
        }

        #[derive(serde::Deserialize)]
        struct IssueChildren {
            children: ChildrenContainer,
        }

        #[derive(serde::Deserialize)]
        struct ChildrenContainer {
            nodes: Vec<ChildTicket>,
        }

        #[derive(serde::Deserialize)]
        struct ChildTicket {
            identifier: String,
            title: String,
            state: TicketState,
            assignee: Option<TicketAssignee>,
        }

        #[derive(serde::Deserialize)]
        struct TicketState {
            name: String,
        }

        #[derive(serde::Deserialize)]
        struct TicketAssignee {
            name: String,
        }

        let response: LinearResponse<ChildrenResponse> =
            self.execute_query(query, variables, verbose).await?;

        let children = response
            .data
            .issue
            .children
            .nodes
            .iter()
            .map(|child| RelatedTicket {
                id: child.identifier.clone(), // Use identifier instead of id
                title: child.title.clone(),
                state: child.state.name.clone(),
                assignee: child.assignee.as_ref().map(|a| a.name.clone()),
            })
            .collect();

        Ok(children)
    }

    /// Fetch related tickets for a ticket
    async fn fetch_related_tickets(
        &self,
        ticket_id: &str,
        verbose: bool,
    ) -> Result<Vec<RelatedTicket>> {
        let query = r#"
        query RelatedIssues($issueId: String!) {
          issue(id: $issueId) {
            relations {
              nodes {
                id
                relatedIssue {
                  id
                  identifier
                  title
                  state {
                    name
                  }
                  assignee {
                    name
                  }
                }
              }
            }
          }
        }
        "#;

        let variables = json!({
            "issueId": ticket_id
        });

        #[derive(serde::Deserialize)]
        struct RelationsResponse {
            issue: IssueRelations,
        }

        #[derive(serde::Deserialize)]
        struct IssueRelations {
            relations: RelationsContainer,
        }

        #[derive(serde::Deserialize)]
        struct RelationsContainer {
            nodes: Vec<Relation>,
        }

        #[derive(serde::Deserialize)]
        struct Relation {
            // Skip this field completely since it's not used
            #[serde(skip)]
            id: String,
            #[serde(rename = "relatedIssue")]
            related_issue: RelatedIssue,
        }

        #[derive(serde::Deserialize)]
        struct RelatedIssue {
            identifier: String,
            title: String,
            state: TicketState,
            assignee: Option<TicketAssignee>,
        }

        #[derive(serde::Deserialize)]
        struct TicketState {
            name: String,
        }

        #[derive(serde::Deserialize)]
        struct TicketAssignee {
            name: String,
        }

        let response: LinearResponse<RelationsResponse> =
            self.execute_query(query, variables, verbose).await?;

        let related = response
            .data
            .issue
            .relations
            .nodes
            .iter()
            .map(|relation| RelatedTicket {
                id: relation.related_issue.identifier.clone(), // Use identifier instead of id
                title: relation.related_issue.title.clone(),
                state: relation.related_issue.state.name.clone(),
                assignee: relation
                    .related_issue
                    .assignee
                    .as_ref()
                    .map(|a| a.name.clone()),
            })
            .collect();

        Ok(related)
    }

    /// Execute a GraphQL query against the Linear API
    async fn execute_query<T>(
        &self,
        query: &str,
        variables: serde_json::Value,
        verbose: bool,
    ) -> Result<LinearResponse<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let payload = json!({
            "query": query,
            "variables": variables
        });

        let response = self
            .client
            .post(LINEAR_API_URL)
            .header("Authorization", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .context("Failed to send request to Linear API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            anyhow::bail!(
                "Linear API request failed with status {}: {}",
                status,
                error_text
            );
        }

        // Check for GraphQL errors in the response
        let response_text = response.text().await?;

        // Debug print - only show if verbose mode is enabled
        if verbose {
            println!(
                "DEBUG API Response: {}",
                &response_text[..std::cmp::min(1000, response_text.len())]
            );
        }

        let json: serde_json::Value = serde_json::from_str(&response_text)
            .context("Failed to parse Linear API response as JSON")?;

        if json.get("errors").is_some() {
            anyhow::bail!("Linear API returned GraphQL errors: {}", response_text);
        }

        // Now deserialize the successful response
        match serde_json::from_str::<LinearResponse<T>>(&response_text) {
            Ok(parsed) => Ok(parsed),
            Err(e) => {
                // More detailed error information
                if verbose {
                    println!("DEBUG Deserialization error: {}", e);
                    println!("DEBUG Linear User struct: {:?}", std::any::type_name::<T>());
                }
                anyhow::bail!("Failed to deserialize Linear API response: {}", e)
            }
        }
    }
}
