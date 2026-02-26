// Jaskier Shared Pattern -- OpenAPI schema contract test
// GeminiHydra v15 - Validates the generated OpenAPI schema

use utoipa::OpenApi;

#[test]
fn openapi_schema_is_valid_json() {
    let schema = serde_json::to_string_pretty(&geminihydra_backend::ApiDoc::openapi())
        .expect("OpenAPI schema should serialize to JSON");
    assert!(!schema.is_empty(), "Schema should not be empty");
}

#[test]
fn openapi_schema_contains_required_fields() {
    let schema = serde_json::to_string_pretty(&geminihydra_backend::ApiDoc::openapi())
        .expect("OpenAPI schema should serialize to JSON");
    assert!(schema.contains("openapi"), "Schema should contain 'openapi' version field");
    assert!(schema.contains("/api/health"), "Schema should document /api/health endpoint");
    assert!(schema.contains("GeminiHydra"), "Schema should contain project name");
}

#[test]
fn openapi_schema_documents_key_endpoints() {
    let schema = serde_json::to_string_pretty(&geminihydra_backend::ApiDoc::openapi())
        .expect("OpenAPI schema should serialize to JSON");
    assert!(schema.contains("/api/agents"), "Schema should document /api/agents");
    assert!(schema.contains("/api/execute"), "Schema should document /api/execute");
    assert!(schema.contains("/api/models"), "Schema should document /api/models");
    assert!(schema.contains("/api/sessions"), "Schema should document /api/sessions");
}

#[test]
fn openapi_schema_parses_to_valid_structure() {
    let doc = geminihydra_backend::ApiDoc::openapi();
    let value = serde_json::to_value(&doc).expect("Schema should convert to Value");
    assert!(value.is_object(), "Schema root should be an object");
    assert!(value.get("info").is_some(), "Schema should have 'info' section");
    assert!(value.get("paths").is_some(), "Schema should have 'paths' section");
}
