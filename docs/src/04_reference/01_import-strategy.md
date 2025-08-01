# Import Strategy

CAAPF follows a simple import strategy for CAPI clusters:

1. Each CAPI cluster has a corresponding Fleet `Cluster` object.
2. Each CAPI Cluster Class has a corresponding Fleet `ClusterGroup` object.
3. When a CAPI `Cluster` references a `ClusterClass` in a different namespace, a `ClusterGroup` is created in the `Cluster` namespace. This `ClusterGroup` targets all clusters in this namespace that reference the same `ClusterClass`. See the [configuration](03_fleet-addon-config#applyclassgroup) section for details.
4. If at least one CAPI `Cluster` references a `ClusterClass` in a different namespace, a [`BundleNamespaceMapping`][mapping] is created in the `ClusterClass` namespace. This allows Fleet `Cluster` resources to use application sources such as `Bundles`, `HelmApps`, or `GitRepos` from the `ClusterClass` namespace as if they were deployed in the `Cluster` namespace. See the [configuration](#cluster-clustergroupbundlenamespacemapping-configuration) section for details.

[mapping]: https://fleet.rancher.io/namespaces#cross-namespace-deployments

**By default, `CAAPF` imports all `CAPI` clusters under Fleet management. See the [configuration](03_fleet-addon-config.md#applyclassgroup) section for details.**

![CAAPF-import-groups excalidraw dark](https://github.com/rancher/cluster-api-addon-provider-fleet/assets/32226600/0e0bf58d-7030-491e-976e-8363023f0c88)

## Label Synchronization

Fleet relies on `Cluster` labels, `Cluster` names, and `ClusterGroups` for target matching when deploying applications or referenced repository content. To ensure consistency, `CAAPF` synchronizes resource labels:

1. From the CAPI `ClusterClass` to the imported Fleet `Cluster` resource.
2. From the CAPI `ClusterClass` to the imported Fleet `ClusterGroup` resource.

When a CAPI `Cluster` references a `ClusterClass`, `CAAPF` applies two specific labels to both the `Cluster` and `ClusterGroup` resources:

- `clusterclass-name.fleet.addons.cluster.x-k8s.io: <class-name>`
- `clusterclass-namespace.fleet.addons.cluster.x-k8s.io: <class-ns>`
