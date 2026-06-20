use std::path::{Path, PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use brassclaw_host_api::{
    CapabilityDescriptor, CapabilityId, EffectKind, ExtensionId, PermissionMode, ResourceCeiling,
    ResourceEstimate, ResourceProfile, RuntimeKind, TrustClass,
};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

// ============================================================================
// V2 Path Validation
// ============================================================================

/// Validate and resolve a path relative to a base directory.
///
/// This function:
/// 1. Checks for empty paths
/// 2. Resolves relative paths against the base directory
/// 3. Normalizes the path lexically
/// 4. Ensures the resolved path doesn't escape the base directory
///
/// Returns the validated absolute path or an error.
fn validate_path(raw: &str, base: Option<&Path>) -> Result<PathBuf, ImagesCapabilityError> {
    if raw.is_empty() {
        return Err(ImagesCapabilityError::input("empty path"));
    }
    
    let path = Path::new(raw);
    
    // Resolve relative to base if provided
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else if let Some(base) = base {
        base.join(path)
    } else {
        path.to_path_buf()
    };
    
    // Normalize lexically to handle ".." and "." components
    let normalized = normalize_lexical(&resolved);
    
    // If base is provided, ensure the normalized path doesn't escape it
    if let Some(base) = base {
        let base_normalized = normalize_lexical(base);
        if !normalized.starts_with(&base_normalized) {
            return Err(ImagesCapabilityError::input(format!(
                "path escapes base directory: {}",
                raw
            )));
        }
    }
    
    Ok(normalized)
}

/// Normalize a path lexically without filesystem access.
///
/// Removes "." components and resolves ".." by popping the previous component.
fn normalize_lexical(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                if !components.is_empty() {
                    components.pop();
                }
            }
            std::path::Component::CurDir => {}
            _ => components.push(component),
        }
    }
    components.iter().collect()
}

// ============================================================================
// END V2 Path Validation
// ============================================================================

pub const PROVIDER_ID: &str = "builtin";
pub const IMAGE_GENERATE_CAPABILITY_ID: &str = "builtin.image_generate";
pub const IMAGE_ANALYZE_CAPABILITY_ID: &str = "builtin.image_analyze";
pub const IMAGE_EDIT_CAPABILITY_ID: &str = "builtin.image_edit";

const DEFAULT_OUTPUT_BYTES: u64 = 64 * 1024;
const MAX_OUTPUT_BYTES: u64 = 10 * 1024 * 1024;
const DEFAULT_WALL_CLOCK_MS: u64 = 30_000;
const MAX_WALL_CLOCK_MS: u64 = 300_000;

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct ImagesCapabilityError {
    pub message: String,
    pub is_input_error: bool,
}

impl ImagesCapabilityError {
    fn input(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            is_input_error: true,
        }
    }

    fn operation(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            is_input_error: false,
        }
    }
}

