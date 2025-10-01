//! Simple MCP Server with Elicitation
//!
//! Demonstrates user name collection via elicitation using low-level typed schema builder

use std::sync::Arc;

use anyhow::Result;
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars::JsonSchema,
    service::{RequestContext, RoleServer},
    tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::Deserialize;
use tokio::sync::Mutex;
use tracing_subscriber::{self, EnvFilter};

/// Simple tool request
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GreetRequest {
    pub greeting: String,
}

/// Simple server with elicitation
#[derive(Clone)]
pub struct ElicitationServer {
    user_name: Arc<Mutex<Option<String>>>,
    tool_router: ToolRouter<ElicitationServer>,
}

impl ElicitationServer {
    pub fn new() -> Self {
        Self {
            user_name: Arc::new(Mutex::new(None)),
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for ElicitationServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl ElicitationServer {
    #[tool(description = "Greet user with name collection")]
    async fn greet_user(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(request): Parameters<GreetRequest>,
    ) -> Result<CallToolResult, McpError> {
        // Check if we have user name
        let current_name = self.user_name.lock().await.clone();

        let user_name = if let Some(name) = current_name {
            name
        } else {
            // Request user name via typed elicitation schema
            let schema = ElicitationSchema::builder()
                .string(
                    "name",
                    StringPropertySchema::new()
                        .with_description("User's name")
                        .with_length_range(1, 100),
                )
                .required("name")
                .build();

            let request_param = CreateElicitationRequestParam {
                message: "Please provide your name".to_string(),
                requested_schema: schema.to_json_object(),
            };

            match context.peer.create_elicitation(request_param).await {
                Ok(result) if result.action == ElicitationAction::Accept => {
                    if let Some(content) = result.content {
                        if let Some(name_value) = content.get("name") {
                            if let Some(name) = name_value.as_str() {
                                let name = name.to_string();
                                *self.user_name.lock().await = Some(name.clone());
                                name
                            } else {
                                "Guest".to_string()
                            }
                        } else {
                            "Guest".to_string()
                        }
                    } else {
                        "Guest".to_string()
                    }
                }
                _ => "Unknown".to_string(),
            }
        };

        Ok(CallToolResult::success(vec![Content::text(format!(
            "{} {}!",
            request.greeting, user_name
        ))]))
    }

    #[tool(description = "Reset stored user name")]
    async fn reset_name(&self) -> Result<CallToolResult, McpError> {
        *self.user_name.lock().await = None;
        Ok(CallToolResult::success(vec![Content::text(
            "User name reset. Next greeting will ask for name again.".to_string(),
        )]))
    }
}

#[tool_handler]
impl ServerHandler for ElicitationServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "Simple server demonstrating elicitation for user name collection".to_string(),
            ),
            ..Default::default()
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    println!("Simple MCP Elicitation Demo");

    // Get current executable path for Inspector
    let current_exe = std::env::current_exe()
        .map(|path| path.display().to_string())
        .unwrap();

    println!("To test with MCP Inspector:");
    println!("1. Run: npx @modelcontextprotocol/inspector");
    println!("2. Enter server command: {}", current_exe);

    let service = ElicitationServer::new()
        .serve(stdio())
        .await
        .inspect_err(|e| {
            tracing::error!("serving error: {:?}", e);
        })?;

    service.waiting().await?;
    Ok(())
}
