//! REST API Framework
//!
//! Comprehensive REST API implementation with OpenAPI 3.0 specification,
//! automatic documentation generation, and developer-friendly endpoints.

use crate::sdk_v2::{
    error::{SDKError, SDKResult},
    types::*,
    config::Config,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use reqwest::{Client, Method, Response};
use url::Url;

/// REST API client for BitCraps platform
#[derive(Debug)]
pub struct RestClient {
    client: Client,
    base_url: String,
    api_key: String,
    default_headers: HashMap<String, String>,
}

impl RestClient {
    /// Create a new REST client
    pub fn new(config: &Config) -> SDKResult<Self> {
        let client = Client::builder()
            .timeout(config.request_timeout)
            .user_agent(&config.user_agent)
            .build()
            .map_err(|e| SDKError::ConfigurationError(e.to_string()))?;

        let mut default_headers = config.custom_headers.clone();
        default_headers.insert("Content-Type".to_string(), "application/json".to_string());
        default_headers.insert("Accept".to_string(), "application/json".to_string());
        default_headers.insert("Authorization".to_string(), format!("Bearer {}", config.api_key));

        Ok(Self {
            client,
            base_url: config.base_url.clone(),
            api_key: config.api_key.clone(),
            default_headers,
        })
    }

    /// Make a GET request
    pub async fn get<T>(&self, path: &str) -> SDKResult<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.request::<(), T>(Method::GET, path, None).await
    }

    /// Make a GET request with query parameters
    pub async fn get_with_params<Q, T>(&self, path: &str, params: Q) -> SDKResult<T>
    where
        Q: Serialize,
        T: for<'de> Deserialize<'de>,
    {
        let url = self.build_url_with_params(path, &params)?;
        self.request_url::<(), T>(Method::GET, &url, None).await
    }

    /// Make a POST request
    pub async fn post<Q, T>(&self, path: &str, body: Q) -> SDKResult<T>
    where
        Q: Serialize,
        T: for<'de> Deserialize<'de>,
    {
        self.request(Method::POST, path, Some(body)).await
    }

    /// Make a PUT request
    pub async fn put<Q, T>(&self, path: &str, body: Q) -> SDKResult<T>
    where
        Q: Serialize,
        T: for<'de> Deserialize<'de>,
    {
        self.request(Method::PUT, path, Some(body)).await
    }

    /// Make a PATCH request
    pub async fn patch<Q, T>(&self, path: &str, body: Q) -> SDKResult<T>
    where
        Q: Serialize,
        T: for<'de> Deserialize<'de>,
    {
        self.request(Method::PATCH, path, Some(body)).await
    }

    /// Make a DELETE request
    pub async fn delete<T>(&self, path: &str) -> SDKResult<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.request::<(), T>(Method::DELETE, path, None).await
    }

    /// Generic request method
    async fn request<Q, T>(&self, method: Method, path: &str, body: Option<Q>) -> SDKResult<T>
    where
        Q: Serialize,
        T: for<'de> Deserialize<'de>,
    {
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), path.trim_start_matches('/'));
        self.request_url(method, &url, body).await
    }

    /// Request with full URL
    async fn request_url<Q, T>(&self, method: Method, url: &str, body: Option<Q>) -> SDKResult<T>
    where
        Q: Serialize,
        T: for<'de> Deserialize<'de>,
    {
        let mut request = self.client.request(method, url);

        // Add default headers
        for (key, value) in &self.default_headers {
            request = request.header(key, value);
        }

        // Add body if provided
        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request.send().await?;
        self.handle_response(response).await
    }

    /// Handle HTTP response and convert to typed result
    async fn handle_response<T>(&self, response: Response) -> SDKResult<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let status = response.status();
        let headers = response.headers().clone();

        if status.is_success() {
            let body = response.text().await?;
            if body.is_empty() {
                // For empty responses, try to deserialize unit type or default
                serde_json::from_str("null").map_err(|e| SDKError::SerializationError(e.to_string()))
            } else {
                serde_json::from_str(&body).map_err(|e| {
                    SDKError::SerializationError(format!("Failed to deserialize response: {}", e))
                })
            }
        } else {
            let body = response.text().await.unwrap_or_default();
            
            // Try to parse error response
            if let Ok(api_error) = serde_json::from_str::<ApiErrorResponse>(&body) {
                Err(SDKError::ApiError {
                    status: status.as_u16(),
                    message: api_error.message,
                    error_code: api_error.error_code,
                    details: api_error.details,
                })
            } else {
                // Check for rate limiting
                if status.as_u16() == 429 {
                    let retry_after = headers.get("retry-after")
                        .and_then(|h| h.to_str().ok())
                        .and_then(|s| s.parse().ok());
                    
                    Err(SDKError::RateLimitError {
                        message: "Rate limit exceeded".to_string(),
                        retry_after,
                        limit: None,
                        remaining: None,
                    })
                } else {
                    Err(SDKError::ApiError {
                        status: status.as_u16(),
                        message: body,
                        error_code: None,
                        details: None,
                    })
                }
            }
        }
    }

    /// Build URL with query parameters
    fn build_url_with_params<Q>(&self, path: &str, params: &Q) -> SDKResult<String>
    where
        Q: Serialize,
    {
        let base_url = format!("{}/{}", self.base_url.trim_end_matches('/'), path.trim_start_matches('/'));
        let mut url = Url::parse(&base_url)?;

        // Convert params to query string
        let query_string = serde_urlencoded::to_string(params)
            .map_err(|e| SDKError::SerializationError(e.to_string()))?;
        
        if !query_string.is_empty() {
            url.set_query(Some(&query_string));
        }

        Ok(url.to_string())
    }
}

