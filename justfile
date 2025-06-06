NAME := "cluster-api-addon-provider-fleet"
KUBE_VERSION := env_var_or_default('KUBE_VERSION', '1.32.0')
ORG := "ghcr.io/rancher-sandbox"
TAG := "dev"
HOME_DIR := env_var('HOME')
YQ_VERSION := "v4.43.1"
CLUSTERCTL_VERSION := "v1.9.5"
OUT_DIR := "_out"
KUSTOMIZE_VERSION := "v5.4.1"
ARCH := if arch() == "aarch64" { "arm64"} else { "amd64" }
DIST := os()
REFRESH_BIN := env_var_or_default('REFRESH_BIN', '1')

export PATH := "_out:_out/bin:" + env_var('PATH')

[private]
default:
    @just --list --unsorted --color=always

# Generates stuff
generate features="":
    just generate-addon-crds {{features}}

[private]
_generate-kopium-url kpath="" source="" dest="" yqexp="." condition="":
    curl -sSL {{source}} | yq '{{yqexp}}' | {{kpath}} -D Default {{condition}} -A -d -f - > {{dest}}

generate-addon-crds features="":
    cargo run --features={{features}} --bin crdgen > config/crds/fleet-addon-config.yaml

# run with opentelemetry
run-telemetry:
    OTEL_EXPORTER_OTLP_LOGS_ENDPOINT=http://127.0.0.1:55680 RUST_LOG=info,kube=trace,controller=debug cargo run --features=telemetry

# run without opentelemetry
run:
    RUST_LOG=info,kube=debug,controller=debug cargo run

# format with nightly rustfmt
fmt:
    cargo +nightly fmt

# run unit tests
test-unit:
  cargo test

# run clippy
clippy: fmt
  cargo clippy --all-targets --all-features --fix --allow-dirty -- -W clippy::pedantic

# compile for musl (for docker image)
compile features="":  _create-out-dir
  #!/usr/bin/env bash
  docker run --rm \
    -v cargo-cache:/root/.cargo \
    -v $PWD:/volume \
    -w /volume \
    -t clux/muslrust:stable \
    cargo build --release --features={{features}} --bin controller
  cp target/{{arch()}}-unknown-linux-musl/release/controller {{OUT_DIR}}/controller

[private]
_build features="":
  docker buildx build -t {{ORG}}/{{NAME}}:{{TAG}} .

# docker build base
build-base: (_build "")

# docker build base with agent initiated
build-agent-initiated: (_build "agent-initiated")

# docker build with telemetry
build-otel: (_build "telemetry")

# Build  docker image
docker-build:
    docker buildx build . -t {{ORG}}/{{NAME}}:{{TAG}}

# Push the docker images
docker-push:
    docker push {{ORG}}/{{NAME}}:{{TAG}}

build-and-load:
    docker build . -t {{ORG}}/{{NAME}}:{{TAG}}
    kind load docker-image {{ORG}}/{{NAME}}:{{TAG}} --name dev

load-base features="":
    just _build {{features}}
    kind load docker-image {{ORG}}/{{NAME}}:{{TAG}} --name dev

# Start local dev environment
start-dev: _cleanup-out-dir _create-out-dir _download-kubectl
    just update-helm-repos
    kind delete cluster --name dev || true
    kind create cluster --image=kindest/node:v{{KUBE_VERSION}} --config testdata/kind-config.yaml
    just install-capi
    kubectl wait pods --for=condition=Ready --timeout=500s --all --all-namespaces

# Stop the local dev environment
stop-dev:
    kind delete cluster --name dev || true

# Deploy CRS to dev cluster
deploy-kindnet:
    kubectl --context kind-dev apply -f testdata/cni.yaml

deploy-calico:
    kubectl --context kind-dev apply -f testdata/helm.yaml -n clusterclass

deploy-calico-gitrepo: _download-yq
    #!/usr/bin/env bash
    set -euxo pipefail
    repo=`git remote get-url origin`
    branch=`git branch --show-current`
    cp testdata/gitrepo-calico.yaml {{OUT_DIR}}/gitrepo-calico.yaml
    yq -i ".spec.repo = \"${repo}\"" {{OUT_DIR}}/gitrepo-calico.yaml
    yq -i ".spec.branch = \"${branch}\"" {{OUT_DIR}}/gitrepo-calico.yaml
    kubectl apply -f {{OUT_DIR}}/gitrepo-calico.yaml

# Deploy an example app bundle to the cluster
deploy-app:
    kubectl --context kind-dev apply -f testdata/bundle.yaml

# Deploy child cluster using docker & kubeadm
deploy-child-cluster:
    kind delete cluster --name docker-demo || true
    kubectl --context kind-dev apply -f testdata/cluster_docker_kcp.yaml