pub struct ImagesContext {
    pub api_base_url: String,
    pub api_key: SecretString,
    pub gen_model: String,
    pub vision_model: String,
    pub client: reqwest::Client,
    pub base_dir: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct ImageGenRequest {
    model: String,
    prompt: String,
    size: String,
    response_format: String,
    n: u32,
}

#[derive(Debug, Deserialize)]
struct ImageGenResponse {
    data: Vec<ImageGenData>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ImageGenData {
    b64_json: Option<String>,
    url: Option<String>,
}

fn resource_profile() -> Option<ResourceProfile> {
    Some(ResourceProfile {
        default_estimate: ResourceEstimate {
            wall_clock_ms: Some(DEFAULT_WALL_CLOCK_MS),
            output_bytes: Some(DEFAULT_OUTPUT_BYTES),
            ..ResourceEstimate::default()
        },
        hard_ceiling: Some(ResourceCeiling {
            max_usd: None,
            max_input_tokens: None,
            max_output_tokens: None,
            max_wall_clock_ms: Some(MAX_WALL_CLOCK_MS),
            max_output_bytes: Some(MAX_OUTPUT_BYTES),
            sandbox: None,
        }),
    })
}

fn make_descriptor(
    id: &str,
    description: &str,
    effects: Vec<EffectKind>,
    parameters_schema: Value,
    default_permission: PermissionMode,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: CapabilityId::new(id).expect("valid capability id"),
        provider: ExtensionId::new(PROVIDER_ID).expect("valid provider id"),
        runtime: RuntimeKind::FirstParty,
        trust_ceiling: TrustClass::Sandbox,
        description: description.to_string(),
        parameters_schema,
        effects,
        default_permission,
        runtime_credentials: Vec::new(),
        resource_profile: resource_profile(),
    }
}

pub fn image_generate_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        IMAGE_GENERATE_CAPABILITY_ID,
        "Generate an image from a text prompt using an AI image generation model. Returns the generated image data.",
        vec![EffectKind::Network, EffectKind::ExternalWrite],
        json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "Text description of the image to generate (max 4000 chars)",
                    "maxLength": 4000
                },
                "size": {
                    "type": "string",
                    "description": "Image dimensions",
                    "enum": ["1024x1024", "1792x1024", "1024x1792"],
                    "default": "1024x1024"
                }
            },
            "required": ["prompt"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn image_analyze_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        IMAGE_ANALYZE_CAPABILITY_ID,
        "Analyze an image using a vision-capable AI model. Provide a workspace path to the image and an optional analysis question.",
        vec![EffectKind::Network, EffectKind::ExternalWrite],
        json!({
            "type": "object",
            "properties": {
                "image_path": {
                    "type": "string",
                    "description": "Path to the image file in the workspace"
                },
                "question": {
                    "type": "string",
                    "description": "Specific question to answer about the image. Defaults to general analysis.",
                    "default": "Describe this image in detail."
                }
            },
            "required": ["image_path"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn image_edit_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        IMAGE_EDIT_CAPABILITY_ID,
        "Edit an existing image using an AI model. Provide the workspace path to the source image and a text prompt describing the desired edits.",
        vec![EffectKind::Network, EffectKind::ExternalWrite],
        json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "Text description of the edits to apply to the image",
                    "maxLength": 4000
                },
                "image_path": {
                    "type": "string",
                    "description": "Path to the source image in the workspace"
                }
            },
            "required": ["prompt", "image_path"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn descriptors() -> Vec<CapabilityDescriptor> {
    vec![
        image_generate_descriptor(),
        image_analyze_descriptor(),
        image_edit_descriptor(),
    ]
}

fn require_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, ImagesCapabilityError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ImagesCapabilityError::input(format!("missing required parameter: {key}")))
}

fn endpoint_url(api_base_url: &str, path: &str) -> String {
    // V1 - DISABLED - depends on deleted V1 code
    // crate::tools::builtin::image_api_endpoint_url(api_base_url, path)
    format!("{}{}", api_base_url, path)
}

fn media_type_from_path(_path: &str) -> String {
    // V1 - DISABLED - depends on deleted V1 code
    // crate::tools::builtin::media_type_from_path(path)
    "image/png".to_string()
}

pub(crate) fn infer_generated_image_media_type(image_b64: &str) -> &'static str {
    let prefix_len = image_b64.len().min(64);
    let decodable_len = prefix_len - (prefix_len % 4);
    if decodable_len == 0 {
        return "image/png";
    }
    let prefix = image_b64.get(..decodable_len).unwrap_or("");
    let Ok(bytes) = STANDARD.decode(prefix) else {
        return "image/png";
    };

    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        "image/png"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif"
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "image/png"
    }
}

async fn read_image_bytes(
    image_path: &str,
    base_dir: Option<&Path>,
) -> Result<Vec<u8>, ImagesCapabilityError> {
    // V2: Validate path to prevent directory traversal attacks
    let resolved = validate_path(image_path, base_dir)?;

    tokio::fs::read(&resolved)
        .await
        .map_err(|e| ImagesCapabilityError::operation(format!("Failed to read image file: {e}")))
}

pub async fn execute_image_generate(
    params: &Value,
    ctx: &ImagesContext,
) -> Result<Value, ImagesCapabilityError> {
    let prompt = require_str(params, "prompt")?;

    if prompt.len() > 4000 {
        return Err(ImagesCapabilityError::input(
            "Prompt exceeds 4000 character limit",
        ));
    }

    let size = params
        .get("size")
        .and_then(|v| v.as_str())
        .unwrap_or("1024x1024");

    if !["1024x1024", "1792x1024", "1024x1792"].contains(&size) {
        return Err(ImagesCapabilityError::input(format!(
            "Invalid size '{}'. Must be 1024x1024, 1792x1024, or 1024x1792",
            size
        )));
    }

    let url = endpoint_url(&ctx.api_base_url, "/images/generations");

    let request_body = ImageGenRequest {
        model: ctx.gen_model.clone(),
        prompt: prompt.to_string(),
        size: size.to_string(),
        response_format: "b64_json".to_string(),
        n: 1,
    };

    let response = ctx
        .client
        .post(&url)
        .bearer_auth(ctx.api_key.expose_secret())
        .json(&request_body)
        .send()
        .await
        .map_err(|e| ImagesCapabilityError::operation(format!("Image generation request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ImagesCapabilityError::operation(format!(
            "Image generation API returned {status}: {body}"
        )));
    }

    let gen_response: ImageGenResponse = response.json().await.map_err(|e| {
        ImagesCapabilityError::operation(format!("Failed to parse image generation response: {e}"))
    })?;

    let image_data = gen_response
        .data
        .first()
        .and_then(|d| d.b64_json.as_deref())
        .ok_or_else(|| ImagesCapabilityError::operation("No image data in response"))?;

    let media_type = infer_generated_image_media_type(image_data);

    Ok(json!({
        "type": "image_generated",
        "data": format!("data:{media_type};base64,{}", image_data),
        "media_type": media_type,
        "prompt": prompt,
        "size": size
    }))
}

