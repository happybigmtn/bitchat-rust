# BitCraps CDN Infrastructure

This directory contains comprehensive CDN deployment configurations and infrastructure for global content distribution of the BitCraps gaming platform.

## 📁 Directory Structure

```
infrastructure/cdn/
├── terraform/              # Infrastructure as Code
│   ├── main.tf             # Primary Terraform configuration
│   ├── variables.tf        # Input variables
│   ├── cloudfront.tf       # AWS CloudFront configuration
│   ├── distributions.tf    # Specialized CDN distributions
│   ├── lambda-edge.tf      # Lambda@Edge functions
│   ├── waf.tf             # Web Application Firewall
│   └── monitoring.tf       # Monitoring and analytics
├── cloudflare/             # Cloudflare configurations
│   ├── workers.js         # Cloudflare Workers edge processing
│   └── wrangler.toml      # Cloudflare Workers configuration
├── fastly/                # Fastly CDN configuration
│   └── vcl/
│       └── main.vcl       # Fastly VCL configuration
├── cloudfront/            # Additional CloudFront configurations
│   └── distributions.tf   # Specialized distributions
├── monitoring/            # Monitoring configurations
│   ├── cloudwatch-dashboard.json    # AWS CloudWatch dashboard
│   ├── grafana-dashboard.json       # Grafana dashboard
│   └── real-user-monitoring.js     # Client-side RUM
└── README.md              # This file
```

## 🚀 Features

### Multi-CDN Setup
- **AWS CloudFront**: Primary CDN with multiple specialized distributions
- **Cloudflare**: Edge processing with Workers for dynamic content
- **Fastly**: High-performance VCL-based configuration

### Content-Specific Optimization
- **WASM Distribution**: Optimized for WebAssembly modules with proper CORS headers
- **API Distribution**: Low-latency API responses with authentication
- **Game Distribution**: Real-time gaming content with WebSocket support
- **Static Assets**: Long-term caching for images, CSS, and JavaScript

### Security Features
- **WAF Protection**: AWS WAF with managed rule sets and custom rules
- **DDoS Protection**: Multi-layer protection across all CDN providers
- **Rate Limiting**: Intelligent rate limiting to prevent abuse
- **Geographic Restrictions**: Configurable geo-blocking capabilities

### Performance Optimization
- **Lambda@Edge Functions**:
  - API authentication and authorization
  - Image optimization and responsive image serving
  - A/B testing infrastructure
- **Compression**: Gzip and Brotli compression for all content types
- **HTTP/2 & HTTP/3**: Latest protocol support for improved performance

### Monitoring & Analytics
- **Real User Monitoring (RUM)**: Client-side performance tracking
- **CloudWatch Dashboards**: Comprehensive CDN metrics visualization
- **Grafana Integration**: Advanced analytics and alerting
- **Custom Metrics**: Game-specific performance indicators

## 🛠 Deployment

### Prerequisites

1. **Required Tools**:
   ```bash
   # Install Terraform
   wget https://releases.hashicorp.com/terraform/1.6.0/terraform_1.6.0_linux_amd64.zip
   unzip terraform_1.6.0_linux_amd64.zip
   sudo mv terraform /usr/local/bin/

   # Install AWS CLI
   curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
   unzip awscliv2.zip
   sudo ./aws/install

   # Install Wrangler (for Cloudflare)
   npm install -g wrangler
   ```

2. **Environment Variables**:
   ```bash
   export CLOUDFLARE_API_TOKEN="your-cloudflare-api-token"
   export CLOUDFLARE_ZONE_ID="your-zone-id"
   export FASTLY_API_KEY="your-fastly-api-key"
   export DOMAIN_NAME="bitcraps.io"
   ```

### Quick Deployment

1. **Deploy CDN Infrastructure**:
   ```bash
   # Navigate to project root
   cd /path/to/bitchat-rust

   # Make deployment script executable
   chmod +x scripts/deploy-cdn.sh

   # Deploy to production
   ENVIRONMENT=prod ./scripts/deploy-cdn.sh
   ```

2. **Build and Upload Assets**:
   ```bash
   # Build optimized assets
   ./scripts/build-assets.sh

   # Assets are automatically uploaded during deployment
   ```

### Manual Deployment

1. **Initialize Terraform**:
   ```bash
   cd infrastructure/cdn/terraform
   terraform init
   terraform workspace new prod  # or staging/dev
   ```

2. **Plan Infrastructure Changes**:
   ```bash
   terraform plan -var-file="prod.tfvars"
   ```

3. **Apply Infrastructure**:
   ```bash
   terraform apply -var-file="prod.tfvars"
   ```

4. **Deploy Cloudflare Workers**:
   ```bash
   cd ../cloudflare
   wrangler deploy --env production
   ```

## ⚙️ Configuration

### Terraform Variables

