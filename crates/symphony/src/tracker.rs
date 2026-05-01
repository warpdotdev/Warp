//! Linear GraphQL client (spec §11).
//!
//! Direct HTTPS GraphQL — no MCP, no SDK. The query strings are isolated in
//! private functions so schema drift only requires updating one place.

use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use doppler::SecretValue;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Reference to another issue that blocks the holder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockerRef {
    /// Linear issue id of the blocker.
    pub id: String,
    /// Human-readable identifier of the blocker (e.g. `PDX-12`).
    pub identifier: String,
    /// Current state of the blocker, if known.
    pub state: Option<String>,
}

/// Normalized issue projection used by the orchestrator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    /// Linear internal id (uuid).
    pub id: String,
    /// Human-readable identifier (`TEAM-NN`).
    pub identifier: String,
    /// Issue title.
    pub title: String,
    /// Optional Markdown description.
    pub description: Option<String>,
    /// Numeric priority (Linear convention: 0 none, 1 urgent, … 4 low).
    pub priority: Option<i32>,
    /// State name (`Todo`, `In Progress`, …).
    pub state: String,
    /// Optional canonical URL to the issue in Linear.
    pub url: Option<String>,
    /// Lower-cased label names attached to the issue.
    pub labels: Vec<String>,
    /// Issues that block this one.
    pub blocked_by: Vec<BlockerRef>,
    /// Creation timestamp.
    pub created_at: Option<DateTime<Utc>>,
    /// Last updated timestamp.
    pub updated_at: Option<DateTime<Utc>>,
}

/// Errors raised by the tracker client.
#[derive(Debug, Error)]
pub enum TrackerError {
    /// Network / HTTP failure while talking to the tracker.
    #[error("http error: {0}")]
    Http(String),
    /// Tracker returned a structured GraphQL `errors` array.
    #[error("graphql error: {0}")]
    GraphQl(String),
    /// Tracker response could not be deserialized.
    #[error("decode error: {0}")]
    Decode(String),
}

impl From<reqwest::Error> for TrackerError {
    fn from(value: reqwest::Error) -> Self {
        TrackerError::Http(value.to_string())
    }
}

/// Trait abstracting the HTTP transport so tests can swap a mock in.
#[async_trait]
pub trait TrackerHttp: Send + Sync {
    /// POST a GraphQL query and return the raw JSON response.
    async fn post_graphql(
        &self,
        endpoint: &str,
        api_key: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, TrackerError>;
}

/// Default `reqwest`-backed transport.
pub struct ReqwestTransport {
    http: reqwest::Client,
}

impl ReqwestTransport {
    /// Construct a new transport with a sensible timeout.
    pub fn new() -> Result<Self, TrackerError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(TrackerError::from)?;
        Ok(Self { http })
    }
}

#[async_trait]
impl TrackerHttp for ReqwestTransport {
    async fn post_graphql(
        &self,
        endpoint: &str,
        api_key: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, TrackerError> {
        let resp = self
            .http
            .post(endpoint)
            .header("Authorization", api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;
        let status = resp.status();
        let value: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| TrackerError::Decode(e.to_string()))?;
        if !status.is_success() {
            return Err(TrackerError::Http(format!(
                "status {}: {}",
                status, value
            )));
        }
        Ok(value)
    }
}

/// Linear GraphQL client.
pub struct LinearClient {
    endpoint: String,
    api_key: SecretValue,
    project_slug: String,
    http: Box<dyn TrackerHttp>,
}

impl LinearClient {
    /// Construct a new client with the given parameters.
    pub fn new(
        endpoint: String,
        api_key: SecretValue,
        project_slug: String,
    ) -> Result<Self, TrackerError> {
        let transport = ReqwestTransport::new()?;
        Ok(Self::with_transport(
            endpoint,
            api_key,
            project_slug,
            Box::new(transport),
        ))
    }

