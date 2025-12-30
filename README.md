# Datum Router - Layer 7 Application Router for Galactic VPC

A Rust-based Layer 7 application router that sits above Galactic VPC's Layer 3 SRv6 overlay network. The router provides service discovery, HTTP/gRPC routing, and traffic management across multi-cloud Kubernetes clusters.

## Features

✅ **Phase 1: Complete**
- Kubernetes CRDs for VPCService, VPCRoute, ServiceBinding, VPCIngress, VPCEgress
- Galactic VPC integration and service discovery
- Service registry with endpoint management
- Router controller for CRD reconciliation
- Service discovery daemon for cross-VPC discovery
- Full Kubernetes manifests (CRDs, RBAC, deployments)

✅ **Phase 2: Complete**
- HTTP/1.1 gateway with request routing (`router-gateway` binary)
- Load balancing with 4 strategies (round-robin, least-connections, source-IP hash, consistent hash)
- VPCRoute controller with path/header/method matching
- VPCIngress controller for external ingress management
- Multi-controller orchestration in router-controller
- Router module with path matching (exact, prefix, wildcard)
- Service endpoint discovery and selection
- VPCAttachment integration for Galactic VPC connectivity
- Full deployment manifests with HA configuration

✅ **Phase 3: Complete**
- Health checking framework with configurable intervals and thresholds
- Traffic policies (timeouts, retries with exponential backoff, circuit breaker)
- Circuit breaker pattern (Closed/Open/HalfOpen states)
- HTTP request/response body forwarding infrastructure
- Request forwarder with timeout protection
- Integrated into router-gateway startup with detailed logging
- Full unit tests for all policies and health checks

🚀 **Phase 4: Upcoming**
- Actual HTTP client forwarding with connection pooling
- TLS termination for HTTPS
- Request/response middleware support
- Prometheus metrics and observability
- mTLS between services

🔮 **Phase 5: Future**
- Geographic routing based on Location CRD
- Advanced load balancing (weighted, sticky sessions)
- Iroh P2P tunnels for non-VPC connectivity

## Architecture

```
┌─────────────────────────────────────────┐
│ External Client / Service Consumer      │
└────────────────┬────────────────────────┘
                 │ HTTP/gRPC
                 ▼
┌─────────────────────────────────────────┐
│ router-gateway (Layer 7)                │
│ - Receives requests                     │
│ - Routes to VPCService                  │
│ - VPCAttachment: galactic0              │
└────────────────┬────────────────────────┘
                 │ HTTP to VPC IP
                 ▼
┌─────────────────────────────────────────┐
│ Galactic VPC (Layer 3 - SRv6)           │
│ - Transparent packet routing            │
│ - Cross-cloud/cloud-agnostic            │
└────────────────┬────────────────────────┘
                 │ SRv6 over IPv6
                 ▼
┌─────────────────────────────────────────┐
│ Backend Service (Different Cloud)       │
│ - Receives routed request               │
└─────────────────────────────────────────┘
```

## Custom Resources

### VPCService
Represents a service running inside a Galactic VPC that should be discoverable across VPCs.

```yaml
apiVersion: router.datum.net/v1alpha1
kind: VPCService
metadata:
  name: my-api
  namespace: default
spec:
  vpcAttachmentRef:
    name: vpc-attachment-1
  protocol: HTTP
  port: 8080
  healthCheck:
    httpPath: /healthz
    intervalSeconds: 10
```

### VPCRoute
Defines Layer 7 routing rules for traffic between VPCs or from external clients.

```yaml
apiVersion: router.datum.net/v1alpha1
kind: VPCRoute
metadata:
  name: api-routes
  namespace: default
spec:
  match:
    pathPrefix: /api/v1
    methods: [GET, POST]
  destinations:
    - vpcServiceRef:
        name: my-api
      weight: 100
  loadBalancing: round-robin
```

### ServiceBinding
Binds a Kubernetes Service to a VPCService for automatic endpoint synchronization.

```yaml
apiVersion: router.datum.net/v1alpha1
kind: ServiceBinding
metadata:
  name: api-binding
  namespace: default
spec:
  serviceRef:
    name: api-service
  vpcServiceRef:
    name: my-api
  portMappings:
    - servicePort: 8080
      vpcPort: 8080
  autoSync: true
```

