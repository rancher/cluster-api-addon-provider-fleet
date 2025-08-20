# What is Cluster API Add-on Provider for Fleet (CAAPF)?

Cluster API Add-on Provider for Fleet (CAAPF) is a Cluster API (CAPI) provider that provides integration with [Fleet](https://github.com/rancher/fleet) to enable the easy deployment of applications to a CAPI provisioned cluster.

It provides the following functionality:

- Addon provider automatically installs `Fleet` in your management cluster.
- The provider will register a newly provisioned CAPI cluster with `Fleet` so that applications can be automatically deployed to the created cluster via GitOps, `Bundle` or `HelmOp`.
- The provider will automatically create a [Fleet Cluster Group](https://fleet.rancher.io/cluster-group) for every [CAPI ClusterClass](https://cluster-api.sigs.k8s.io/tasks/experimental-features/cluster-class/). This enables you to deploy the same applications to all clusters created from the same ClusterClass.
- `CAPI` `Cluster`, `ControlPlane` resources are automatically added to the `Fleet` `Cluster` resource templates, allowing to perform per-cluster configuration templating for `Helm` based installations.


{{#include ./02_getting_started/01_installation.md}}

## Demo

<script src="https://asciinema.org/a/659626.js" id="asciicast-659626" async="true"></script>

## Calico CNI installation demo

<script src="https://asciinema.org/a/700924.js" id="asciicast-700924" async="true"></script>
