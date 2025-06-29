// cargo test --features "server client" --package rmcp test_elicitation
#![cfg(feature = "mcp_spec-2025-06-18")]
mod common;

use std::sync::{Arc, Mutex};

use common::handlers::{TestClientHandler, TestServer};
use rmcp::{ServiceExt, model::*};
use serde_json::json;
use tokio::sync::Notify;

#[tokio::test]
async fn test_elicitation_spec_compliance() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);
    let receive_signal = Arc::new(Notify::new());
    let received_requests = Arc::new(Mutex::new(Vec::<CreateElicitationRequestParam>::new()));

    // Enhanced TestClientHandler to capture elicitation requests
    struct ElicitationTestClientHandler {
        inner: TestClientHandler,
        receive_signal: Arc<Notify>,
        received_requests: Arc<Mutex<Vec<CreateElicitationRequestParam>>>,
    }

    impl ElicitationTestClientHandler {
        fn new(
            receive_signal: Arc<Notify>,
            received_requests: Arc<Mutex<Vec<CreateElicitationRequestParam>>>,
        ) -> Self {
            Self {
                inner: TestClientHandler::with_notification(
                    false,
                    false,
                    receive_signal.clone(),
                    Arc::new(Mutex::new(Vec::new())),
                ),
                receive_signal,
                received_requests,
            }
        }
    }

    impl rmcp::handler::client::ClientHandler for ElicitationTestClientHandler {
        async fn create_elicitation(
            &self,
            params: CreateElicitationRequestParam,
            _context: rmcp::service::RequestContext<rmcp::service::RoleClient>,
        ) -> Result<CreateElicitationResult, rmcp::Error> {
            // Store the request for verification
            {
                let mut requests = self.received_requests.lock().unwrap();
                requests.push(params.clone());
            }

            // Signal that we received a request
            self.receive_signal.notify_one();

            // Simulate user accepting the elicitation with test data
            Ok(CreateElicitationResult {
                action: ElicitationAction::Accept,
                content: Some(json!({
                    "email": "test@example.com",
                    "age": 25,
                    "confirmed": true
                })),
            })
        }

        async fn ping(
            &self,
            context: rmcp::service::RequestContext<rmcp::service::RoleClient>,
        ) -> Result<(), rmcp::Error> {
            self.inner.ping(context).await
        }

        async fn create_message(
            &self,
            params: rmcp::model::CreateMessageRequestParam,
            context: rmcp::service::RequestContext<rmcp::service::RoleClient>,
        ) -> Result<rmcp::model::CreateMessageResult, rmcp::Error> {
            self.inner.create_message(params, context).await
        }

        async fn list_roots(
            &self,
            context: rmcp::service::RequestContext<rmcp::service::RoleClient>,
        ) -> Result<rmcp::model::ListRootsResult, rmcp::Error> {
            self.inner.list_roots(context).await
        }
    }

    // Start server in a separate task
    let server_handle = tokio::spawn(async move {
        let server = TestServer::new().serve(server_transport).await?;

        // Test server can send elicitation request
        let schema = json!({
            "type": "object",
            "properties": {
                "email": {
                    "type": "string",
                    "format": "email"
                },
                "age": {
                    "type": "integer",
                    "minimum": 18
                },
                "confirmed": {
                    "type": "boolean",
                    "default": false
                }
            },
            "required": ["email", "age"]
        });

        let result = server
            .peer()
            .create_elicitation(CreateElicitationRequestParam {
                message: "Please provide your contact information".to_string(),
                requested_schema: schema,
            })
            .await?;

        // Verify the response
        assert_eq!(result.action, ElicitationAction::Accept);
        assert!(result.content.is_some());

        let content = result.content.unwrap();
        assert_eq!(content["email"], "test@example.com");
        assert_eq!(content["age"], 25);
        assert_eq!(content["confirmed"], true);

        server.waiting().await?;
        anyhow::Ok(())
    });

    let client =
        ElicitationTestClientHandler::new(receive_signal.clone(), received_requests.clone())
            .serve(client_transport)
            .await?;

    // Wait for the elicitation request
    receive_signal.notified().await;

    // Verify the request was received correctly
    {
        let requests = received_requests.lock().unwrap();
        assert_eq!(
            requests.len(),
            1,
            "Should receive exactly one elicitation request"
        );

        let request = &requests[0];
        assert_eq!(request.message, "Please provide your contact information");

        // Verify the schema structure
        let schema = &request.requested_schema;
        assert!(schema["type"].as_str().unwrap() == "object");
        assert!(schema["properties"].is_object());
        assert!(schema["required"].is_array());
    }

    // Cancel the client before ending the test
    client.cancel().await?;

    // Wait for server to complete
    server_handle.await??;

    Ok(())
}

