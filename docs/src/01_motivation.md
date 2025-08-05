# Motivation

Currently, in the CAPI ecosystem, several solutions exist for deploying applications as add-ons on clusters provisioned by CAPI. However, this idea and its alternatives have not been actively explored upstream, particularly in the `GitOps` space. The need to address this gap was raised in the [Cluster API Addon Orchestration proposal][proposal].

[proposal]: https://github.com/kubernetes-sigs/cluster-api/blob/4e60bd3e7a6d9a94ac74e3fca2d3df935ff47ed9/docs/proposals/20220712-cluster-api-addon-orchestration.md#motivation

One of the projects involved in deploying `Helm` charts on CAPI-provisioned clusters is the [CAPI Addon Provider Helm (CAAPH)][CAAPH]. This solution enables users to automatically install `HelmChartProxy` on provisioned clusters.

[CAAPH]: https://github.com/kubernetes-sigs/cluster-api-addon-provider-helm

Fleet also supports deploying Helm charts via the (experimental) `HelmOp` resource, which offers similar capabilities to `HelmChartProxy`. However, Fleet primarily focuses on providing `GitOps` capabilities for managing `CAPI` clusters and application states within these clusters.

Out of the box, `Fleet` allows users to deploy and maintain the state of arbitrary templates on child clusters using the `Fleet` [`Bundle`][] resource. This approach addresses the need for alternatives to [`ClusterResourceSet`][] while offering full application lifecycle management.

[`ClusterResourceSet`]: https://github.com/kubernetes-sigs/cluster-api/blob/4e60bd3e7a6d9a94ac74e3fca2d3df935ff47ed9/docs/proposals/20220712-cluster-api-addon-orchestration.md#why-not-clusterresourcesets
[`Bundle`]: https://fleet.rancher.io/bundle-add

`CAAPF` is designed to streamline and enhance native `Fleet` integration with `CAPI`. It functions as a separate `Addon` provider that can be [installed][] via `clusterctl` or the `CAPI Operator`.

[installed]: ./02_getting_started/01_installation.md

## User Stories

### User Story 1

As an infrastructure provider, I want to deploy my provisioning application to every provisioned child cluster so that I can provide immediate functionality during and after cluster bootstrap.

### User Story 2

As a DevOps engineer, I want to use GitOps practices to deploy CAPI clusters and applications centrally so that I can manage all cluster configurations and deployed applications from a single location.

### User Story 3

As a user, I want to deploy applications into my CAPI clusters and configure those applications based on the cluster infrastructure templates so that they are correctly provisioned for the cluster environment.

### User Story 4

As a cluster operator, I want to streamline the provisioning of Cluster API child clusters so that they can be successfully provisioned and become `Ready` from a template without manual intervention.

### User Story 5

As a cluster operator, I want to facilitate the provisioning of Cluster API child clusters located behind NAT so that they can be successfully provisioned and establish connectivity with the management cluster.