/// API error response structure
#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    message: String,
    error_code: Option<String>,
    details: Option<serde_json::Value>,
}

/// OpenAPI 3.0 specification builder
pub struct OpenAPIBuilder {
    spec: OpenAPISpec,
}

/// OpenAPI 3.0 specification structure
#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAPISpec {
    pub openapi: String,
    pub info: ApiInfo,
    pub servers: Vec<ServerInfo>,
    pub paths: HashMap<String, PathItem>,
    pub components: Components,
    pub security: Vec<SecurityRequirement>,
    pub tags: Vec<Tag>,
}

/// API information
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiInfo {
    pub title: String,
    pub description: String,
    pub version: String,
    pub contact: Option<Contact>,
    pub license: Option<License>,
}

/// Server information
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerInfo {
    pub url: String,
    pub description: String,
}

/// API path item
#[derive(Debug, Serialize, Deserialize)]
pub struct PathItem {
    pub get: Option<Operation>,
    pub post: Option<Operation>,
    pub put: Option<Operation>,
    pub patch: Option<Operation>,
    pub delete: Option<Operation>,
    pub parameters: Option<Vec<Parameter>>,
}

/// API operation
#[derive(Debug, Serialize, Deserialize)]
pub struct Operation {
    pub summary: String,
    pub description: Option<String>,
    pub operation_id: String,
    pub tags: Vec<String>,
    pub parameters: Option<Vec<Parameter>>,
    pub request_body: Option<RequestBody>,
    pub responses: HashMap<String, Response>,
    pub security: Option<Vec<SecurityRequirement>>,
}

/// API parameter
#[derive(Debug, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub r#in: String, // "query", "header", "path", "cookie"
    pub description: Option<String>,
    pub required: bool,
    pub schema: Schema,
}

/// Request body specification
#[derive(Debug, Serialize, Deserialize)]
pub struct RequestBody {
    pub description: Option<String>,
    pub content: HashMap<String, MediaType>,
    pub required: bool,
}

/// Media type specification
#[derive(Debug, Serialize, Deserialize)]
pub struct MediaType {
    pub schema: Schema,
    pub examples: Option<HashMap<String, Example>>,
}

/// API response specification (using different name to avoid conflict)
#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub description: String,
    pub content: Option<HashMap<String, MediaType>>,
    pub headers: Option<HashMap<String, Header>>,
}

/// Header specification
#[derive(Debug, Serialize, Deserialize)]
pub struct Header {
    pub description: Option<String>,
    pub schema: Schema,
}

/// Schema specification
#[derive(Debug, Serialize, Deserialize)]
pub struct Schema {
    pub r#type: Option<String>,
    pub format: Option<String>,
    pub items: Option<Box<Schema>>,
    pub properties: Option<HashMap<String, Schema>>,
    pub required: Option<Vec<String>>,
    pub example: Option<serde_json::Value>,
    pub r#enum: Option<Vec<serde_json::Value>>,
    #[serde(rename = "$ref")]
    pub reference: Option<String>,
}

/// Components specification
#[derive(Debug, Serialize, Deserialize)]
pub struct Components {
    pub schemas: HashMap<String, Schema>,
    pub security_schemes: HashMap<String, SecurityScheme>,
}