#[tokio::test]
async fn test_elicitation_action_types() -> anyhow::Result<()> {
    // Test different action types
    for (action_type, expected_action) in [
        ("accept", ElicitationAction::Accept),
        ("reject", ElicitationAction::Reject),
        ("cancel", ElicitationAction::Cancel),
    ] {
        let (server_transport, client_transport) = tokio::io::duplex(4096);
        let receive_signal = Arc::new(Notify::new());

        struct ActionTestClientHandler {
            action: ElicitationAction,
            receive_signal: Arc<Notify>,
        }

        impl rmcp::handler::client::ClientHandler for ActionTestClientHandler {
            async fn create_elicitation(
                &self,
                _params: CreateElicitationRequestParam,
                _context: rmcp::service::RequestContext<rmcp::service::RoleClient>,
            ) -> Result<CreateElicitationResult, rmcp::Error> {
                self.receive_signal.notify_one();

                Ok(CreateElicitationResult {
                    action: self.action.clone(),
                    content: match self.action {
                        ElicitationAction::Accept => Some(json!({"test": "data"})),
                        _ => None,
                    },
                })
            }

            async fn ping(
                &self,
                _context: rmcp::service::RequestContext<rmcp::service::RoleClient>,
            ) -> Result<(), rmcp::Error> {
                Ok(())
            }
        }

        let client_handler = ActionTestClientHandler {
            action: expected_action.clone(),
            receive_signal: receive_signal.clone(),
        };

        let server_handle = tokio::spawn(async move {
            let server = TestServer::new().serve(server_transport).await?;

            let result = server
                .peer()
                .create_elicitation(CreateElicitationRequestParam {
                    message: format!("Test {} action", action_type),
                    requested_schema: json!({"type": "object"}),
                })
                .await?;

            assert_eq!(result.action, expected_action);

            match expected_action {
                ElicitationAction::Accept => {
                    assert!(result.content.is_some());
                    assert_eq!(result.content.unwrap()["test"], "data");
                }
                _ => {
                    assert!(result.content.is_none());
                }
            }

            server.waiting().await?;
            anyhow::Ok(())
        });

        let client = client_handler.serve(client_transport).await?;
        receive_signal.notified().await;
        client.cancel().await?;
        server_handle.await??;
    }

    Ok(())
}

#[test]
fn test_elicitation_serialization() {
    // Test ElicitationAction serialization
    let test_cases = [
        (ElicitationAction::Accept, "accept"),
        (ElicitationAction::Reject, "reject"),
        (ElicitationAction::Cancel, "cancel"),
    ];

    for (action, expected) in test_cases {
        let serialized = serde_json::to_string(&action).unwrap();
        let serialized = serialized.trim_matches('"');
        assert_eq!(
            serialized, expected,
            "ElicitationAction::{:?} should serialize to \"{}\"",
            action, expected
        );

        // Test deserialization
        let deserialized: ElicitationAction =
            serde_json::from_str(&format!("\"{}\"", expected)).unwrap();
        assert_eq!(
            deserialized, action,
            "\"{}\" should deserialize to ElicitationAction::{:?}",
            expected, action
        );
    }

    // Test CreateElicitationRequestParam serialization
    let param = CreateElicitationRequestParam {
        message: "Test message".to_string(),
        requested_schema: json!({
            "type": "object",
            "properties": {
                "email": {"type": "string", "format": "email"}
            }
        }),
    };

    let serialized = serde_json::to_value(&param).unwrap();
    assert_eq!(serialized["message"], "Test message");
    assert_eq!(serialized["requestedSchema"]["type"], "object");

    // Test CreateElicitationResult serialization
    let result = CreateElicitationResult {
        action: ElicitationAction::Accept,
        content: Some(json!({"email": "test@example.com"})),
    };

    let serialized = serde_json::to_value(&result).unwrap();
    assert_eq!(serialized["action"], "accept");
    assert_eq!(serialized["content"]["email"], "test@example.com");

    // Test result without content
    let result_no_content = CreateElicitationResult {
        action: ElicitationAction::Cancel,
        content: None,
    };

    let serialized = serde_json::to_value(&result_no_content).unwrap();
    assert_eq!(serialized["action"], "cancel");
    assert!(
        serialized.get("content").is_none(),
        "content field should be omitted when None"
    );
}