pub async fn execute_image_analyze(
    params: &Value,
    ctx: &ImagesContext,
) -> Result<Value, ImagesCapabilityError> {
    let image_path = require_str(params, "image_path")?;

    let question = params
        .get("question")
        .and_then(|v| v.as_str())
        .unwrap_or("Describe this image in detail.");

    let image_bytes = read_image_bytes(image_path, ctx.base_dir.as_deref()).await?;
    if image_bytes.is_empty() {
        return Err(ImagesCapabilityError::operation("Image file is empty"));
    }

    let mt = media_type_from_path(image_path);
    let b64 = STANDARD.encode(&image_bytes);
    let data_url = format!("data:{mt};base64,{b64}");

    let url = format!(
        "{}/v1/chat/completions",
        ctx.api_base_url.trim_end_matches('/')
    );

    let request_body = json!({
        "model": &ctx.vision_model,
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": question },
                { "type": "image_url", "image_url": { "url": data_url } }
            ]
        }],
        "max_tokens": 2048
    });

    let response = ctx
        .client
        .post(&url)
        .bearer_auth(ctx.api_key.expose_secret())
        .json(&request_body)
        .send()
        .await
        .map_err(|e| ImagesCapabilityError::operation(format!("Vision API request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ImagesCapabilityError::operation(format!(
            "Vision API returned {status}: {body}"
        )));
    }

    let resp: Value = response.json().await.map_err(|e| {
        ImagesCapabilityError::operation(format!("Failed to parse vision API response: {e}"))
    })?;

    let analysis = resp
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or("No analysis available.");

    Ok(json!({ "analysis": analysis }))
}

pub async fn execute_image_edit(
    params: &Value,
    ctx: &ImagesContext,
) -> Result<Value, ImagesCapabilityError> {
    let prompt = require_str(params, "prompt")?;
    let image_path = require_str(params, "image_path")?;

    if prompt.len() > 4000 {
        return Err(ImagesCapabilityError::input(
            "Prompt exceeds 4000 character limit",
        ));
    }

    let image_bytes = read_image_bytes(image_path, ctx.base_dir.as_deref()).await?;
    if image_bytes.is_empty() {
        return Err(ImagesCapabilityError::operation(
            "Source image file is empty",
        ));
    }

    let mt = media_type_from_path(image_path);
    let url = endpoint_url(&ctx.api_base_url, "/images/edits");

    let form = reqwest::multipart::Form::new()
        .text("model", ctx.gen_model.clone())
        .text("prompt", prompt.to_string())
        .text("response_format", "b64_json")
        .part(
            "image",
            reqwest::multipart::Part::bytes(image_bytes)
                .mime_str(&mt)
                .map_err(|e| ImagesCapabilityError::operation(format!("Invalid media type: {e}")))?
                .file_name("image"),
        );

    let response = ctx
        .client
        .post(&url)
        .bearer_auth(ctx.api_key.expose_secret())
        .multipart(form)
        .send()
        .await
        .map_err(|e| ImagesCapabilityError::operation(format!("Image edit request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if status.as_u16() == 404 {
            tracing::warn!(
                "Image edit endpoint returned 404, falling back to generation API. \
                 Note: the source image will NOT be used."
            );
            return fallback_generate(ctx, prompt).await;
        }

        return Err(ImagesCapabilityError::operation(format!(
            "Image edit API returned {status}: {body}"
        )));
    }

    let resp: Value = response.json().await.map_err(|e| {
        ImagesCapabilityError::operation(format!("Failed to parse image edit response: {e}"))
    })?;

    let edited_data = resp
        .pointer("/data/0/b64_json")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ImagesCapabilityError::operation("No image data in edit response"))?;
    let generated_media_type = infer_generated_image_media_type(edited_data);

    Ok(json!({
        "type": "image_generated",
        "data": format!("data:{generated_media_type};base64,{}", edited_data),
        "media_type": generated_media_type,
        "prompt": prompt,
        "source_path": image_path
    }))
}

