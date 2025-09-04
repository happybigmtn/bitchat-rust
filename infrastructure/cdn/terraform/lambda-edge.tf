# Lambda@Edge Functions for CDN Edge Processing

# IAM role for Lambda@Edge functions
resource "aws_iam_role" "lambda_edge_role" {
  name = "bitcraps-lambda-edge-role-${var.environment}"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = [
            "lambda.amazonaws.com",
            "edgelambda.amazonaws.com"
          ]
        }
      }
    ]
  })

  tags = local.common_tags
}

# IAM policy for Lambda@Edge execution
resource "aws_iam_role_policy_attachment" "lambda_edge_execution" {
  role       = aws_iam_role.lambda_edge_role.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

# Additional permissions for edge functions
resource "aws_iam_role_policy" "lambda_edge_policy" {
  name = "bitcraps-lambda-edge-policy-${var.environment}"
  role = aws_iam_role.lambda_edge_role.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "logs:CreateLogGroup",
          "logs:CreateLogStream",
          "logs:PutLogEvents"
        ]
        Resource = "arn:aws:logs:*:*:*"
      },
      {
        Effect = "Allow"
        Action = [
          "dynamodb:GetItem",
          "dynamodb:PutItem",
          "dynamodb:UpdateItem"
        ]
        Resource = "arn:aws:dynamodb:*:*:table/bitcraps-*"
      }
    ]
  })
}

# API Authentication Lambda@Edge function
resource "aws_lambda_function" "api_auth_lambda" {
  count = var.enable_api_lambda ? 1 : 0

  filename         = data.archive_file.api_auth_lambda_zip[0].output_path
  function_name    = "bitcraps-api-auth-${var.environment}"
  role            = aws_iam_role.lambda_edge_role.arn
  handler         = "index.handler"
  source_code_hash = data.archive_file.api_auth_lambda_zip[0].output_base64sha256
  runtime         = "nodejs18.x"
  timeout         = 5
  memory_size     = 128
  publish         = true

  tags = merge(local.common_tags, {
    Purpose = "API Authentication"
  })
}

