//! Typed JSON Schema definitions for MCP 2025-06-18 elicitation
//!
//! This module provides strongly-typed structures for creating elicitation
//! request schemas that comply with the MCP 2025-06-18 specification.
//!
//! ## Overview
//!
//! The MCP specification requires elicitation schemas to be flat objects with primitive
//! property types only. This module provides types that enforce these constraints at
//! compile time while maintaining zero-cost abstraction principles.
//!
//! ## Supported Property Types
//!
//! - **String**: Text input with optional length constraints and format validation
//! - **Number/Integer**: Numeric input with optional min/max constraints
//! - **Boolean**: True/false input with optional default value
//! - **Enum**: Selection from predefined string values
//!
//! ## Example Usage
//!
//! ```rust
//! use rmcp::model::{ElicitationSchema, StringPropertySchema, StringFormat};
//!
//! let schema = ElicitationSchema::builder()
//!     .string(
//!         "email",
//!         StringPropertySchema::new()
//!             .with_format(StringFormat::Email)
//!             .with_description("Your email address")
//!     )
//!     .required("email")
//!     .build();
//!
//! // Convert to JsonObject for use with CreateElicitationRequestParam
//! let json_object = schema.to_json_object();
//! ```

use std::{borrow::Cow, collections::HashMap};

use serde::{Deserialize, Serialize};

use crate::{const_string, model::ConstString};

// =============================================================================
// TYPE CONSTANTS
// =============================================================================

const_string!(StringType = "string");
const_string!(BooleanType = "boolean");
const_string!(ObjectType = "object");

// =============================================================================
// FORMAT TYPES
// =============================================================================

/// Format types for string property validation (MCP 2025-06-18)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum StringFormat {
    /// Email address format (RFC 5322)
    Email,
    /// URI format (RFC 3986)
    Uri,
    /// Date format (RFC 3339)
    Date,
    /// Date-time format (RFC 3339)
    #[serde(rename = "date-time")]
    DateTime,
}

/// Number type variants for numeric properties
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum NumberType {
    /// Floating point number
    Number,
    /// Integer number
    Integer,
}

// =============================================================================
// PROPERTY SCHEMAS
// =============================================================================

/// Schema definition for string properties
///
/// Supports optional length constraints and format validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct StringPropertySchema {
    #[serde(rename = "type")]
    pub schema_type: StringType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Cow<'static, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Cow<'static, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<StringFormat>,
}

/// Schema definition for numeric properties
///
/// Supports both integer and floating-point numbers with optional range constraints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct NumberPropertySchema {
    #[serde(rename = "type")]
    pub schema_type: NumberType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Cow<'static, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Cow<'static, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
}

/// Schema definition for boolean properties
///
/// Supports optional default value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct BooleanPropertySchema {
    #[serde(rename = "type")]
    pub schema_type: BooleanType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Cow<'static, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Cow<'static, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<bool>,
}

/// Schema definition for enum properties (string-based selection)
///
/// Supports optional display names for enum values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct EnumPropertySchema {
    #[serde(rename = "type")]
    pub schema_type: StringType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Cow<'static, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Cow<'static, str>>,
    #[serde(rename = "enum")]
    pub values: Vec<Cow<'static, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_names: Option<Vec<Cow<'static, str>>>,
}

/// Union of all primitive property schema types
///
/// This enum uses untagged serialization to ensure clean JSON output
/// that matches the MCP specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum PropertySchema {
    String(StringPropertySchema),
    Number(NumberPropertySchema),
    Boolean(BooleanPropertySchema),
    Enum(EnumPropertySchema),
}

// =============================================================================
// COMPLETE SCHEMA
// =============================================================================

/// Complete elicitation request schema (MCP 2025-06-18 compliant)
///
/// This represents a flat object schema with primitive-typed properties only.
/// Use the builder API to construct schemas in a type-safe manner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ElicitationSchema {
    #[serde(rename = "type")]
    pub schema_type: ObjectType,
    pub properties: HashMap<Cow<'static, str>, PropertySchema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<Cow<'static, str>>>,
}

// =============================================================================
// IMPLEMENTATIONS
// =============================================================================

impl StringPropertySchema {
    /// Create a new string property schema with default values
    #[inline]
    pub const fn new() -> Self {
        Self {
            schema_type: StringType,
            title: None,
            description: None,
            min_length: None,
            max_length: None,
            format: None,
        }
    }

