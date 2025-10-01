//! Tests for typed elicitation schema (MCP 2025-06-18)

use rmcp::model::*;
use serde_json::json;

// =============================================================================
// STRING SCHEMA TESTS
// =============================================================================

#[test]
fn test_string_schema_basic() {
    let schema = StringPropertySchema::new().with_description("User's name");

    let json = serde_json::to_value(&schema).unwrap();
    assert_eq!(json["type"], "string");
    assert_eq!(json["description"], "User's name");
    assert!(json.get("title").is_none());
    assert!(json.get("minLength").is_none());
}

#[test]
fn test_string_schema_with_format() {
    let schema = StringPropertySchema::new()
        .with_format(StringFormat::Email)
        .with_description("Email address");

    let json = serde_json::to_value(&schema).unwrap();
    assert_eq!(json["type"], "string");
    assert_eq!(json["format"], "email");
    assert_eq!(json["description"], "Email address");
}

#[test]
fn test_string_schema_all_formats() {
    let formats = vec![
        (StringFormat::Email, "email"),
        (StringFormat::Uri, "uri"),
        (StringFormat::Date, "date"),
        (StringFormat::DateTime, "date-time"),
    ];

    for (format, expected) in formats {
        let schema = StringPropertySchema::new().with_format(format);
        let json = serde_json::to_value(&schema).unwrap();
        assert_eq!(json["format"], expected);
    }
}

#[test]
fn test_string_schema_with_length_constraints() {
    let schema = StringPropertySchema::new()
        .with_min_length(3)
        .with_max_length(50);

    let json = serde_json::to_value(&schema).unwrap();
    assert_eq!(json["minLength"], 3);
    assert_eq!(json["maxLength"], 50);
}

#[test]
fn test_string_schema_with_length_range() {
    let schema = StringPropertySchema::new().with_length_range(1, 100);

    let json = serde_json::to_value(&schema).unwrap();
    assert_eq!(json["minLength"], 1);
    assert_eq!(json["maxLength"], 100);
}

// =============================================================================
// NUMBER SCHEMA TESTS
// =============================================================================

#[test]
fn test_number_schema_integer() {
    let schema = NumberPropertySchema::integer()
        .with_description("Age")
        .with_range(18.0, 120.0);

    let json = serde_json::to_value(&schema).unwrap();
    assert_eq!(json["type"], "integer");
    assert_eq!(json["description"], "Age");
    assert_eq!(json["minimum"], 18.0);
    assert_eq!(json["maximum"], 120.0);
}

#[test]
fn test_number_schema_number() {
    let schema = NumberPropertySchema::number()
        .with_description("Temperature")
        .with_range(-273.15, 1000.0);

    let json = serde_json::to_value(&schema).unwrap();
    assert_eq!(json["type"], "number");
    assert_eq!(json["minimum"], -273.15);
    assert_eq!(json["maximum"], 1000.0);
}

#[test]
fn test_number_schema_only_minimum() {
    let schema = NumberPropertySchema::integer().with_minimum(0.0);

    let json = serde_json::to_value(&schema).unwrap();
    assert_eq!(json["minimum"], 0.0);
    assert!(json.get("maximum").is_none());
}

#[test]
fn test_number_schema_only_maximum() {
    let schema = NumberPropertySchema::number().with_maximum(100.0);

    let json = serde_json::to_value(&schema).unwrap();
    assert_eq!(json["maximum"], 100.0);
    assert!(json.get("minimum").is_none());
}

// =============================================================================
// BOOLEAN SCHEMA TESTS
// =============================================================================

#[test]
fn test_boolean_schema_basic() {
    let schema = BooleanPropertySchema::new().with_description("Accept terms");

    let json = serde_json::to_value(&schema).unwrap();
    assert_eq!(json["type"], "boolean");
    assert_eq!(json["description"], "Accept terms");
    assert!(json.get("default").is_none());
}

#[test]
fn test_boolean_schema_with_default() {
    let schema = BooleanPropertySchema::new()
        .with_description("Subscribe to newsletter")
        .with_default(false);

    let json = serde_json::to_value(&schema).unwrap();
    assert_eq!(json["type"], "boolean");
    assert_eq!(json["default"], false);
}

// =============================================================================
// ENUM SCHEMA TESTS
// =============================================================================

#[test]
fn test_enum_schema_basic() {
    let schema = EnumPropertySchema::new(vec![
        "red".into(),
        "green".into(),
        "blue".into(),
    ])
    .with_description("Favorite color");

    let json = serde_json::to_value(&schema).unwrap();
    assert_eq!(json["type"], "string");
    assert_eq!(json["enum"], json!(["red", "green", "blue"]));
    assert_eq!(json["description"], "Favorite color");
    assert!(json.get("enumNames").is_none());
}