# Deploy child cluster using docker & rke2
deploy-child-rke2-cluster:
    kind delete cluster --name docker-demo || true
    kubectl --context kind-dev apply -f testdata/cluster_docker_rke2.yaml

# Deploy child cluster-call based cluster using docker & kubeadm
deploy-child-cluster-class:
    kind delete cluster --name capi-quickstart || true
    kubectl --context kind-dev apply -f testdata/capi-quickstart.yaml

# Add and update helm repos used
update-helm-repos:
    helm repo add fleet https://rancher.github.io/fleet-helm-charts/
    helm repo update

# Install fleet into the k8s cluster
install-fleet: _create-out-dir
    #!/usr/bin/env bash
    set -euxo pipefail
    helm -n cattle-fleet-system install --create-namespace --wait fleet-crd fleet/fleet-crd
    helm install --create-namespace -n cattle-fleet-system --set bootstrap.enabled=false fleet fleet/fleet --wait

# Install cluster api and any providers
install-capi: _download-clusterctl
    EXP_CLUSTER_RESOURCE_SET=true CLUSTER_TOPOLOGY=true clusterctl init -i docker -b rke2 -c rke2 -b kubeadm -c kubeadm

# Deploy will deploy the operator
deploy features="": _download-kustomize
    just generate {{features}}
    just build-and-load
    kustomize build config/default | kubectl apply -f -
    kubectl -n caapf-system rollout restart deployment/caapf-controller-manager
    kubectl --context kind-dev apply -f testdata/config.yaml
    kubectl wait fleetaddonconfigs fleet-addon-config --timeout=150s --for=condition=Ready=true

undeploy: _download-kustomize
    kustomize build config/default | kubectl delete --ignore-not-found=true -f -

release-manifests: _create-out-dir _download-kustomize
    kustomize build config/default > {{OUT_DIR}}/addon-components.yaml

# Full e2e test of importing cluster in fleet
test-import: start-dev deploy deploy-child-cluster deploy-kindnet deploy-app && collect-test-import
    kubectl wait pods --for=condition=Ready --timeout=150s --all --all-namespaces
    kubectl wait cluster --timeout=500s --for=condition=ControlPlaneReady=true docker-demo
    kubectl wait clusters.fleet.cattle.io --timeout=500s --for=condition=Ready=true docker-demo

# Full e2e test of importing cluster in fleet
test-import-rke2: start-dev deploy deploy-child-rke2-cluster deploy-calico-gitrepo deploy-app
    kubectl wait pods --for=condition=Ready --timeout=150s --all --all-namespaces
    kubectl wait cluster --timeout=500s --for=condition=ControlPlaneReady=true docker-demo
    kubectl wait clusters.fleet.cattle.io --timeout=500s --for=condition=Ready=true docker-demo

collect-test-import:
    -just collect-artifacts dev
    -just collect-artifacts docker-demo

# Full e2e test of importing cluster in fleet
test-cluster-class-import: start-dev deploy deploy-child-cluster-class deploy-calico deploy-app _test-import-all _test-delete-all && collect-test-cluster-class-import

collect-test-cluster-class-import:
    -just collect-artifacts dev
    -just collect-artifacts capi-quickstart

# Test e2e with agent initiated connection procedure
test-cluster-class-import-agent-initated: start-dev && collect-test-cluster-class-import
    just deploy "agent-initiated"
    just deploy-child-cluster-class
    just deploy-kindnet
    just deploy-app
    just _test-import-all
    just _test-delete-all

collect-artifacts cluster:
    kind get kubeconfig --name {{cluster}} > {{OUT_DIR}}/kubeconfig
    just crust-gather collect -f {{OUT_DIR}}/gather/{{cluster}} -k {{OUT_DIR}}/kubeconfig

# Full e2e test of importing cluster and ClusterClass in fleet
[private]
_test-import-all:
    kubectl wait clustergroups.fleet.cattle.io -n clusterclass --timeout=500s --for=create quick-start
    kubectl wait clustergroups.fleet.cattle.io -n clusterclass --timeout=500s --for=condition=Ready=true quick-start
    # Verify that cluster group created for cluster referencing clusterclass in a different namespace
    kubectl wait bundlenamespacemappings.fleet.cattle.io --timeout=500s --for=create -n clusterclass default
    kubectl wait clustergroups.fleet.cattle.io --timeout=500s --for=create quick-start.clusterclass
    kubectl wait clustergroups.fleet.cattle.io --timeout=500s --for=jsonpath='{.status.clusterCount}=1' quick-start.clusterclass
    kubectl wait clustergroups.fleet.cattle.io --timeout=500s --for=condition=Ready=true quick-start.clusterclass
    kubectl wait clusters.fleet.cattle.io --timeout=500s --for=create capi-quickstart
    kubectl wait clusters.fleet.cattle.io --timeout=500s --for=condition=Ready=true capi-quickstart