# Create the API auth Lambda source code
resource "local_file" "api_auth_lambda_source" {
  count = var.enable_api_lambda ? 1 : 0

  filename = "${path.module}/lambda-sources/api-auth/index.js"
  content = <<-EOT
'use strict';

const crypto = require('crypto');

// Rate limiting storage (in-memory for Lambda@Edge)
const rateLimitStore = new Map();

// API key validation
function validateApiKey(apiKey) {
    // Implement your API key validation logic
    // This could check against a DynamoDB table or hardcoded keys
    const validKeys = process.env.VALID_API_KEYS ? process.env.VALID_API_KEYS.split(',') : [];
    return validKeys.includes(apiKey);
}

// JWT token validation (simplified)
function validateJWT(token) {
    try {
        // In production, use proper JWT validation
        const parts = token.split('.');
        if (parts.length !== 3) return false;
        
        const payload = JSON.parse(Buffer.from(parts[1], 'base64').toString());
        const now = Math.floor(Date.now() / 1000);
        
        return payload.exp > now && payload.iat <= now;
    } catch (error) {
        return false;
    }
}

// Rate limiting
function checkRateLimit(clientIP, limit = 100, windowMs = 60000) {
    const now = Date.now();
    const key = clientIP;
    
    if (!rateLimitStore.has(key)) {
        rateLimitStore.set(key, { count: 1, resetTime: now + windowMs });
        return true;
    }
    
    const record = rateLimitStore.get(key);
    
    if (now > record.resetTime) {
        record.count = 1;
        record.resetTime = now + windowMs;
        return true;
    }
    
    if (record.count >= limit) {
        return false;
    }
    
    record.count++;
    return true;
}

// Security headers
function getSecurityHeaders() {
    return {
        'strict-transport-security': [{
            key: 'Strict-Transport-Security',
            value: 'max-age=31536000; includeSubDomains'
        }],
        'x-content-type-options': [{
            key: 'X-Content-Type-Options',
            value: 'nosniff'
        }],
        'x-frame-options': [{
            key: 'X-Frame-Options',
            value: 'DENY'
        }],
        'x-xss-protection': [{
            key: 'X-XSS-Protection',
            value: '1; mode=block'
        }]
    };
}

// Main Lambda@Edge handler
exports.handler = async (event) => {
    const request = event.Records[0].cf.request;
    const headers = request.headers;
    const clientIP = headers['cloudfront-viewer-address'] ? 
                    headers['cloudfront-viewer-address'][0].value.split(':')[0] : 'unknown';
    
    console.log(`Processing request: ${request.method} ${request.uri} from ${clientIP}`);
    
    // Skip authentication for health checks
    if (request.uri === '/health' || request.uri === '/status') {
        return request;
    }
    
    // Rate limiting
    if (!checkRateLimit(clientIP)) {
        console.log(`Rate limit exceeded for ${clientIP}`);
        return {
            status: '429',
            statusDescription: 'Too Many Requests',
            headers: {
                ...getSecurityHeaders(),
                'retry-after': [{
                    key: 'Retry-After',
                    value: '60'
                }]
            },
            body: JSON.stringify({
                error: 'Rate limit exceeded',
                retryAfter: 60
            })
        };
    }
    
    // API key authentication
    const apiKey = headers['x-api-key'] ? headers['x-api-key'][0].value : null;
    const authHeader = headers.authorization ? headers.authorization[0].value : null;
    
    let isAuthenticated = false;
    
    // Check API key
    if (apiKey && validateApiKey(apiKey)) {
        isAuthenticated = true;
        request.headers['x-authenticated-via'] = [{ key: 'X-Authenticated-Via', value: 'api-key' }];
    }
    // Check JWT token
    else if (authHeader && authHeader.startsWith('Bearer ')) {
        const token = authHeader.substring(7);
        if (validateJWT(token)) {
            isAuthenticated = true;
            request.headers['x-authenticated-via'] = [{ key: 'X-Authenticated-Via', value: 'jwt' }];
        }
    }
    
    // Public endpoints that don't require authentication
    const publicEndpoints = ['/api/health', '/api/status', '/api/games/public'];
    const isPublicEndpoint = publicEndpoints.some(endpoint => request.uri.startsWith(endpoint));
    
    if (!isAuthenticated && !isPublicEndpoint) {
        console.log(`Unauthorized request to ${request.uri} from ${clientIP}`);
        return {
            status: '401',
            statusDescription: 'Unauthorized',
            headers: {
                ...getSecurityHeaders(),
                'www-authenticate': [{
                    key: 'WWW-Authenticate',
                    value: 'Bearer realm="BitCraps API"'
                }]
            },
            body: JSON.stringify({
                error: 'Authentication required',
                message: 'Please provide a valid API key or authorization token'
            })
        };
    }
    
    // Add security and identification headers
    request.headers['x-edge-processed'] = [{ key: 'X-Edge-Processed', value: 'true' }];
    request.headers['x-client-ip'] = [{ key: 'X-Client-IP', value: clientIP }];
    request.headers['x-request-id'] = [{ 
        key: 'X-Request-ID', 
        value: crypto.randomBytes(16).toString('hex')
    }];
    
    console.log(`Authenticated request forwarded: ${request.uri}`);
    return request;
};
EOT

  depends_on = [local_file.lambda_directory]
}

# Create lambda sources directory
resource "local_file" "lambda_directory" {
  count = var.enable_api_lambda ? 1 : 0

  filename = "${path.module}/lambda-sources/.gitkeep"
  content  = ""

  provisioner "local-exec" {
    command = "mkdir -p ${path.module}/lambda-sources/api-auth"
  }
}

# Zip the Lambda function source
data "archive_file" "api_auth_lambda_zip" {
  count = var.enable_api_lambda ? 1 : 0

  type        = "zip"
  source_dir  = "${path.module}/lambda-sources/api-auth"
  output_path = "${path.module}/lambda-sources/api-auth.zip"

  depends_on = [local_file.api_auth_lambda_source]
}