#[test]
fn test_enum_schema_with_names() {
    let schema = EnumPropertySchema::new(vec![
        "low".into(),
        "medium".into(),
        "high".into(),
    ])
    .with_names(vec![
        "Low Priority".into(),
        "Medium Priority".into(),
        "High Priority".into(),
    ]);

    let json = serde_json::to_value(&schema).unwrap();
    assert_eq!(json["type"], "string");
    assert_eq!(json["enum"], json!(["low", "medium", "high"]));
    assert_eq!(
        json["enumNames"],
        json!(["Low Priority", "Medium Priority", "High Priority"])
    );
}

// =============================================================================
// PROPERTY SCHEMA UNION TESTS
// =============================================================================

#[test]
fn test_property_schema_untagged_serialization() {
    // Test that PropertySchema serializes without tags
    let string_prop = PropertySchema::String(StringPropertySchema::new());
    let json = serde_json::to_value(&string_prop).unwrap();

    // Should be flat object, not {"String": {...}}
    assert!(json.is_object());
    assert_eq!(json["type"], "string");
    assert!(json.get("String").is_none());
}

#[test]
fn test_property_schema_all_variants() {
    let schemas = vec![
        (
            PropertySchema::String(StringPropertySchema::new()),
            "string",
        ),
        (
            PropertySchema::Number(NumberPropertySchema::integer()),
            "integer",
        ),
        (
            PropertySchema::Boolean(BooleanPropertySchema::new()),
            "boolean",
        ),
        (
            PropertySchema::Enum(EnumPropertySchema::new(vec!["a".into(), "b".into()])),
            "string", // Enum type is string
        ),
    ];

    for (schema, expected_type) in schemas {
        let json = serde_json::to_value(&schema).unwrap();
        assert_eq!(json["type"], expected_type);
    }
}

// =============================================================================
// COMPLETE ELICITATION SCHEMA TESTS
// =============================================================================

#[test]
fn test_elicitation_schema_simple() {
    let schema = ElicitationSchema::builder()
        .string("name", StringPropertySchema::new())
        .required("name")
        .build();

    let json = serde_json::to_value(&schema).unwrap();

    assert_eq!(json["type"], "object");
    assert!(json["properties"].is_object());
    assert_eq!(json["properties"]["name"]["type"], "string");
    assert_eq!(json["required"], json!(["name"]));
}

#[test]
fn test_elicitation_schema_complex() {
    let schema = ElicitationSchema::builder()
        .string(
            "email",
            StringPropertySchema::new()
                .with_format(StringFormat::Email)
                .with_description("Your email address"),
        )
        .number(
            "age",
            NumberPropertySchema::integer()
                .with_range(18.0, 120.0)
                .with_description("Your age"),
        )
        .boolean(
            "newsletter",
            BooleanPropertySchema::new()
                .with_description("Subscribe to newsletter")
                .with_default(true),
        )
        .enumeration(
            "country",
            EnumPropertySchema::new(vec!["us".into(), "uk".into(), "ca".into()])
                .with_names(vec![
                    "United States".into(),
                    "United Kingdom".into(),
                    "Canada".into(),
                ]),
        )
        .required("email")
        .required("age")
        .build();

    let json = serde_json::to_value(&schema).unwrap();

    assert_eq!(json["type"], "object");
    assert_eq!(json["properties"]["email"]["type"], "string");
    assert_eq!(json["properties"]["email"]["format"], "email");
    assert_eq!(json["properties"]["age"]["type"], "integer");
    assert_eq!(json["properties"]["age"]["minimum"], 18.0);
    assert_eq!(json["properties"]["newsletter"]["type"], "boolean");
    assert_eq!(json["properties"]["newsletter"]["default"], true);
    assert_eq!(json["properties"]["country"]["type"], "string");
    assert_eq!(json["properties"]["country"]["enum"], json!(["us", "uk", "ca"]));
    assert_eq!(json["required"], json!(["email", "age"]));
}

#[test]
fn test_elicitation_schema_no_required_fields() {
    let schema = ElicitationSchema::builder()
        .string("name", StringPropertySchema::new())
        .build();

    let json = serde_json::to_value(&schema).unwrap();

    // required field should not be serialized when empty
    assert!(json.get("required").is_none() || json["required"].is_null());
}

// =============================================================================
// TO_JSON_OBJECT TESTS
// =============================================================================

#[test]
fn test_to_json_object_conversion() {
    let schema = ElicitationSchema::builder()
        .string("name", StringPropertySchema::new())
        .required("name")
        .build();

    let json_object = schema.to_json_object();

    // Should be a valid JsonObject (serde_json::Map)
    assert!(json_object.contains_key("type"));
    assert!(json_object.contains_key("properties"));
    assert!(json_object.contains_key("required"));

    // Can be used with CreateElicitationRequestParam
    let param = CreateElicitationRequestParam {
        message: "Enter your name".to_string(),
        requested_schema: json_object,
    };

    let json = serde_json::to_value(&param).unwrap();
    assert_eq!(json["message"], "Enter your name");
    assert_eq!(json["requestedSchema"]["type"], "object");
    assert_eq!(json["requestedSchema"]["properties"]["name"]["type"], "string");
}

