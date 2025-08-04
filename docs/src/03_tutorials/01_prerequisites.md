# Prerequisites

## Requirements

- [helm](https://helm.sh/)
- [CAPI management cluster](https://cluster-api.sigs.k8s.io/).
    - Features `EXP_CLUSTER_RESOURCE_SET` and `CLUSTER_TOPOLOGY` must be enabled.
    - [clusterctl](https://cluster-api.sigs.k8s.io/user/quick-start.html?highlight=clusterctl#install-clusterctl).

### Create your local cluster

> NOTE: if you prefer to opt for a one-command installation, you can refer to the notes on how to use `just` and the project's `justfile` [here](../05_developers/02_development.md#create-a-local-development-environment).

1. Start by adding the helm repositories that are required to proceed with the installation.
```
helm repo add fleet https://rancher.github.io/fleet-helm-charts/
helm repo update
```
2. Create the local cluster
```
kind create cluster --config testdata/kind-config.yaml
```
3. Install [fleet](https://github.com/rancher/fleet) and specify the `API_SERVER_URL` and CA.
```
# We start by retrieving the CA data from the cluster
kubectl config view -o json --raw | jq -r '.clusters[] | select(.name=="kind-dev").cluster["certificate-authority-data"]' | base64 -d > _out/ca.pem
# Set the API server URL
API_SERVER_URL=`kubectl config view -o json --raw | jq -r '.clusters[] | select(.name=="kind-dev").cluster["server"]'`
# And proceed with the installation via helm
helm -n cattle-fleet-system install --version v0.12.0 --create-namespace --wait fleet-crd fleet/fleet-crd
helm install --create-namespace --version v0.12.0 -n cattle-fleet-system --set apiServerURL=$API_SERVER_URL --set-file apiServerCA=_out/ca.pem fleet fleet/fleet --wait
```
4. Install CAPI with the required experimental features enabled and initialized the Docker provider for testing.
```
EXP_CLUSTER_RESOURCE_SET=true CLUSTER_TOPOLOGY=true clusterctl init -i docker --addon rancher-fleet
```

Wait for all pods to become ready and your cluster should be ready to use CAAPF!

### Create your downstream cluster

In order to initiate CAAPF autoimport, a `CAPI` Cluster needs to be created.

To create one, we can either follow [quickstart](https://cluster-api.sigs.k8s.io/user/quick-start#initialization-for-common-providers) documentation or create a cluster from existing template.

```bash
kubectl apply -f testdata/capi-quickstart.yaml
```

For more advanced cluster import strategy, check the [configuration](../02_getting_started/02_configuration.md) section.

**Remember that you can follow along with the video demo to install the provider and get started quickly.**

<script src="https://asciinema.org/a/659626.js" id="asciicast-659626" async="true"></script>
