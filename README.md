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

🚀 **Phase 2: Upcoming**
- HTTP/gRPC gateway with request routing
- Load balancing (round-robin, least-connections, consistent hash)
- Health checking and failover
- TLS termination for VPCIngress

🔮 **Phase 3: Future**
- Traffic policies (timeouts, retries, circuit breaking)
- mTLS between services
- Observability (metrics, tracing, logging)
- Geographic routing based on Location CRD
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

## Project Structure

```
router/
├── Cargo.toml (workspace)
├── bin/
│   ├── router-controller/     # CRD controllers (VPCService, VPCRoute, etc.)
│   ├── router-gateway/        # Layer 7 HTTP/gRPC gateway
│   ├── service-discovery/     # Cross-VPC service discovery daemon
│   └── tunnel-gateway/        # Iroh tunnel termination (optional)
├── lib/
│   ├── router-api/           # CRD types and Galactic VPC bindings
│   ├── router-core/          # Service registry, endpoint management
│   ├── router-galactic/      # Galactic VPC integration
│   ├── router-proxy/         # HTTP/gRPC proxy implementation
│   └── router-tunnel/        # Tunnel management
├── manifests/
│   ├── crds/                 # Kubernetes CRD definitions
│   ├── rbac/                 # Service account and RBAC
│   ├── deployments/          # Controller and gateway deployments
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

2. **Deploy the router controller and service discovery:**

```bash
kubectl apply -f manifests/deployments/router-controller.yaml
```

3. **Deploy an example VPCService and VPCRoute:**

```bash
kubectl apply -f manifests/examples/basic-example.yaml
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

## Development

### Running Tests

```bash
cargo test
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
- [x] CRD definitions
- [x] Galactic VPC integration
- [x] Service registry
- [x] Router controller
- [x] Service discovery daemon
- [x] Kubernetes manifests
- [x] Example configurations

### Phase 2: In Progress 🚀
- [ ] HTTP/gRPC gateway implementation
- [ ] Request routing logic
- [ ] Load balancing algorithms
- [ ] Health checking
- [ ] TLS termination

### Phase 3: Planned 🔮
- [ ] Traffic policies
- [ ] Circuit breaking
- [ ] mTLS between services
- [ ] Observability (metrics, traces)
- [ ] Geographic routing
- [ ] Iroh P2P tunnel support

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