# Image optimization Lambda@Edge function
resource "aws_lambda_function" "image_optimization_lambda" {
  count = var.enable_image_optimization ? 1 : 0

  filename         = data.archive_file.image_optimization_lambda_zip[0].output_path
  function_name    = "bitcraps-image-optimization-${var.environment}"
  role            = aws_iam_role.lambda_edge_role.arn
  handler         = "index.handler"
  source_code_hash = data.archive_file.image_optimization_lambda_zip[0].output_base64sha256
  runtime         = "nodejs18.x"
  timeout         = 30
  memory_size     = 512
  publish         = true

  tags = merge(local.common_tags, {
    Purpose = "Image Optimization"
  })
}

# Create the image optimization Lambda source code
resource "local_file" "image_optimization_lambda_source" {
  count = var.enable_image_optimization ? 1 : 0

  filename = "${path.module}/lambda-sources/image-optimization/index.js"
  content = <<-EOT
'use strict';

const querystring = require('querystring');

// Supported image formats and quality settings
const SUPPORTED_FORMATS = ['jpeg', 'jpg', 'png', 'webp'];
const DEFAULT_QUALITY = 80;
const MAX_WIDTH = 2048;
const MAX_HEIGHT = 2048;

// Image format detection
function getImageFormat(uri, accept) {
    const extension = uri.split('.').pop().toLowerCase();
    
    // Client supports WebP and image is not already WebP
    if (accept && accept.includes('image/webp') && extension !== 'webp') {
        return 'webp';
    }
    
    // Return original format if supported
    if (SUPPORTED_FORMATS.includes(extension)) {
        return extension === 'jpg' ? 'jpeg' : extension;
    }
    
    return 'jpeg'; // Default fallback
}

// Parse image transformation parameters
function parseImageParams(queryString) {
    const params = querystring.parse(queryString);
    
    return {
        width: Math.min(parseInt(params.w) || null, MAX_WIDTH),
        height: Math.min(parseInt(params.h) || null, MAX_HEIGHT),
        quality: Math.min(Math.max(parseInt(params.q) || DEFAULT_QUALITY, 1), 100),
        format: params.f || null,
        fit: params.fit || 'cover' // cover, contain, fill, inside, outside
    };
}

// Generate optimized image URL
function generateOptimizedUrl(originalUri, params, format) {
    const path = originalUri.split('?')[0];
    const optimizedParams = [];
    
    if (params.width) optimizedParams.push(`w=${params.width}`);
    if (params.height) optimizedParams.push(`h=${params.height}`);
    if (params.quality !== DEFAULT_QUALITY) optimizedParams.push(`q=${params.quality}`);
    if (params.format || format !== 'jpeg') optimizedParams.push(`f=${params.format || format}`);
    if (params.fit !== 'cover') optimizedParams.push(`fit=${params.fit}`);
    
    const queryString = optimizedParams.length > 0 ? `?${optimizedParams.join('&')}` : '';
    
    // Change file extension to match output format
    const basePath = path.replace(/\.[^.]+$/, '');
    const extension = format === 'jpeg' ? 'jpg' : format;
    
    return `${basePath}_optimized.${extension}${queryString}`;
}

// Cache key generation for optimized images
function generateCacheKey(uri, params, format) {
    const baseKey = uri.split('?')[0];
    return `${baseKey}_w${params.width || 'auto'}_h${params.height || 'auto'}_q${params.quality}_${format}_${params.fit}`;
}

exports.handler = async (event) => {
    const request = event.Records[0].cf.request;
    const headers = request.headers;
    
    console.log(`Image optimization request: ${request.uri}`);
    
    // Only process image requests
    if (!request.uri.match(/\.(jpg|jpeg|png|webp)$/i)) {
        return request;
    }
    
    // Parse transformation parameters
    const params = parseImageParams(request.querystring || '');
    const acceptHeader = headers.accept ? headers.accept[0].value : '';
    const targetFormat = params.format || getImageFormat(request.uri, acceptHeader);
    
    // Generate cache key for optimized image
    const cacheKey = generateCacheKey(request.uri, params, targetFormat);
    request.headers['x-image-cache-key'] = [{ key: 'X-Image-Cache-Key', value: cacheKey }];
    
    // If no optimization parameters provided, return original request
    if (!params.width && !params.height && !params.format && targetFormat === 'jpeg') {
        return request;
    }
    
    // Generate optimized image path
    const optimizedUri = generateOptimizedUrl(request.uri, params, targetFormat);
    
    console.log(`Optimizing image: ${request.uri} -> ${optimizedUri}`);
    
    // Update request to point to optimized version
    request.uri = optimizedUri;
    request.querystring = '';
    
    // Add transformation headers for origin processing
    request.headers['x-image-width'] = [{ key: 'X-Image-Width', value: String(params.width || '') }];
    request.headers['x-image-height'] = [{ key: 'X-Image-Height', value: String(params.height || '') }];
    request.headers['x-image-quality'] = [{ key: 'X-Image-Quality', value: String(params.quality) }];
    request.headers['x-image-format'] = [{ key: 'X-Image-Format', value: targetFormat }];
    request.headers['x-image-fit'] = [{ key: 'X-Image-Fit', value: params.fit }];
    request.headers['x-original-uri'] = [{ key: 'X-Original-URI', value: event.Records[0].cf.request.uri }];
    
    return request;
};
EOT
}