/// Security scheme
#[derive(Debug, Serialize, Deserialize)]
pub struct SecurityScheme {
    pub r#type: String,
    pub scheme: Option<String>,
    pub bearer_format: Option<String>,
    pub description: Option<String>,
}

/// Security requirement
#[derive(Debug, Serialize, Deserialize)]
pub struct SecurityRequirement {
    #[serde(flatten)]
    pub requirements: HashMap<String, Vec<String>>,
}

/// API tag
#[derive(Debug, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub description: Option<String>,
}

/// Example specification
#[derive(Debug, Serialize, Deserialize)]
pub struct Example {
    pub summary: Option<String>,
    pub description: Option<String>,
    pub value: serde_json::Value,
}

/// Contact information
#[derive(Debug, Serialize, Deserialize)]
pub struct Contact {
    pub name: String,
    pub url: Option<String>,
    pub email: Option<String>,
}

/// License information
#[derive(Debug, Serialize, Deserialize)]
pub struct License {
    pub name: String,
    pub url: Option<String>,
}

impl OpenAPIBuilder {
    /// Create a new OpenAPI specification builder
    pub fn new() -> Self {
        let spec = OpenAPISpec {
            openapi: "3.0.3".to_string(),
            info: ApiInfo {
                title: "BitCraps API".to_string(),
                description: "Comprehensive REST API for the BitCraps gaming platform".to_string(),
                version: "2.0.0".to_string(),
                contact: Some(Contact {
                    name: "BitCraps Support".to_string(),
                    url: Some("https://bitcraps.com/support".to_string()),
                    email: Some("api@bitcraps.com".to_string()),
                }),
                license: Some(License {
                    name: "MIT".to_string(),
                    url: Some("https://opensource.org/licenses/MIT".to_string()),
                }),
            },
            servers: vec![
                ServerInfo {
                    url: "https://api.bitcraps.com/v2".to_string(),
                    description: "Production server".to_string(),
                },
                ServerInfo {
                    url: "https://staging-api.bitcraps.com/v2".to_string(),
                    description: "Staging server".to_string(),
                },
            ],
            paths: HashMap::new(),
            components: Components {
                schemas: HashMap::new(),
                security_schemes: {
                    let mut schemes = HashMap::new();
                    schemes.insert("bearerAuth".to_string(), SecurityScheme {
                        r#type: "http".to_string(),
                        scheme: Some("bearer".to_string()),
                        bearer_format: Some("JWT".to_string()),
                        description: Some("JWT token for authentication".to_string()),
                    });
                    schemes
                },
            },
            security: vec![{
                let mut req = HashMap::new();
                req.insert("bearerAuth".to_string(), vec![]);
                SecurityRequirement { requirements: req }
            }],
            tags: vec![
                Tag {
                    name: "Games".to_string(),
                    description: Some("Game management operations".to_string()),
                },
                Tag {
                    name: "Players".to_string(),
                    description: Some("Player management operations".to_string()),
                },
                Tag {
                    name: "Consensus".to_string(),
                    description: Some("Consensus and voting operations".to_string()),
                },
                Tag {
                    name: "Network".to_string(),
                    description: Some("Network and peer management".to_string()),
                },
            ],
        };

        Self { spec }
    }

    /// Add a path to the specification
    pub fn add_path(mut self, path: &str, item: PathItem) -> Self {
        self.spec.paths.insert(path.to_string(), item);
        self
    }

    /// Add a schema component
    pub fn add_schema(mut self, name: &str, schema: Schema) -> Self {
        self.spec.components.schemas.insert(name.to_string(), schema);
        self
    }

    /// Build the complete specification
    pub fn build(mut self) -> OpenAPISpec {
        // Add common schemas
        self = self.add_common_schemas();
        self.spec
    }