    /// Construct with a caller-supplied transport (for tests).
    pub fn with_transport(
        endpoint: String,
        api_key: SecretValue,
        project_slug: String,
        http: Box<dyn TrackerHttp>,
    ) -> Self {
        Self {
            endpoint,
            api_key,
            project_slug,
            http,
        }
    }

    /// Fetch all issues whose state is in `active_states`, paginated 50 per page.
    pub async fn fetch_candidate_issues(
        &self,
        active_states: &[String],
    ) -> Result<Vec<Issue>, TrackerError> {
        let mut all: Vec<Issue> = Vec::new();
        let mut after: Option<String> = None;
        loop {
            let body = serde_json::json!({
                "query": candidate_query(),
                "variables": {
                    "first": 50,
                    "after": after,
                    "projectSlug": self.project_slug,
                    "states": active_states,
                }
            });
            let resp = self
                .http
                .post_graphql(&self.endpoint, self.api_key.expose(), body)
                .await?;
            check_graphql_errors(&resp)?;
            let nodes = resp
                .pointer("/data/issues/nodes")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for node in &nodes {
                if let Some(issue) = parse_issue(node) {
                    all.push(issue);
                }
            }
            let has_next = resp
                .pointer("/data/issues/pageInfo/hasNextPage")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let end_cursor = resp
                .pointer("/data/issues/pageInfo/endCursor")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if !has_next || end_cursor.is_none() {
                break;
            }
            after = end_cursor;
        }
        Ok(all)
    }

    /// Look up the current state for a set of issues by id.
    pub async fn fetch_issue_states_by_ids(
        &self,
        ids: &[String],
    ) -> Result<HashMap<String, String>, TrackerError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let body = serde_json::json!({
            "query": states_by_ids_query(),
            "variables": { "ids": ids }
        });
        let resp = self
            .http
            .post_graphql(&self.endpoint, self.api_key.expose(), body)
            .await?;
        check_graphql_errors(&resp)?;
        let nodes = resp
            .pointer("/data/issues/nodes")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut out = HashMap::new();
        for n in nodes {
            let id = n.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let state = n
                .pointer("/state/name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !id.is_empty() {
                out.insert(id, state);
            }
        }
        Ok(out)
    }

    /// Post a comment to a Linear issue. Used by the orchestrator's
    /// post-run handler to write back agent completion / failure summaries
    /// without requiring an agent-side `linear_graphql` tool.
    ///
    /// Spec §11.5 boundary: Symphony writes only when configured to (via
    /// `WORKFLOW.md`'s `agent.comment_on_completion`). State transitions
    /// remain agent-driven (or manual) per spec.
    pub async fn add_comment(
        &self,
        issue_id: &str,
        body: &str,
    ) -> Result<(), TrackerError> {
        let payload = serde_json::json!({
            "query": comment_create_mutation(),
            "variables": {
                "input": {
                    "issueId": issue_id,
                    "body": body,
                }
            }
        });
        let resp = self
            .http
            .post_graphql(&self.endpoint, self.api_key.expose(), payload)
            .await?;
        check_graphql_errors(&resp)?;
        // Linear's commentCreate returns { success: bool, comment: { ... } }.
        let success = resp
            .pointer("/data/commentCreate/success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !success {
            return Err(TrackerError::GraphQl(
                "commentCreate returned success=false".to_string(),
            ));
        }
        Ok(())
    }
}

fn check_graphql_errors(resp: &serde_json::Value) -> Result<(), TrackerError> {
    if let Some(errors) = resp.get("errors").and_then(|v| v.as_array()) {
        if !errors.is_empty() {
            return Err(TrackerError::GraphQl(
                serde_json::to_string(errors).unwrap_or_default(),
            ));
        }
    }
    Ok(())
}