# Zip the image optimization Lambda function source
data "archive_file" "image_optimization_lambda_zip" {
  count = var.enable_image_optimization ? 1 : 0

  type        = "zip"
  source_dir  = "${path.module}/lambda-sources/image-optimization"
  output_path = "${path.module}/lambda-sources/image-optimization.zip"

  depends_on = [local_file.image_optimization_lambda_source]
}

# A/B Testing Lambda@Edge function
resource "aws_lambda_function" "ab_testing_lambda" {
  count = var.enable_ab_testing ? 1 : 0

  filename         = data.archive_file.ab_testing_lambda_zip[0].output_path
  function_name    = "bitcraps-ab-testing-${var.environment}"
  role            = aws_iam_role.lambda_edge_role.arn
  handler         = "index.handler"
  source_code_hash = data.archive_file.ab_testing_lambda_zip[0].output_base64sha256
  runtime         = "nodejs18.x"
  timeout         = 5
  memory_size     = 128
  publish         = true

  tags = merge(local.common_tags, {
    Purpose = "A/B Testing"
  })
}

# Create the A/B testing Lambda source code
resource "local_file" "ab_testing_lambda_source" {
  count = var.enable_ab_testing ? 1 : 0

  filename = "${path.module}/lambda-sources/ab-testing/index.js"
  content = <<-EOT
'use strict';

// A/B test configurations
const AB_TESTS = {
    'game-ui': {
        variations: ['control', 'variant-a', 'variant-b'],
        weights: [50, 30, 20], // Percentage allocation
        paths: ['/game', '/play']
    },
    'landing-page': {
        variations: ['original', 'new-design'],
        weights: [70, 30],
        paths: ['/']
    }
};

// Hash function for consistent user assignment
function hashString(str) {
    let hash = 0;
    for (let i = 0; i < str.length; i++) {
        const char = str.charCodeAt(i);
        hash = ((hash << 5) - hash) + char;
        hash = hash & hash; // Convert to 32-bit integer
    }
    return Math.abs(hash);
}

// Assign user to A/B test variation
function assignVariation(userId, testName, test) {
    const hash = hashString(userId + testName);
    const bucket = hash % 100;
    
    let cumulativeWeight = 0;
    for (let i = 0; i < test.variations.length; i++) {
        cumulativeWeight += test.weights[i];
        if (bucket < cumulativeWeight) {
            return test.variations[i];
        }
    }
    
    return test.variations[0]; // Fallback to first variation
}

// Generate user ID from IP and User-Agent
function generateUserId(request) {
    const ip = request.headers['cloudfront-viewer-address'] ? 
               request.headers['cloudfront-viewer-address'][0].value : 'unknown';
    const userAgent = request.headers['user-agent'] ? 
                     request.headers['user-agent'][0].value : 'unknown';
    
    return hashString(ip + userAgent).toString();
}

// Check if request path matches test configuration
function getActiveTest(uri) {
    for (const [testName, test] of Object.entries(AB_TESTS)) {
        if (test.paths.some(path => uri.startsWith(path))) {
            return { testName, test };
        }
    }
    return null;
}

exports.handler = async (event) => {
    const request = event.Records[0].cf.request;
    const response = event.Records[0].cf.response;
    
    // Only process viewer-response events for HTML content
    if (response && response.headers['content-type'] && 
        response.headers['content-type'][0].value.includes('text/html')) {
        
        const activeTest = getActiveTest(request.uri);
        if (!activeTest) {
            return response;
        }
        
        const userId = generateUserId(request);
        const variation = assignVariation(userId, activeTest.testName, activeTest.test);
        
        console.log(`A/B Test: ${activeTest.testName}, User: ${userId}, Variation: ${variation}`);
        
        // Add variation information to response headers
        response.headers['x-ab-test'] = [{ 
            key: 'X-AB-Test', 
            value: activeTest.testName 
        }];
        response.headers['x-ab-variation'] = [{ 
            key: 'X-AB-Variation', 
            value: variation 
        }];
        
        // Inject JavaScript for client-side A/B testing
        const abScript = `
            <script>
            window.abTest = {
                test: '${activeTest.testName}',
                variation: '${variation}',
                userId: '${userId}'
            };
            // Send analytics event
            if (window.gtag) {
                gtag('event', 'ab_test_assignment', {
                    'test_name': '${activeTest.testName}',
                    'variation': '${variation}',
                    'user_id': '${userId}'
                });
            }
            </script>
        `;
        
        // Inject script into HTML head
        if (response.body && response.body.data) {
            let html = Buffer.from(response.body.data, 'base64').toString('utf-8');
            html = html.replace('</head>', `${abScript}</head>`);
            
            response.body.data = Buffer.from(html, 'utf-8').toString('base64');
            response.body.encoding = 'base64';
        }
        
        return response;
    }
    
    return response;
};
EOT
}