### VPCIngress
Defines external ingress into VPC networks via the router-gateway.

### VPCEgress
Controls outbound traffic from VPCs to external services.

## Router Gateway

The `router-gateway` is the Layer 7 HTTP/1.1 gateway that:

### Features
- **HTTP/1.1 Server**: Listens on port 8080 with Hyper 1.0 async architecture
- **Request Routing**: Matches incoming HTTP requests against VPCRoute resources
- **Path Matching**: Supports exact, prefix, and wildcard path matching:
  - Exact: `/api/v1/users` matches only `/api/v1/users`
  - Prefix: `/api/v1/` matches `/api/v1/users`, `/api/v1/posts`, etc.
  - Wildcard: `/api/v1/*` matches anything under `/api/v1/`
- **HTTP Methods**: Supports GET, POST, PUT, DELETE, PATCH, OPTIONS, and custom methods
- **Header Matching**: Can match on HTTP headers (prepared for Phase 3)
- **Load Balancing**: 4 strategies for endpoint selection:
  - **Round-Robin**: Evenly distribute traffic across all endpoints
  - **Least Connections**: Route to endpoint with fewest active connections
  - **Source IP Hash**: Sticky sessions - same client always routes to same endpoint
  - **Consistent Hash**: Hash-based routing for distributed caching

### Request Flow
```
Client Request
    ↓
router-gateway (/healthz → 200 OK)
    ↓
VPCRoute Match (path/method/headers)
    ↓
Load Balancer Selection (pick endpoint)
    ↓
Service Registry Lookup (get IP:port)
    ↓
Galactic VPC (Layer 3 transparent routing)
    ↓
Backend Service Response
```

### Configuration
Gateway behavior is controlled via:
- **ConfigMap**: Default timeouts, load balancing strategy
- **VPCRoute Resources**: Dynamic routing rules created by users
- **VPCService Resources**: Backend service definitions
- **Environment Variables**: Logging level via `RUST_LOG`

## Project Structure

```
router/
├── Cargo.toml (workspace)
├── bin/
│   ├── router-controller/           # Multi-controller orchestration
│   │   ├── vpc_service_controller.rs # VPCService reconciliation
│   │   ├── vpc_route_controller.rs   # VPCRoute reconciliation
│   │   └── vpc_ingress_controller.rs # VPCIngress reconciliation (Phase 2)
│   ├── router-gateway/              # Layer 7 HTTP/1.1 gateway (Phase 2)
│   │   ├── main.rs                  # HTTP server and request handling
│   │   └── router.rs                # Path/method matching logic
│   ├── service-discovery/           # Cross-VPC service discovery daemon
│   └── tunnel-gateway/              # Iroh tunnel termination (optional)
├── lib/
│   ├── router-api/           # CRD types and Galactic VPC bindings
│   ├── router-core/          # Service registry, endpoint management
│   ├── router-galactic/      # Galactic VPC integration
│   ├── router-proxy/         # HTTP proxy + load balancing (Phase 2)
│   │   ├── http.rs           # HTTP proxy implementation
│   │   └── load_balancer.rs  # 4 load balancing strategies
│   └── router-tunnel/        # Tunnel management
├── manifests/
│   ├── crds/                 # Kubernetes CRD definitions
│   ├── rbac/                 # Service account and RBAC
│   ├── deployments/
│   │   ├── router-controller.yaml  # Controllers + service-discovery
│   │   └── router-gateway.yaml     # Gateway deployment (Phase 2)
│   └── examples/             # Example VPCService, VPCRoute, etc.
└── docs/
```

## Getting Started

### Prerequisites

- Kubernetes 1.24+
- Galactic VPC deployed and configured
- Rust 1.70+ (for building from source)

### Installation

1. **Deploy the CRDs and RBAC:**

```bash
kubectl apply -f manifests/crds/
kubectl apply -f manifests/rbac/rbac.yaml
```

2. **Deploy the router controllers (VPCService, VPCRoute, VPCIngress) and service discovery:**

```bash
kubectl apply -f manifests/deployments/router-controller.yaml
```

3. **Deploy the router-gateway (Layer 7 HTTP gateway):**

