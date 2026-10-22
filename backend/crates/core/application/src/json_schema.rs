use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JsonSchemaError {
    #[error("invalid JSON Schema: {0}")]
    InvalidSchema(String),
    #[error("instance does not match JSON Schema: {0}")]
    Mismatch(String),
}

pub fn validate_definition(schema: &Value) -> Result<(), String> {
    validator_for(schema).map(|_| ())
}

pub fn validate_instance(schema: &Value, instance: &Value) -> Result<(), JsonSchemaError> {
    let validator = validator_for(schema).map_err(JsonSchemaError::InvalidSchema)?;
    let errors = validator
        .iter_errors(instance)
        .map(|error| format!("{} at {}", error, error.instance_path()))
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(JsonSchemaError::Mismatch(errors.join("; ")))
    }
}

fn validator_for(schema: &Value) -> Result<jsonschema::Validator, String> {
    jsonschema::validator_for(schema).map_err(|error| error.to_string())
}