Create `prod.tfvars`:
```hcl
environment = "prod"
domain_name = "bitcraps.io"
aws_primary_region = "us-west-2"

# CDN subdomains
cdn_subdomains = {
  assets = "assets"
  api    = "api"
  game   = "game"
  wasm   = "wasm"
}

# Cache TTL settings
cache_ttl_settings = {
  static_assets = 86400    # 24 hours
  wasm_files    = 604800   # 7 days
  api_responses = 300      # 5 minutes
  game_data     = 60       # 1 minute
}

# Security
enable_waf = true
enable_ddos_protection = true

# Performance
enable_api_lambda = true
enable_image_optimization = true
enable_ab_testing = false

# Monitoring
monitoring_config = {
  enable_real_time_metrics = true
  enable_enhanced_metrics  = true
  alarm_thresholds = {
    error_rate_threshold     = 5.0
    origin_latency_threshold = 3000
    cache_hit_rate_threshold = 85.0
  }
}
```

### Environment-Specific Configuration

- **Production**: Full feature set with global distribution
- **Staging**: Reduced feature set for testing
- **Development**: Local-friendly configuration

## 📊 Monitoring

### Key Metrics

1. **Performance Metrics**:
   - Cache hit rate (target: >85%)
   - Origin latency (target: <3s)
   - Request rate and bandwidth utilization

2. **Error Metrics**:
   - 4xx/5xx error rates (target: <5%)
   - WAF blocked requests
   - Lambda@Edge function errors

3. **Security Metrics**:
   - DDoS attacks blocked
   - Geographic access patterns
   - Bot traffic detection

### Dashboard Access

- **CloudWatch**: Available in AWS Console after deployment
- **Grafana**: Import dashboard JSON from `monitoring/grafana-dashboard.json`
- **Real User Monitoring**: Automatically injected into web pages

### Alerting

Alerts are sent via SNS to configured endpoints when:
- Error rate exceeds 5%
- Cache hit rate drops below 85%
- Origin latency exceeds 3 seconds
- WAF blocks unusual traffic patterns

## 🔧 Maintenance

### Cache Invalidation

```bash
# Invalidate all CloudFront caches
aws cloudfront create-invalidation --distribution-id DISTRIBUTION_ID --paths "/*"

# Purge Cloudflare cache
curl -X POST "https://api.cloudflare.com/client/v4/zones/$ZONE_ID/purge_cache" \
     -H "Authorization: Bearer $CLOUDFLARE_API_TOKEN" \
     -H "Content-Type: application/json" \
     --data '{"purge_everything":true}'
```

### SSL Certificate Management

SSL certificates are managed through AWS Certificate Manager:
```bash
# Request new certificate
aws acm request-certificate \
    --domain-name "*.bitcraps.io" \
    --subject-alternative-names "bitcraps.io" \
    --validation-method DNS
```

### Log Analysis

CDN access logs are stored in S3 and can be analyzed using:
```bash
# Query CloudFront logs with Amazon Athena
aws athena start-query-execution \
    --query-string "SELECT * FROM cloudfront_logs WHERE status >= 400 LIMIT 100" \
    --result-configuration OutputLocation=s3://query-results-bucket/
```

## 🔐 Security Best Practices

1. **API Keys**: Store in AWS Secrets Manager or similar
2. **IAM Roles**: Use least-privilege principle
3. **Network Security**: Enable AWS Shield Advanced for DDoS protection
4. **Content Security**: Implement proper CSP headers
5. **Access Control**: Use IAM policies for infrastructure access

## 🚨 Troubleshooting

### Common Issues

1. **High Origin Latency**:
   - Check origin server health
   - Verify origin shield configuration
   - Review cache hit rates

2. **SSL Certificate Issues**:
   - Verify certificate validation
   - Check DNS configuration
   - Ensure certificate covers all subdomains

3. **WAF Blocking Legitimate Traffic**:
   - Review WAF logs
   - Adjust rule sensitivity
   - Add whitelist rules for known good IPs

4. **WASM Loading Issues**:
   - Verify CORS headers
   - Check COEP/COOP policies
   - Ensure proper MIME type

### Debug Commands

```bash
# Test CDN endpoints
curl -I https://assets.bitcraps.io/test.js
curl -I https://wasm.bitcraps.io/bitcraps.wasm
curl -I https://api.bitcraps.io/health

# Check DNS resolution
dig assets.bitcraps.io
nslookup wasm.bitcraps.io

# Test from different locations
curl -H "CF-IPCountry: CN" https://bitcraps.io/
```

## 📈 Performance Optimization Tips

1. **Cache Headers**: Set appropriate cache-control headers
2. **Image Optimization**: Use WebP format with fallbacks
3. **WASM Optimization**: Enable compression and proper caching
4. **API Caching**: Cache GET requests where appropriate
5. **Geographic Distribution**: Use all CDN edge locations

## 🤝 Contributing

1. Test changes in development environment first
2. Update documentation for any configuration changes
3. Run `terraform plan` before applying changes
4. Monitor metrics after deployment

## 📄 License

This CDN infrastructure configuration is part of the BitCraps project and follows the same licensing terms.

---

For questions or support, please refer to the main project documentation or create an issue in the project repository.