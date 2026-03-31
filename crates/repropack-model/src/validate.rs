use serde_json::Value;
use std::fmt;
use std::sync::OnceLock;

static MANIFEST_SCHEMA_STR: &str = include_str!("../../../schema/manifest-v1.schema.json");
static RECEIPT_SCHEMA_STR: &str = include_str!("../../../schema/receipt-v1.schema.json");

/// Error returned when JSON fails schema validation.
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "validation error at {}: {}", self.path, self.message)
    }
}

impl std::error::Error for ValidationError {}

fn manifest_schema() -> &'static Value {
    static SCHEMA: OnceLock<Value> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        serde_json::from_str(MANIFEST_SCHEMA_STR).expect("invalid manifest schema JSON")
    })
}

fn receipt_schema() -> &'static Value {
    static SCHEMA: OnceLock<Value> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        serde_json::from_str(RECEIPT_SCHEMA_STR).expect("invalid receipt schema JSON")
    })
}

/// Validate a JSON value against the manifest schema.
pub fn validate_manifest(json: &Value) -> Result<(), ValidationError> {
    validate_against(manifest_schema(), json)
}

/// Validate a JSON value against the receipt schema.
pub fn validate_receipt(json: &Value) -> Result<(), ValidationError> {
    validate_against(receipt_schema(), json)
}

fn validate_against(schema: &Value, instance: &Value) -> Result<(), ValidationError> {
    let validator = jsonschema::validator_for(schema).expect("failed to compile JSON schema");
    let mut errors = validator.iter_errors(instance);
    if let Some(error) = errors.next() {
        return Err(ValidationError {
            path: error.instance_path.to_string(),
            message: error.to_string(),
        });
    }
    Ok(())
}
