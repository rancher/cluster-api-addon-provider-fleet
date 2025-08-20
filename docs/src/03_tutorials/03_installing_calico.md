# Installing Calico CNI using HelmOp

<div class="warning">

Note: For this setup to work, you need to install Fleet and Fleet CRDs charts via
`FleetAddonConfig` resource. Both need to have version >= v0.12.0,
which provides support for `HelmOp` resource.

</div>

In this tutorial we will deploy `Calico` CNI using `HelmOp` resource and `Fleet` cluster substitution mechanism.

## Deploying Calico CNI

Here's an example of how a `HelmOp` resource can be used in combination with templateValues to deploy application consistently on any matching cluster.

In this scenario we are matching cluster directly by name, using `clusterName` reference, but a `clusterGroup` or a label based selection can be used instead or together with `clusterName`:
```yaml
  targets:
  - clusterName: docker-demo
```

We are deploying `HelmOp` resource in the `default` namespace. The namespace should be the same for the CAPI Cluster for fleet to locate it.

```yaml
{{#include ../../../testdata/helm.yaml}}
```

`HelmOp` supports fleet [templating][] options, otherwise available exclusively to the `fleet.yaml` configuration, stored in the [git repository contents][], and applied via the `GitRepo` resource.

[templating]: https://fleet.rancher.io/ref-fleet-yaml#templating
[git repository contents]: https://fleet.rancher.io/gitrepo-content

In this example we are using values from the `Cluster.spec.clusterNetwork.pods.cidrBlocks` list to define `ipPools` for the `calicoNetwork`. These chart settings will be unique per each matching cluster, and based on the observed cluster state at any moment.

After appying the resource we will observe the app rollout:

```bash
> kubectl apply -f testdata/helm.yaml
helmop.fleet.cattle.io/calico created
> kubectl get helmop
NAME     REPO                                   CHART             VERSION   BUNDLEDEPLOYMENTS-READY   STATUS
calico   https://docs.tigera.io/calico/charts   tigera-operator   v3.29.2   0/1                       NotReady(1) [Bundle calico]; apiserver.operator.tigera.io default [progressing]
# After some time
> kubectl get helmop
NAME     REPO                                   CHART             VERSION   BUNDLEDEPLOYMENTS-READY   STATUS
calico   https://docs.tigera.io/calico/charts   tigera-operator   v3.29.2   1/1
> kubectl get pods -n calico-system --context capi-quickstart
NAME                                      READY   STATUS    RESTARTS   AGE
calico-kube-controllers-9cd68cb75-p46pz   1/1     Running   0          53s
calico-node-bx5b6                         1/1     Running   0          53s
calico-node-hftwd                         1/1     Running   0          53s
calico-typha-6d9fb6bcb4-qz6kt             1/1     Running   0          53s
csi-node-driver-88jqc                     2/2     Running   0          53s
csi-node-driver-mjwxc                     2/2     Running   0          53s
```

## Demo

You can follow along with the demo to verify that your deployment is matching expected result:

<script src="https://asciinema.org/a/700924.js" id="asciicast-700924" async="true"></script>
