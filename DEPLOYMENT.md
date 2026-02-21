# Deployment Guide

This document describes how to build and deploy `svg-trmnl-server` using Docker
and Flux CD (GitOps continuous delivery on Kubernetes).

---

## Table of Contents

1. [Docker image](#1-docker-image)
2. [Kubernetes manifests](#2-kubernetes-manifests)
3. [Flux CD setup](#3-flux-cd-setup)
4. [Automated image updates](#4-automated-image-updates)
5. [Configuration reference](#5-configuration-reference)
6. [Exposing the server](#6-exposing-the-server)

---

## 1. Docker image

### Prerequisites

- Docker ≥ 20.10 (BuildKit enabled by default)
- Container registry access (e.g. GitHub Container Registry `ghcr.io`)

### Build

```bash
docker build -t ghcr.io/<YOUR_ORG>/svg-trmnl-server:0.1.0 .
```

The image uses a two-stage build:

| Stage   | Base image               | Purpose                         |
|---------|--------------------------|---------------------------------|
| builder | `rust:1.85-bookworm`     | Compiles the release binary     |
| runtime | `debian:bookworm-slim`   | Minimal image with the binary   |

Dependency caching is split into a separate layer so that only changed
source files trigger a full recompile.

### Run locally

```bash
# Create a config file (copy and edit the example)
cp config/config.example.toml config/config.toml

docker run --rm \
  -p 9080:9080 \
  -v "$(pwd)/config:/app/config:ro" \
  ghcr.io/<YOUR_ORG>/svg-trmnl-server:0.1.0
```

The server listens on `http://localhost:9080`.

### Push to registry

```bash
docker push ghcr.io/<YOUR_ORG>/svg-trmnl-server:0.1.0
# Also tag as latest for Flux image automation
docker tag ghcr.io/<YOUR_ORG>/svg-trmnl-server:0.1.0 \
           ghcr.io/<YOUR_ORG>/svg-trmnl-server:latest
docker push ghcr.io/<YOUR_ORG>/svg-trmnl-server:latest
```

---

## 2. Kubernetes manifests

All Kubernetes resources live under `deploy/`.

```
deploy/
├── kustomization.yaml           # Kustomize root – applies everything below
├── namespace.yaml               # Namespace: trmnl
├── configmap.yaml               # config.toml mounted into the pod
├── deployment.yaml              # Deployment (1 replica by default)
├── service.yaml                 # ClusterIP Service on port 9080
├── gitrepository.yaml           # Flux: watches this Git repo
├── imagerepository.yaml         # Flux: polls container registry for new tags
├── imagepolicy.yaml             # Flux: semver policy for tag selection
├── imageupdateautomation.yaml   # Flux: commits updated image tags to Git
└── flux-kustomization.yaml      # Flux: applies deploy/ from GitRepository
```

Before applying, replace every occurrence of `<YOUR_ORG>` with your GitHub
organisation or registry path:

```bash
grep -r '<YOUR_ORG>' deploy/
# e.g. replace with "myorg"
sed -i 's|<YOUR_ORG>|myorg|g' deploy/*.yaml
```

---

## 3. Flux CD setup

### Prerequisites

- Kubernetes cluster (1.26+)
- `kubectl` configured for the cluster
- [Flux CLI](https://fluxcd.io/flux/installation/) (`flux` binary on PATH)
- GitHub personal access token with `repo` scope (for bootstrapping)

### 3.1 Bootstrap Flux

If Flux is not yet installed on the cluster, bootstrap it from your repository:

```bash
export GITHUB_TOKEN=<your-github-token>

flux bootstrap github \
  --owner=<YOUR_ORG> \
  --repository=svg-trmnl-server \
  --branch=main \
  --path=deploy \
  --personal          # omit if this is an organisation repo
```

Flux will:
1. Install its controllers into `flux-system`.
2. Create a deploy key on the repository.
3. Commit `flux-system/` manifests back to `main`.

### 3.2 Apply the GitRepository and Flux Kustomization

If Flux is already installed, register the repository and kustomization:

```bash
kubectl apply -f deploy/gitrepository.yaml
kubectl apply -f deploy/flux-kustomization.yaml
```

Flux will reconcile `deploy/` from Git within the configured `interval`
(default: 5 minutes).

### 3.3 Verify reconciliation

```bash
# Watch Flux Kustomization status
flux get kustomizations -n flux-system

# Watch the Deployment roll out
kubectl rollout status deployment/svg-trmnl-server -n trmnl

# Tail application logs
kubectl logs -n trmnl -l app=svg-trmnl-server -f
```

---

## 4. Automated image updates

Flux Image Automation keeps the running image in sync with newly pushed tags.

### 4.1 Enable the image-automation and image-reflector controllers

These are not installed by the default `flux bootstrap`. Enable them:

```bash
flux install \
  --components-extra=image-reflector-controller,image-automation-controller
```

### 4.2 (Private registry) Create pull credentials

```bash
kubectl create secret docker-registry ghcr-credentials \
  --namespace=trmnl \
  --docker-server=ghcr.io \
  --docker-username=<YOUR_ORG> \
  --docker-password=<GITHUB_TOKEN>
```

Then uncomment the `secretRef` block in `deploy/imagerepository.yaml`.

### 4.3 Apply image automation objects

```bash
kubectl apply -f deploy/imagerepository.yaml
kubectl apply -f deploy/imagepolicy.yaml
kubectl apply -f deploy/imageupdateautomation.yaml
```

### 4.4 Mark the image field for automation

Add the `# {"$imagepolicy": "trmnl:svg-trmnl-server"}` marker comment to
`deploy/deployment.yaml` so the automation controller knows which field to
update:

```yaml
          image: ghcr.io/<YOUR_ORG>/svg-trmnl-server:0.1.0 # {"$imagepolicy": "trmnl:svg-trmnl-server"}
```

### 4.5 Check automation status

```bash
flux get image repository svg-trmnl-server -n trmnl
flux get image policy    svg-trmnl-server -n trmnl
flux get image update    svg-trmnl-server -n trmnl
```

Every time you push a new semver-tagged image (e.g. `0.1.1`), Flux will
detect it, update `deploy/deployment.yaml` in Git, and roll out the new pod
automatically.

---

## 5. Configuration reference

Edit `deploy/configmap.yaml` to adjust the server configuration. Key fields:

| Field                   | Description                                              |
|-------------------------|----------------------------------------------------------|
| `base_url`              | Publicly reachable URL of this server (used in API responses returned to devices) |
| `templates_path`        | Path inside the container to Jinja templates (`/app/templates`) |
| `fonts_path`            | Path inside the container to fonts (`/app/fonts`)        |
| `devices[].mac_address` | MAC address of the TRMNL device                          |
| `devices[].api_key`     | Secret key the device presents in `Access-Token` header  |

**Sensitive values** (api keys, etc.) should be stored in a Kubernetes
`Secret` and mounted as a file or injected via an init container that writes
`config.toml` at pod start, rather than putting them in the ConfigMap
directly.

---

## 6. Exposing the server

The `Service` is `ClusterIP`-only. Choose one of the following to make the
server reachable from TRMNL devices on your network:

### Option A – Ingress (recommended for cloud clusters)

Create an `Ingress` resource pointing to `svg-trmnl-server:9080` and
configure your Ingress controller (nginx, traefik, etc.) accordingly.

### Option B – NodePort

Change `spec.type` in `deploy/service.yaml` to `NodePort` and set a fixed
`nodePort` (30000–32767):

```yaml
  type: NodePort
  ports:
    - name: http
      port: 9080
      targetPort: http
      nodePort: 30080
```

### Option C – LoadBalancer

Change `spec.type` to `LoadBalancer` to request an external IP from the
cloud provider.

### Option D – port-forward (local testing only)

```bash
kubectl port-forward -n trmnl svc/svg-trmnl-server 9080:9080
```