// =============================================================================
// ROUNDTRIP SERIALIZATION TESTS
// =============================================================================

#[test]
fn test_string_schema_roundtrip() {
    let original = StringPropertySchema::new()
        .with_format(StringFormat::Email)
        .with_length_range(5, 100);

    let json = serde_json::to_value(&original).unwrap();
    let deserialized: StringPropertySchema = serde_json::from_value(json).unwrap();

    assert_eq!(original, deserialized);
}

#[test]
fn test_number_schema_roundtrip() {
    let original = NumberPropertySchema::integer().with_range(0.0, 100.0);

    let json = serde_json::to_value(&original).unwrap();
    let deserialized: NumberPropertySchema = serde_json::from_value(json).unwrap();

    assert_eq!(original, deserialized);
}

#[test]
fn test_boolean_schema_roundtrip() {
    let original = BooleanPropertySchema::new().with_default(true);

    let json = serde_json::to_value(&original).unwrap();
    let deserialized: BooleanPropertySchema = serde_json::from_value(json).unwrap();

    assert_eq!(original, deserialized);
}

#[test]
fn test_enum_schema_roundtrip() {
    let original =
        EnumPropertySchema::new(vec!["a".into(), "b".into(), "c".into()])
            .with_names(vec!["A".into(), "B".into(), "C".into()]);

    let json = serde_json::to_value(&original).unwrap();
    let deserialized: EnumPropertySchema = serde_json::from_value(json).unwrap();

    assert_eq!(original, deserialized);
}

#[test]
fn test_elicitation_schema_roundtrip() {
    let original = ElicitationSchema::builder()
        .string(
            "email",
            StringPropertySchema::new().with_format(StringFormat::Email),
        )
        .number(
            "age",
            NumberPropertySchema::integer().with_range(18.0, 120.0),
        )
        .required("email")
        .build();

    let json = serde_json::to_value(&original).unwrap();
    let deserialized: ElicitationSchema = serde_json::from_value(json).unwrap();

    assert_eq!(original, deserialized);
}

// =============================================================================
// COW<'STATIC, STR> OPTIMIZATION TESTS
// =============================================================================

#[test]
fn test_cow_static_string_no_allocation() {
    // Static strings should not allocate
    let schema = StringPropertySchema::new().with_description("static description");

    let json = serde_json::to_value(&schema).unwrap();
    assert_eq!(json["description"], "static description");
}

#[test]
fn test_cow_owned_string() {
    // Dynamic strings should work too
    let dynamic_desc = format!("dynamic {}", "description");
    let schema = StringPropertySchema::new().with_description(dynamic_desc);

    let json = serde_json::to_value(&schema).unwrap();
    assert_eq!(json["description"], "dynamic description");
}

// =============================================================================
// BUILDER PATTERN TESTS
// =============================================================================

#[test]
fn test_builder_method_chaining() {
    let schema = ElicitationSchema::builder()
        .string("field1", StringPropertySchema::new())
        .number("field2", NumberPropertySchema::integer())
        .boolean("field3", BooleanPropertySchema::new())
        .required("field1")
        .required("field2")
        .build();

    assert_eq!(schema.properties.len(), 3);
    assert_eq!(schema.required.as_ref().unwrap().len(), 2);
}

#[test]
fn test_builder_can_be_reused() {
    let base_builder = ElicitationSchema::builder()
        .string("name", StringPropertySchema::new());

    let schema1 = base_builder.clone()
        .required("name")
        .build();

    let schema2 = base_builder
        .number("age", NumberPropertySchema::integer())
        .build();

    assert_eq!(schema1.properties.len(), 1);
    assert!(schema1.required.is_some());

    assert_eq!(schema2.properties.len(), 2);
    assert!(schema2.required.is_none());
}

// =============================================================================
// MCP SPEC COMPLIANCE TESTS
// =============================================================================

#[test]
fn test_mcp_spec_object_type_literal() {
    let schema = ElicitationSchema::builder().build();
    let json = serde_json::to_value(&schema).unwrap();

    // MCP spec requires type to be literal "object"
    assert_eq!(json["type"], "object");
}

#[test]
fn test_mcp_spec_camel_case_fields() {
    let schema = StringPropertySchema::new()
        .with_min_length(1)
        .with_max_length(100);

    let json = serde_json::to_value(&schema).unwrap();

    // MCP spec uses camelCase
    assert!(json.get("minLength").is_some());
    assert!(json.get("maxLength").is_some());
    assert!(json.get("min_length").is_none());
    assert!(json.get("max_length").is_none());
}

#[test]
fn test_mcp_spec_enum_field_name() {
    let schema = EnumPropertySchema::new(vec!["a".into(), "b".into()]);
    let json = serde_json::to_value(&schema).unwrap();

    // MCP spec uses "enum" not "values"
    assert!(json.get("enum").is_some());
    assert!(json.get("values").is_none());
}
