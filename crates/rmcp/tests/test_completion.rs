use std::collections::HashMap;

use rmcp::{handler::server::completion::*, model::*};
use serde_json::json;

#[test]
fn test_completion_context_serialization() {
    let mut args = HashMap::new();
    args.insert("key1".to_string(), "value1".to_string());
    args.insert("key2".to_string(), "value2".to_string());

    let context = CompletionContext::with_arguments(args);

    // Test serialization
    let json = serde_json::to_value(&context).unwrap();
    let expected = json!({
        "arguments": {
            "key1": "value1",
            "key2": "value2"
        }
    });
    assert_eq!(json, expected);

    // Test deserialization
    let deserialized: CompletionContext = serde_json::from_value(expected).unwrap();
    assert_eq!(deserialized, context);
}

#[test]
fn test_completion_context_methods() {
    let mut args = HashMap::new();
    args.insert("city".to_string(), "San Francisco".to_string());
    args.insert("country".to_string(), "USA".to_string());

    let context = CompletionContext::with_arguments(args);

    assert!(context.has_arguments());
    assert_eq!(
        context.get_argument("city"),
        Some(&"San Francisco".to_string())
    );
    assert_eq!(context.get_argument("missing"), None);

    let names = context.argument_names();
    assert!(names.contains(&"city"));
    assert!(names.contains(&"country"));
    assert_eq!(names.len(), 2);
}