[private]
_test-delete-all:
    # Verify that deleting everything causes full re-import
    kubectl delete clustergroups.fleet.cattle.io quick-start.clusterclass --wait
    kubectl delete clustergroups.fleet.cattle.io -n clusterclass quick-start --wait
    kubectl delete bundlenamespacemappings.fleet.cattle.io -n clusterclass default --wait
    kubectl delete clusters.fleet.cattle.io capi-quickstart --wait
    just _test-import-all

# Install kopium
[private]
_install-kopium:
    #!/usr/bin/env bash
    set -euxo pipefail
    [ -z `which kopium` ] || [ {{REFRESH_BIN}} != "0" ] || exit 0
    cargo install --git https://github.com/kube-rs/kopium.git --root {{OUT_DIR}}

download-kustomize: _download-kustomize

# Download kustomize
[private]
[linux]
[macos]
_download-kustomize:
    #!/usr/bin/env bash
    set -euxo pipefail
    [ -z `which kustomize` ] || [ {{REFRESH_BIN}} != "0" ] || exit 0
    curl -sSL https://github.com/kubernetes-sigs/kustomize/releases/download/kustomize/{{KUSTOMIZE_VERSION}}/kustomize_{{KUSTOMIZE_VERSION}}_{{DIST}}_{{ARCH}}.tar.gz -o {{OUT_DIR}}/kustomize.tar.gz
    tar -xzf {{OUT_DIR}}/kustomize.tar.gz -C {{OUT_DIR}}
    chmod +x {{OUT_DIR}}/kustomize

[private]
[linux]
_download-clusterctl:
    #!/usr/bin/env bash
    set -euxo pipefail
    [ -z `which clusterctl` ] || [ {{REFRESH_BIN}} != "0" ] || exit 0
    curl -L https://github.com/kubernetes-sigs/cluster-api/releases/download/{{CLUSTERCTL_VERSION}}/clusterctl-linux-{{ARCH}} -o {{OUT_DIR}}/clusterctl
    chmod +x {{OUT_DIR}}/clusterctl

[private]
[macos]
_download-clusterctl:
    #!/usr/bin/env bash
    set -euxo pipefail
    [ -z `which clusterctl` ] || [ {{REFRESH_BIN}} != "0" ] || exit 0
    curl -L https://github.com/kubernetes-sigs/cluster-api/releases/download/{{CLUSTERCTL_VERSION}}/clusterctl-darwin-{{ARCH}} -o {{OUT_DIR}}/clusterctl
    chmod +x {{OUT_DIR}}/clusterctl

# Download yq
[private]
[linux]
_download-yq:
    #!/usr/bin/env bash
    set -euxo pipefail
    [ -z `which yq` ] || [ {{REFRESH_BIN}} != "0" ] || exit 0
    curl -sSL https://github.com/mikefarah/yq/releases/download/{{YQ_VERSION}}/yq_linux_{{ARCH}} -o {{OUT_DIR}}/yq
    chmod +x {{OUT_DIR}}/yq

[private]
[macos]
_download-yq:
    #!/usr/bin/env bash
    set -euxo pipefail
    [ -z `which yq` ] || [ {{REFRESH_BIN}} != "0" ] || exit 0
    curl -sSL https://github.com/mikefarah/yq/releases/download/{{YQ_VERSION}}/yq_darwin_{{ARCH}} -o {{OUT_DIR}}/yq
    chmod +x {{OUT_DIR}}/yq

[private]
_create-out-dir:
    mkdir -p {{OUT_DIR}}

[private]
_cleanup-out-dir:
    rm -rf {{OUT_DIR}} || true

crust-gather *flags: _download-crust-gather
    crust-gather {{flags}}

[private]
_download-crust-gather: _create-out-dir
    #!/usr/bin/env bash
    set -euxo pipefail
    [ -z `which crust-gather` ] || [ {{REFRESH_BIN}} != "0" ] || exit 0
    curl -sSfL https://github.com/crust-gather/crust-gather/raw/main/install.sh | sh -s - -f -b {{OUT_DIR}}

[private]
[linux]
_download-kubectl: _create-out-dir
    #!/usr/bin/env bash
    set -euxo pipefail
    [ -z `which kubectl` ] || [ {{REFRESH_BIN}} != "0" ] || exit 0
    cd {{OUT_DIR}}
    curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/{{ARCH}}/kubectl"
    chmod +x kubectl

[private]
[macos]
_download-kubectl: _create-out-dir
    #!/usr/bin/env bash
    set -euxo pipefail
    [ -z `which kubectl` ] || [ {{REFRESH_BIN}} != "0" ] || exit 0
    cd {{OUT_DIR}}
    curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/darwin/{{ARCH}}/kubectl"
    chmod +x kubectl