```bash
kubectl apply -f manifests/deployments/router-gateway.yaml
```

4. **Deploy example VPCServices and VPCRoutes:**

```bash
kubectl apply -f manifests/examples/basic-example.yaml
```

### Gateway Setup

The `router-gateway` deployment includes:
- **2 replicas** for high availability
- **Service ClusterIP** on port 8080 for internal routing
- **VPCAttachment integration** via annotation `galactic.datumapis.com/vpc: "default"` - automatically joins pods to Galactic VPC
- **Health check endpoints**: `/healthz` for liveness/readiness probes
- **Resource limits**: 200m-1000m CPU, 256Mi-1Gi memory
- **Security context**: Non-root user with read-only filesystem
- **Pod anti-affinity**: Spread replicas across nodes

To customize the gateway (e.g., different VPC, port, replicas):

```bash
# Edit the deployment
kubectl edit deployment router-gateway -n datum-router

# Or patch specific values
kubectl patch deployment router-gateway -n datum-router \
  -p '{"spec":{"replicas":3}}'
```

### Building from Source

```bash
# Build all components
cargo build --release

# Build individual components
cargo build --release -p router-controller
cargo build --release -p service-discovery
cargo build --release -p router-gateway
```

## Usage Examples

### Create a VPCService

```yaml
apiVersion: router.datum.net/v1alpha1
kind: VPCService
metadata:
  name: backend-api
  namespace: production
spec:
  vpcAttachmentRef:
    name: prod-vpc
  protocol: HTTP
  port: 8080
  healthCheck:
    httpPath: /health
    intervalSeconds: 10
```

### Create a VPCRoute

```yaml
apiVersion: router.datum.net/v1alpha1
kind: VPCRoute
metadata:
  name: api-routing
  namespace: production
spec:
  match:
    pathPrefix: /api
  destinations:
    - vpcServiceRef:
        name: backend-api
  loadBalancing: round-robin
  retries:
    maxRetries: 3
    retryOnStatus: [502, 503, 504]
```

### Bind a Kubernetes Service

```yaml
apiVersion: router.datum.net/v1alpha1
kind: ServiceBinding
metadata:
  name: api-binding
  namespace: production
spec:
  serviceRef:
    name: api-backend
  vpcServiceRef:
    name: backend-api
  autoSync: true
```

## Integration with Galactic VPC

The router seamlessly integrates with Galactic VPC:

1. **Automatic VPC Discovery**: Discovers VPCs and VPCAttachments from the cluster
2. **Transparent Layer 3 Routing**: Galactic VPC handles all SRv6 encapsulation and packet routing
3. **Cross-Cloud Connectivity**: Services can communicate across clouds without configuration

Example: A request to `/api/v1/users` can be routed to a backend service running in a completely different cloud:
- Request enters via router-gateway in AWS
- VPCRoute matches `/api/v1` prefix
- Request routed to backend VPCService IP (which could be in GCP, Azure, or on-prem)
- Galactic VPC's SRv6 transparently handles the cross-cloud routing

## Load Balancing Strategies

The `router-gateway` supports 4 load balancing strategies for distributing traffic across backend endpoints:

### Round-Robin (Default)
Distributes requests evenly across all healthy endpoints in a circular pattern.
```
Request 1 → Endpoint 1
Request 2 → Endpoint 2
Request 3 → Endpoint 3
Request 4 → Endpoint 1 (circle back)
```
**Use case**: General-purpose traffic distribution, works well with stateless services

### Least Connections
Routes each request to the endpoint with the fewest active connections.
```
Endpoint 1: 5 connections
Endpoint 2: 3 connections ← New request goes here
Endpoint 3: 4 connections
```
**Use case**: Services with long-lived connections or varying request durations

### Source IP Hash
Uses a hash of the client's source IP to select an endpoint.
```
Client A (IP: 10.0.0.5) → Hash → Endpoint 2 (always)
Client B (IP: 10.0.0.6) → Hash → Endpoint 1 (always)
Client C (IP: 10.0.0.7) → Hash → Endpoint 3 (always)
```
**Use case**: Sticky sessions, maintaining client affinity for stateful applications