    /// Add common schemas used throughout the API
    fn add_common_schemas(self) -> Self {
        self.add_schema("GameInfo", Schema {
            r#type: Some("object".to_string()),
            properties: Some({
                let mut props = HashMap::new();
                props.insert("id".to_string(), Schema {
                    r#type: Some("string".to_string()),
                    description: Some("Unique game identifier".to_string()),
                    ..Default::default()
                });
                props.insert("name".to_string(), Schema {
                    r#type: Some("string".to_string()),
                    description: Some("Game name".to_string()),
                    ..Default::default()
                });
                props.insert("status".to_string(), Schema {
                    reference: Some("#/components/schemas/GameStatus".to_string()),
                    ..Default::default()
                });
                props
            }),
            required: Some(vec!["id".to_string(), "name".to_string(), "status".to_string()]),
            ..Default::default()
        })
        .add_schema("GameStatus", Schema {
            r#type: Some("string".to_string()),
            r#enum: Some(vec![
                serde_json::Value::String("Waiting".to_string()),
                serde_json::Value::String("InProgress".to_string()),
                serde_json::Value::String("Finished".to_string()),
                serde_json::Value::String("Cancelled".to_string()),
            ]),
            ..Default::default()
        })
    }
}

impl Default for Schema {
    fn default() -> Self {
        Self {
            r#type: None,
            format: None,
            items: None,
            properties: None,
            required: None,
            example: None,
            r#enum: None,
            reference: None,
        }
    }
}

/// Generate OpenAPI specification for BitCraps API
pub fn generate_openapi_spec() -> OpenAPISpec {
    OpenAPIBuilder::new()
        .add_path("/games", PathItem {
            get: Some(Operation {
                summary: "List games".to_string(),
                description: Some("Retrieve a list of available games with optional filtering".to_string()),
                operation_id: "listGames".to_string(),
                tags: vec!["Games".to_string()],
                parameters: Some(vec![
                    Parameter {
                        name: "status".to_string(),
                        r#in: "query".to_string(),
                        description: Some("Filter by game status".to_string()),
                        required: false,
                        schema: Schema {
                            reference: Some("#/components/schemas/GameStatus".to_string()),
                            ..Default::default()
                        },
                    },
                    Parameter {
                        name: "limit".to_string(),
                        r#in: "query".to_string(),
                        description: Some("Maximum number of results".to_string()),
                        required: false,
                        schema: Schema {
                            r#type: Some("integer".to_string()),
                            format: Some("int32".to_string()),
                            ..Default::default()
                        },
                    },
                ]),
                request_body: None,
                responses: {
                    let mut responses = HashMap::new();
                    responses.insert("200".to_string(), Response {
                        description: "List of games".to_string(),
                        content: Some({
                            let mut content = HashMap::new();
                            content.insert("application/json".to_string(), MediaType {
                                schema: Schema {
                                    r#type: Some("array".to_string()),
                                    items: Some(Box::new(Schema {
                                        reference: Some("#/components/schemas/GameInfo".to_string()),
                                        ..Default::default()
                                    })),
                                    ..Default::default()
                                },
                                examples: None,
                            });
                            content
                        }),
                        headers: None,
                    });
                    responses
                },
                security: None,
            }),
            post: Some(Operation {
                summary: "Create game".to_string(),
                description: Some("Create a new game instance".to_string()),
                operation_id: "createGame".to_string(),
                tags: vec!["Games".to_string()],
                parameters: None,
                request_body: Some(RequestBody {
                    description: Some("Game creation parameters".to_string()),
                    content: {
                        let mut content = HashMap::new();
                        content.insert("application/json".to_string(), MediaType {
                            schema: Schema {
                                reference: Some("#/components/schemas/CreateGameRequest".to_string()),
                                ..Default::default()
                            },
                            examples: None,
                        });
                        content
                    },
                    required: true,
                }),
                responses: {
                    let mut responses = HashMap::new();
                    responses.insert("201".to_string(), Response {
                        description: "Game created successfully".to_string(),
                        content: Some({
                            let mut content = HashMap::new();
                            content.insert("application/json".to_string(), MediaType {
                                schema: Schema {
                                    reference: Some("#/components/schemas/GameInfo".to_string()),
                                    ..Default::default()
                                },
                                examples: None,
                            });
                            content
                        }),
                        headers: None,
                    });
                    responses
                },
                security: None,
            }),
            put: None,
            patch: None,
            delete: None,
            parameters: None,
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk_v2::config::{Config, Environment};

    #[test]
    fn test_openapi_spec_generation() {
        let spec = generate_openapi_spec();
        assert_eq!(spec.openapi, "3.0.3");
        assert_eq!(spec.info.title, "BitCraps API");
        assert!(spec.paths.contains_key("/games"));
    }

    #[tokio::test]
    async fn test_rest_client_creation() {
        let config = Config::builder()
            .api_key("test-key")
            .environment(Environment::Testing)
            .build()
            .unwrap();

        let client = RestClient::new(&config);
        assert!(client.is_ok());
    }
}