/// GraphQL query string for paginating candidate issues.
fn candidate_query() -> &'static str {
    r#"query Candidates($first: Int!, $after: String, $projectSlug: String!, $states: [String!]!) {
  issues(
    first: $first
    after: $after
    filter: {
      project: { slugId: { eq: $projectSlug } }
      state: { name: { in: $states } }
    }
  ) {
    pageInfo { hasNextPage endCursor }
    nodes {
      id
      identifier
      title
      description
      priority
      url
      createdAt
      updatedAt
      state { name }
      labels { nodes { name } }
      relations {
        nodes {
          type
          relatedIssue { id identifier state { name } }
        }
      }
      inverseRelations {
        nodes {
          type
          issue { id identifier state { name } }
        }
      }
    }
  }
}
"#
}

/// GraphQL mutation for posting a comment on an issue.
fn comment_create_mutation() -> &'static str {
    r#"mutation Comment($input: CommentCreateInput!) {
  commentCreate(input: $input) {
    success
    comment { id }
  }
}
"#
}

/// GraphQL query for fetching a flat `id → state.name` mapping.
fn states_by_ids_query() -> &'static str {
    r#"query StatesByIds($ids: [ID!]!) {
  issues(filter: { id: { in: $ids } }) {
    nodes { id state { name } }
  }
}
"#
}

/// Parse one issue node from the GraphQL response.
fn parse_issue(v: &serde_json::Value) -> Option<Issue> {
    let id = v.get("id")?.as_str()?.to_string();
    let identifier = v.get("identifier")?.as_str()?.to_string();
    let title = v.get("title")?.as_str()?.to_string();
    let description = v
        .get("description")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    let priority = v.get("priority").and_then(|x| x.as_i64()).map(|n| n as i32);
    let url = v.get("url").and_then(|x| x.as_str()).map(|s| s.to_string());
    let state = v
        .pointer("/state/name")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let labels: Vec<String> = v
        .pointer("/labels/nodes")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|n| n.get("name").and_then(|s| s.as_str()).map(|s| s.to_lowercase()))
                .collect()
        })
        .unwrap_or_default();
    // Build blocked_by from inverseRelations where type == "blocks"
    // (this issue is the target of someone else's "blocks" relation).
    // Filter client-side because Linear's schema no longer accepts a
    // `filter` arg on Issue.relations / inverseRelations.
    let blocked_by: Vec<BlockerRef> = v
        .pointer("/inverseRelations/nodes")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|n| {
                    n.get("type").and_then(|t| t.as_str()) == Some("blocks")
                })
                .filter_map(|n| {
                    let other = n.get("issue")?;
                    Some(BlockerRef {
                        id: other.get("id")?.as_str()?.to_string(),
                        identifier: other.get("identifier")?.as_str()?.to_string(),
                        state: other
                            .pointer("/state/name")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string()),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let created_at = v
        .get("createdAt")
        .and_then(|x| x.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));
    let updated_at = v
        .get("updatedAt")
        .and_then(|x| x.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    Some(Issue {
        id,
        identifier,
        title,
        description,
        priority,
        state,
        url,
        labels,
        blocked_by,
        created_at,
        updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_issue_minimal() {
        let v = serde_json::json!({
            "id": "abc-123",
            "identifier": "PDX-9",
            "title": "Do the thing",
            "description": null,
            "priority": 2,
            "url": "https://linear.app/x/issue/PDX-9",
            "state": { "name": "Todo" },
            "labels": { "nodes": [{ "name": "Agent:Claude" }, { "name": "bug" }] },
            "relations": { "nodes": [] },
            "createdAt": "2026-01-01T00:00:00Z",
            "updatedAt": "2026-01-02T00:00:00Z"
        });
        let issue = parse_issue(&v).unwrap();
        assert_eq!(issue.identifier, "PDX-9");
        assert_eq!(issue.state, "Todo");
        assert_eq!(issue.labels, vec!["agent:claude", "bug"]);
        assert_eq!(issue.priority, Some(2));
        assert!(issue.created_at.is_some());
    }
}