### Consistent Hash
Uses a hash key to select endpoints in a way that minimizes remapping on endpoint changes.
```
Hash Key: "/api/v1/users/123"
Consistent Hash → Endpoint 2
(If Endpoint 2 fails, only requests with similar hash keys remap)
```
**Use case**: Cache-like scenarios where maintaining mapping is important

Configure load balancing strategy in VPCRoute:
```yaml
spec:
  loadBalancing: round-robin  # Can also be: least-connections, source-ip-hash, consistent-hash
```

## Development

### Running Tests

```bash
cargo test
```

### Testing the Router Module

The `router` module includes comprehensive unit tests for path and method matching:

```bash
# Run tests for the router module
cargo test router::

# Specific tests
cargo test router::test_exact_path_match
cargo test router::test_prefix_path_match
cargo test router::test_wildcard_path_match
cargo test router::test_method_match
```

### Building Docker Images

```bash
# Build router-controller image
docker build -f Dockerfile.controller -t datum-router-controller:latest .

# Build router-gateway image
docker build -f Dockerfile.gateway -t datum-router-gateway:latest .

# Build service-discovery image
docker build -f Dockerfile.discovery -t datum-router-service-discovery:latest .
```

### Logging

Set `RUST_LOG` environment variable:

```bash
# Debug logging
RUST_LOG=router=debug,kube_runtime=debug cargo run

# Info logging
RUST_LOG=router=info cargo run
```

## Configuration

### Environment Variables

- `RUST_LOG`: Log level (trace, debug, info, warn, error)
- `DISCOVERY_INTERVAL_SECS`: Service discovery interval (default: 30 seconds)
- `KUBECONFIG`: Path to Kubernetes config (optional, uses in-cluster auth by default)

## Status and Roadmap

### Phase 1: Complete ✅
- [x] CRD definitions (VPCService, VPCRoute, ServiceBinding, VPCIngress, VPCEgress)
- [x] Galactic VPC integration and discovery
- [x] Service registry with endpoint management
- [x] Router controller with reconciliation framework
- [x] Service discovery daemon for cross-VPC discovery
- [x] Kubernetes manifests (CRDs, RBAC, deployments)
- [x] Example configurations

### Phase 2: Complete ✅
- [x] HTTP/1.1 gateway server (`router-gateway` binary)
- [x] Request routing and path/header/method matching
- [x] Load balancing (4 strategies: round-robin, least-connections, source-IP hash, consistent hash)
- [x] VPCRoute controller with full reconciliation
- [x] VPCIngress controller for external ingress
- [x] Router module with configurable matching logic
- [x] Service endpoint discovery and selection
- [x] VPCAttachment integration for VPC networking
- [x] Multi-controller orchestration in router-controller
- [x] Deployment manifests with HA configuration (2+ replicas)

### Phase 3: Complete ✅
- [x] Health checking framework with configurable intervals
- [x] Traffic policies (timeouts, retries with exponential backoff)
- [x] Circuit breaker with 3-state pattern (Closed/Open/HalfOpen)
- [x] HTTP request/response body forwarding infrastructure
- [x] Request forwarder with timeout protection
- [x] Full unit tests for all policies and state transitions
- [x] Integrated into router-gateway with startup logging

### Phase 4: Upcoming 🚀
- [ ] Actual HTTP client forwarding with hyper
- [ ] Connection pooling and connection reuse
- [ ] TLS termination for HTTPS/VPCIngress
- [ ] Request/response middleware support
- [ ] Prometheus metrics and observability
- [ ] mTLS between services

### Phase 5: Future 🔮
- [ ] Geographic routing based on Location CRD
- [ ] Advanced load balancing (weighted, sticky sessions)
- [ ] Iroh P2P tunnel support for non-VPC connectivity
- [ ] Advanced observability (distributed tracing)

## Contributing

Contributions are welcome! Please see CONTRIBUTING.md for guidelines.

## License

Apache License 2.0

## Support

For issues, questions, or suggestions, please open an issue on GitHub or contact the Datum team.

---

**Architecture Documentation**: See [ARCHITECTURE.md](docs/architecture.md)  
**Galactic VPC**: See [../galactic-operator](../galactic-operator)  
**Network Services Operator**: See [../network-services-operator](../network-services-operator)  
