# Installation

## Clusterctl

To install provider with `clusterctl`:

- Install [`clusterctl`](https://cluster-api.sigs.k8s.io/user/quick-start.html?highlight=helm-chart-proxy#install-clusterctl)
- Run `clusterctl init --addon rancher-fleet`

## Cluster API Operator

You can install production instance of `CAAPF` in your cluster with [`CAPI Operator`](https://github.com/kubernetes-sigs/cluster-api-operator).

We need to install `cert-manager` as a pre-requisite to `CAPI Operator`, if it is not currently installed:
```bash
kubectl apply -f https://github.com/jetstack/cert-manager/releases/latest/download/cert-manager.yaml
```

To install `CAPI Operator`, `docker` infrastructure provider and the fleet addon together:

```bash
helm repo add capi-operator https://kubernetes-sigs.github.io/cluster-api-operator
helm repo update
helm upgrade --install capi-operator capi-operator/cluster-api-operator \
    --create-namespace -n capi-operator-system \
    --set infrastructure.docker.enabled=true --set addon.rancher-fleet.enabled=true
```