# Zip the A/B testing Lambda function source
data "archive_file" "ab_testing_lambda_zip" {
  count = var.enable_ab_testing ? 1 : 0

  type        = "zip"
  source_dir  = "${path.module}/lambda-sources/ab-testing"
  output_path = "${path.module}/lambda-sources/ab-testing.zip"

  depends_on = [local_file.ab_testing_lambda_source]
}

# Output Lambda@Edge function ARNs
output "api_auth_lambda_arn" {
  description = "API Authentication Lambda@Edge function ARN"
  value       = var.enable_api_lambda ? aws_lambda_function.api_auth_lambda[0].qualified_arn : null
}

output "image_optimization_lambda_arn" {
  description = "Image Optimization Lambda@Edge function ARN"
  value       = var.enable_image_optimization ? aws_lambda_function.image_optimization_lambda[0].qualified_arn : null
}

output "ab_testing_lambda_arn" {
  description = "A/B Testing Lambda@Edge function ARN"
  value       = var.enable_ab_testing ? aws_lambda_function.ab_testing_lambda[0].qualified_arn : null
}

# Additional variables for Lambda@Edge features
variable "enable_api_lambda" {
  description = "Enable API authentication Lambda@Edge function"
  type        = bool
  default     = true
}

variable "enable_image_optimization" {
  description = "Enable image optimization Lambda@Edge function"
  type        = bool
  default     = true
}

variable "enable_ab_testing" {
  description = "Enable A/B testing Lambda@Edge function"
  type        = bool
  default     = false
}

variable "enable_regional_distributions" {
  description = "Enable regional CDN distributions"
  type        = bool
  default     = true
}