    /// Set the title for this property
    #[inline]
    pub fn with_title(mut self, title: impl Into<Cow<'static, str>>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the description for this property
    #[inline]
    pub fn with_description(mut self, description: impl Into<Cow<'static, str>>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the minimum length constraint
    #[inline]
    pub const fn with_min_length(mut self, min: u32) -> Self {
        self.min_length = Some(min);
        self
    }

    /// Set the maximum length constraint
    #[inline]
    pub const fn with_max_length(mut self, max: u32) -> Self {
        self.max_length = Some(max);
        self
    }

    /// Set both minimum and maximum length constraints
    #[inline]
    pub const fn with_length_range(mut self, min: u32, max: u32) -> Self {
        self.min_length = Some(min);
        self.max_length = Some(max);
        self
    }

    /// Set the format constraint
    #[inline]
    pub const fn with_format(mut self, format: StringFormat) -> Self {
        self.format = Some(format);
        self
    }
}

impl NumberPropertySchema {
    /// Create a new number property schema
    #[inline]
    pub const fn new(schema_type: NumberType) -> Self {
        Self {
            schema_type,
            title: None,
            description: None,
            minimum: None,
            maximum: None,
        }
    }

    /// Create an integer property schema
    #[inline]
    pub const fn integer() -> Self {
        Self::new(NumberType::Integer)
    }

    /// Create a floating-point number property schema
    #[inline]
    pub const fn number() -> Self {
        Self::new(NumberType::Number)
    }

    /// Set the title for this property
    #[inline]
    pub fn with_title(mut self, title: impl Into<Cow<'static, str>>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the description for this property
    #[inline]
    pub fn with_description(mut self, description: impl Into<Cow<'static, str>>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the minimum value constraint
    #[inline]
    pub const fn with_minimum(mut self, min: f64) -> Self {
        self.minimum = Some(min);
        self
    }

    /// Set the maximum value constraint
    #[inline]
    pub const fn with_maximum(mut self, max: f64) -> Self {
        self.maximum = Some(max);
        self
    }

    /// Set both minimum and maximum value constraints
    #[inline]
    pub const fn with_range(mut self, min: f64, max: f64) -> Self {
        self.minimum = Some(min);
        self.maximum = Some(max);
        self
    }
}

impl BooleanPropertySchema {
    /// Create a new boolean property schema with default values
    #[inline]
    pub const fn new() -> Self {
        Self {
            schema_type: BooleanType,
            title: None,
            description: None,
            default: None,
        }
    }

    /// Set the title for this property
    #[inline]
    pub fn with_title(mut self, title: impl Into<Cow<'static, str>>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the description for this property
    #[inline]
    pub fn with_description(mut self, description: impl Into<Cow<'static, str>>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the default value
    #[inline]
    pub const fn with_default(mut self, default: bool) -> Self {
        self.default = Some(default);
        self
    }
}

impl EnumPropertySchema {
    /// Create a new enum property schema with the given values
    #[inline]
    pub fn new(values: Vec<Cow<'static, str>>) -> Self {
        Self {
            schema_type: StringType,
            title: None,
            description: None,
            values,
            enum_names: None,
        }
    }

    /// Set the title for this property
    #[inline]
    pub fn with_title(mut self, title: impl Into<Cow<'static, str>>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the description for this property
    #[inline]
    pub fn with_description(mut self, description: impl Into<Cow<'static, str>>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set display names for enum values
    #[inline]
    pub fn with_names(mut self, names: Vec<Cow<'static, str>>) -> Self {
        self.enum_names = Some(names);
        self
    }
}

impl Default for StringPropertySchema {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Default for BooleanPropertySchema {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl ElicitationSchema {
    /// Create a new schema builder
    #[inline]
    pub fn builder() -> ElicitationSchemaBuilder {
        ElicitationSchemaBuilder::new()
    }

    /// Convert to generic JsonObject for use with CreateElicitationRequestParam
    ///
    /// This method is infallible as ElicitationSchema is guaranteed to serialize
    /// to a valid JSON object.
    pub fn to_json_object(&self) -> crate::model::JsonObject {
        match serde_json::to_value(self) {
            Ok(serde_json::Value::Object(obj)) => obj,
            _ => unreachable!("ElicitationSchema always serializes to an object"),
        }
    }
}

// =============================================================================
// BUILDER
// =============================================================================

/// Builder for constructing ElicitationSchema instances
///
/// Provides a fluent API for building schemas in a type-safe manner.
#[derive(Debug, Clone, Default)]
pub struct ElicitationSchemaBuilder {
    properties: HashMap<Cow<'static, str>, PropertySchema>,
    required: Vec<Cow<'static, str>>,
}

impl ElicitationSchemaBuilder {
    /// Create a new schema builder
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a string property to the schema
    pub fn string(mut self, name: impl Into<Cow<'static, str>>, schema: StringPropertySchema) -> Self {
        self.properties.insert(name.into(), PropertySchema::String(schema));
        self
    }

    /// Add a number property to the schema
    pub fn number(mut self, name: impl Into<Cow<'static, str>>, schema: NumberPropertySchema) -> Self {
        self.properties.insert(name.into(), PropertySchema::Number(schema));
        self
    }

    /// Add a boolean property to the schema
    pub fn boolean(mut self, name: impl Into<Cow<'static, str>>, schema: BooleanPropertySchema) -> Self {
        self.properties.insert(name.into(), PropertySchema::Boolean(schema));
        self
    }

    /// Add an enum property to the schema
    pub fn enumeration(mut self, name: impl Into<Cow<'static, str>>, schema: EnumPropertySchema) -> Self {
        self.properties.insert(name.into(), PropertySchema::Enum(schema));
        self
    }

    /// Mark a field as required
    pub fn required(mut self, field: impl Into<Cow<'static, str>>) -> Self {
        self.required.push(field.into());
        self
    }

    /// Build the final ElicitationSchema
    pub fn build(self) -> ElicitationSchema {
        ElicitationSchema {
            schema_type: ObjectType,
            properties: self.properties,
            required: if self.required.is_empty() {
                None
            } else {
                Some(self.required)
            },
        }
    }
}