#[test]
fn test_complete_request_param_serialization() {
    let mut args = HashMap::new();
    args.insert("previous_input".to_string(), "test".to_string());

    let request = CompleteRequestParam {
        r#ref: Reference::for_prompt("weather_prompt"),
        argument: ArgumentInfo {
            name: "location".to_string(),
            value: "San".to_string(),
        },
        context: Some(CompletionContext::with_arguments(args)),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert!(json["ref"]["name"].as_str().unwrap() == "weather_prompt");
    assert!(json["argument"]["name"].as_str().unwrap() == "location");
    assert!(json["argument"]["value"].as_str().unwrap() == "San");
    assert!(
        json["context"]["arguments"]["previous_input"]
            .as_str()
            .unwrap()
            == "test"
    );
}

#[test]
fn test_completion_info_validation() {
    // Valid completion with less than max values
    let values = vec!["option1".to_string(), "option2".to_string()];
    let completion = CompletionInfo::new(values.clone()).unwrap();
    assert_eq!(completion.values, values);
    assert!(completion.validate().is_ok());

    // Test max values limit
    let many_values: Vec<String> = (0..=CompletionInfo::MAX_VALUES)
        .map(|i| format!("option_{}", i))
        .collect();

    let result = CompletionInfo::new(many_values);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Too many completion values"));
}

#[test]
fn test_completion_info_helper_methods() {
    let values = vec!["test1".to_string(), "test2".to_string()];

    // Test with_all_values
    let completion = CompletionInfo::with_all_values(values.clone()).unwrap();
    assert_eq!(completion.values, values);
    assert_eq!(completion.total, Some(2));
    assert_eq!(completion.has_more, Some(false));
    assert!(!completion.has_more_results());
    assert_eq!(completion.total_available(), Some(2));

    // Test with_pagination
    let completion = CompletionInfo::with_pagination(values.clone(), Some(10), true).unwrap();
    assert_eq!(completion.values, values);
    assert_eq!(completion.total, Some(10));
    assert_eq!(completion.has_more, Some(true));
    assert!(completion.has_more_results());
    assert_eq!(completion.total_available(), Some(10));
}

#[test]
fn test_reference_convenience_methods() {
    let prompt_ref = Reference::for_prompt("test_prompt");
    assert_eq!(prompt_ref.reference_type(), "ref/prompt");
    assert_eq!(prompt_ref.as_prompt_name(), Some("test_prompt"));
    assert_eq!(prompt_ref.as_resource_uri(), None);

    let resource_ref = Reference::for_resource_template("file://path/to/resource");
    assert_eq!(resource_ref.reference_type(), "ref/resource");
    assert_eq!(
        resource_ref.as_resource_uri(),
        Some("file://path/to/resource")
    );
    assert_eq!(resource_ref.as_prompt_name(), None);
}

#[tokio::test]
async fn test_default_completion_provider() {
    let provider = DefaultCompletionProvider::new();

    // Test prompt completion
    let result = provider
        .complete_prompt_argument("test_prompt", "arg", "ex", None)
        .await
        .unwrap();

    assert!(!result.values.is_empty());
    assert!(result.values.iter().any(|v| v.contains("example")));
    assert!(result.validate().is_ok());

    // Test resource completion
    let result = provider
        .complete_resource_argument("file://path", "filename", "file", None)
        .await
        .unwrap();

    assert!(!result.values.is_empty());
    assert!(result.validate().is_ok());
}

#[tokio::test]
async fn test_fuzzy_matching() {
    let provider = DefaultCompletionProvider::new();
    let candidates = vec![
        "hello_world".to_string(),
        "hello_rust".to_string(),
        "world_peace".to_string(),
        "rust_language".to_string(),
    ];

    // Test exact match
    let matches = provider.fuzzy_match("hello_world", &candidates);
    assert_eq!(matches[0], "hello_world");

    // Test prefix match
    let matches = provider.fuzzy_match("hello", &candidates);
    assert_eq!(matches.len(), 2);
    assert!(matches.contains(&"hello_world".to_string()));
    assert!(matches.contains(&"hello_rust".to_string()));

    // Test substring match
    let matches = provider.fuzzy_match("rust", &candidates);
    assert!(matches.contains(&"hello_rust".to_string()));
    assert!(matches.contains(&"rust_language".to_string()));

    // Test empty query
    let matches = provider.fuzzy_match("", &candidates);
    assert_eq!(
        matches.len(),
        candidates.len().min(provider.max_suggestions)
    );
}

#[tokio::test]
async fn test_completion_provider_with_context() {
    let provider = DefaultCompletionProvider::new();

    let mut args = HashMap::new();
    args.insert("country".to_string(), "USA".to_string());
    let context = CompletionContext::with_arguments(args);

    let result = provider
        .complete_prompt_argument("location_prompt", "city", "example", Some(&context))
        .await
        .unwrap();

    // Our default provider should return some suggestions even with context
    assert!(!result.values.is_empty());
    assert!(context.get_argument("country").is_some());
}

#[test]
fn test_deprecated_resource_reference() {
    // Test that deprecated ResourceReference still works
    #[allow(deprecated)]
    let resource_ref = ResourceReference {
        uri: "test://uri".to_string(),
    };

    // Should be the same as ResourceTemplateReference
    let template_ref = ResourceTemplateReference {
        uri: "test://uri".to_string(),
    };

    // They should be equivalent (since ResourceReference is an alias)
    assert_eq!(resource_ref.uri, template_ref.uri);
}

#[test]
fn test_complete_result_default() {
    let result = CompleteResult::default();
    assert!(result.completion.values.is_empty());
    assert_eq!(result.completion.total, None);
    assert_eq!(result.completion.has_more, None);
}

#[test]
fn test_mcp_schema_compliance() {
    // Test that our structures match MCP 2025-06-18 schema

    // CompleteRequest should have method, ref, argument, and optional context
    let request = CompleteRequestParam {
        r#ref: Reference::for_prompt("test"),
        argument: ArgumentInfo {
            name: "arg".to_string(),
            value: "val".to_string(),
        },
        context: None,
    };

    let json = serde_json::to_value(&request).unwrap();
    assert!(json.get("ref").is_some());
    assert!(json.get("argument").is_some());
    assert!(json.get("context").is_none()); // Should be omitted when None

    // CompleteResult should have completion with values, total, hasMore
    let result = CompleteResult {
        completion: CompletionInfo {
            values: vec!["test".to_string()],
            total: Some(1),
            has_more: Some(false),
        },
    };

    let json = serde_json::to_value(&result).unwrap();
    assert!(json["completion"]["values"].is_array());
    assert_eq!(json["completion"]["total"], 1);
    assert_eq!(json["completion"]["hasMore"], false); // camelCase in JSON
}

#[test]
fn test_completion_context_empty() {
    let context = CompletionContext::new();
    assert!(!context.has_arguments());
    assert_eq!(context.get_argument("any"), None);
    assert!(context.argument_names().is_empty());

    // Empty context should serialize without arguments field
    let json = serde_json::to_value(&context).unwrap();
    assert!(json.get("arguments").is_none());
}

#[test]
fn test_completion_serialization_format() {
    // Test that camelCase is used in JSON serialization
    let completion = CompletionInfo {
        values: vec!["test".to_string()],
        total: Some(5),
        has_more: Some(true),
    };

    let json = serde_json::to_value(&completion).unwrap();
    assert!(json.get("hasMore").is_some()); // Should be camelCase
    assert!(json.get("has_more").is_none()); // Should NOT be snake_case

    // Test deserialization from camelCase
    let json_str = r#"{"values":["test"],"total":5,"hasMore":true}"#;
    let parsed: CompletionInfo = serde_json::from_str(json_str).unwrap();
    assert_eq!(parsed.has_more, Some(true));
}

#[tokio::test]
async fn test_completion_edge_cases() {
    let provider = DefaultCompletionProvider::with_max_suggestions(2);

    // Test with max suggestions limit
    let candidates: Vec<String> = (0..10).map(|i| format!("option_{}", i)).collect();
    let matches = provider.fuzzy_match("option", &candidates);
    assert!(matches.len() <= 2);

    // Test with empty candidates
    let matches = provider.fuzzy_match("test", &[]);
    assert!(matches.is_empty());

    // Test with special characters
    let candidates = vec!["test-value".to_string(), "test_value".to_string()];
    let matches = provider.fuzzy_match("test", &candidates);
    assert_eq!(matches.len(), 2);
}

// Performance test - ensure completion doesn't take too long
#[tokio::test]
async fn test_completion_performance() {
    let provider = DefaultCompletionProvider::new();

    // Create a large set of candidates
    let candidates: Vec<String> = (0..1000).map(|i| format!("candidate_{:04}", i)).collect();

    let start = std::time::Instant::now();
    let _matches = provider.fuzzy_match("candidate_05", &candidates);
    let duration = start.elapsed();

    // Should complete within reasonable time (adjust threshold as needed)
    assert!(
        duration.as_millis() < 100,
        "Completion took too long: {:?}",
        duration
    );
}

#[test]
fn test_completion_info_bounds() {
    // Test exactly at the limit
    let values: Vec<String> = (0..CompletionInfo::MAX_VALUES)
        .map(|i| format!("value_{}", i))
        .collect();

    let completion = CompletionInfo::new(values).unwrap();
    assert_eq!(completion.values.len(), CompletionInfo::MAX_VALUES);
    assert!(completion.validate().is_ok());

    // Test exceeding the limit by 1
    let values: Vec<String> = (0..=CompletionInfo::MAX_VALUES)
        .map(|i| format!("value_{}", i))
        .collect();

    let result = CompletionInfo::new(values);
    assert!(result.is_err());
}