async fn fallback_generate(
    ctx: &ImagesContext,
    prompt: &str,
) -> Result<Value, ImagesCapabilityError> {
    let url = endpoint_url(&ctx.api_base_url, "/images/generations");

    let request_body = json!({
        "model": &ctx.gen_model,
        "prompt": prompt,
        "size": "1024x1024",
        "response_format": "b64_json",
        "n": 1
    });

    let response = ctx
        .client
        .post(&url)
        .bearer_auth(ctx.api_key.expose_secret())
        .json(&request_body)
        .send()
        .await
        .map_err(|e| {
            ImagesCapabilityError::operation(format!("Fallback image generation failed: {e}"))
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ImagesCapabilityError::operation(format!(
            "Fallback generation API returned {status}: {body}"
        )));
    }

    let resp: Value = response.json().await.map_err(|e| {
        ImagesCapabilityError::operation(format!("Failed to parse fallback response: {e}"))
    })?;

    let image_data = resp
        .pointer("/data/0/b64_json")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ImagesCapabilityError::operation("No image data in fallback response")
        })?;
    let generated_media_type = infer_generated_image_media_type(image_data);

    Ok(json!({
        "type": "image_generated",
        "data": format!("data:{generated_media_type};base64,{}", image_data),
        "media_type": generated_media_type,
        "prompt": prompt,
        "note": "Generated new image (edit endpoint unavailable — source image was NOT used)"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_generate_descriptor_is_valid() {
        let desc = image_generate_descriptor();
        assert_eq!(desc.id.as_str(), IMAGE_GENERATE_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::Network));
        assert!(desc.effects.contains(&EffectKind::ExternalWrite));
        assert_eq!(desc.default_permission, PermissionMode::Allow);
    }

    #[test]
    fn image_analyze_descriptor_is_valid() {
        let desc = image_analyze_descriptor();
        assert_eq!(desc.id.as_str(), IMAGE_ANALYZE_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::Network));
    }

    #[test]
    fn image_edit_descriptor_is_valid() {
        let desc = image_edit_descriptor();
        assert_eq!(desc.id.as_str(), IMAGE_EDIT_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::Network));
    }

    #[test]
    fn descriptors_returns_all() {
        let descs = descriptors();
        assert_eq!(descs.len(), 3);
    }

    #[test]
    fn infer_media_type_detects_jpeg() {
        let jpeg_b64 = STANDARD.encode([0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10]);
        assert_eq!(infer_generated_image_media_type(&jpeg_b64), "image/jpeg");
    }

    #[test]
    fn infer_media_type_detects_png() {
        let png_b64 = STANDARD.encode([0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(infer_generated_image_media_type(&png_b64), "image/png");
    }

    #[test]
    fn infer_media_type_detects_gif() {
        let gif_b64 = STANDARD.encode(b"GIF89a");
        assert_eq!(infer_generated_image_media_type(&gif_b64), "image/gif");
    }

    #[test]
    fn infer_media_type_detects_webp() {
        let mut riff = b"RIFF".to_vec();
        riff.extend_from_slice(&[0x00; 4]);
        riff.extend_from_slice(b"WEBP");
        let webp_b64 = STANDARD.encode(&riff);
        assert_eq!(infer_generated_image_media_type(&webp_b64), "image/webp");
    }

    #[test]
    fn infer_media_type_defaults_for_empty() {
        assert_eq!(infer_generated_image_media_type(""), "image/png");
    }

    #[test]
    fn infer_media_type_defaults_for_invalid_base64() {
        assert_eq!(
            infer_generated_image_media_type("!!!not-base64"),
            "image/png"
        );
    }

    #[tokio::test]
    async fn test_read_image_bytes_rejects_path_traversal() {
        let dir = tempfile::TempDir::new().unwrap();
        let result = read_image_bytes("../../etc/passwd", Some(dir.path())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_image_bytes_rejects_absolute_path_outside_sandbox() {
        let dir = tempfile::TempDir::new().unwrap();
        let result = read_image_bytes("/etc/passwd", Some(dir.path())).await;
        assert!(result.is_err());
    }
}
