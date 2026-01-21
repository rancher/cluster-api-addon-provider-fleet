# Installing Calico CNI using GitRepo

<div class="warning">

Note: For this setup to work, you need have Fleet and Fleet CRDs charts installed
with version >= `v0.12.0`.

</div>

In this tutorial we will deploy `Calico` CNI using `GitRepo` resource on `RKE2` based docker cluster.

## Deploying RKE2 docker cluster

We will first need to create a RKE2 based docker cluster from templates:

```bash
> kubectl apply -f testdata/cluster_docker_rke2.yaml
dockercluster.infrastructure.cluster.x-k8s.io/docker-demo created
cluster.cluster.x-k8s.io/docker-demo created
dockermachinetemplate.infrastructure.cluster.x-k8s.io/docker-demo-control-plane created
rke2controlplane.controlplane.cluster.x-k8s.io/docker-demo-control-plane created
dockermachinetemplate.infrastructure.cluster.x-k8s.io/docker-demo-md-0 created
rke2configtemplate.bootstrap.cluster.x-k8s.io/docker-demo-md-0 created
machinedeployment.cluster.x-k8s.io/docker-demo-md-0 created
configmap/docker-demo-lb-config created
```

In this scenario cluster is located in the `default` namespace, where the rest of fleet objects will go.
Cluster is labeled with `cni: calico` in order for the `GitRepo` to match on it.

```yaml
apiVersion: cluster.x-k8s.io/v1beta2
kind: Cluster
metadata:
  name: docker-demo
  labels:
    cni: calico
```

Now that cluster is created, `GitRepo` can be applied which will be evaluated asynchroniously.

## Deploying Calico CNI via `GitRepo`

We will first review the content of our `fleet.yaml` file:

```yaml
{{#include ../../../fleet/applications/calico/fleet.yaml}}
```

In this scenario we are using `helm` definition which is consistent with the `HelmOp` spec from the [previous][] guide, and defines same templating rules.

We also need to [resolve conflicts][], which happen due to in-place modification of some resources by the `calico` controllers. For that, the `diff` section is used, where we remove blocking fields from comparison.

[previous]: ./03_installing_calico.md
[resolve conflicts]: https://fleet.rancher.io/bundle-diffs

Once everything is ready, we need to apply our `GitRepo` in the `default` namespace. In our case, we will match on clusters labeled with `cni: calico` label:

```yaml
{{#include ../../../testdata/gitrepo-calico.yaml}}
```

```bash
> kubectl apply -f testdata/gitrepo-calico.yaml
gitrepo.fleet.cattle.io/calico created
# After some time
> kubectl get gitrepo
NAME     REPO                                                                     COMMIT                                     BUNDLEDEPLOYMENTS-READY   STATUS
calico   https://github.com/rancher/cluster-api-addon-provider-fleet.git   62b4fe6944687e02afb331b9e1839e33c539f0c7   1/1
```

Now our cluster have `calico` installed, and all nodes are marked as `Ready`:

```bash
# exec into one of the CP node containers
> docker exec -it fef3427009f6 /bin/bash
root@docker-demo-control-plane-krtnt:/#
root@docker-demo-control-plane-krtnt:/# kubectl get pods -n calico-system --kubeconfig /var/lib/rancher/rke2/server/cred/api-server.kubeconfig
NAME                                       READY   STATUS    RESTARTS   AGE
calico-kube-controllers-55cbcc7467-j5bbd   1/1     Running   0          3m30s
calico-node-mbrqg                          1/1     Running   0          3m30s
calico-node-wlbwn                          1/1     Running   0          3m30s
calico-typha-f48c7ddf7-kbq6d               1/1     Running   0          3m30s
csi-node-driver-87tlx                      2/2     Running   0          3m30s
csi-node-driver-99pqw                      2/2     Running   0          3m30s
```

## Demo

You can follow along with the demo to verify that your deployment is matching expected result:

<script src="https://asciinema.org/a/706570.js" id="asciicast-706570" async="true"></script>
