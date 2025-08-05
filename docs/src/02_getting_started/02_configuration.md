# Configuration

## Installing Fleet

By default `CAAPF` expects your cluster to have `Fleet` helm chart pre-installed and configured, but it can manage `Fleet` installation via `FleetAddonConfig` resource, named `fleet-addon-config`. To install `Fleet` helm chart with latest stable `Fleet` version:

```yaml
apiVersion: addons.cluster.x-k8s.io/v1alpha1
kind: FleetAddonConfig
metadata:
  name: fleet-addon-config
spec:
  config:
    server:
      inferLocal: true # Uses default `kuberenetes` endpoint and secret for APIServerURL configuration
  install:
    followLatest: true
```

Alternatively, a specific version can be provided in the `spec.install.version`:

```yaml
apiVersion: addons.cluster.x-k8s.io/v1alpha1
kind: FleetAddonConfig
metadata:
  name: fleet-addon-config
spec:
  config:
    server:
      inferLocal: true # Uses default `kuberenetes` endpoint and secret for APIServerURL configuration
  install:
    followLatest: true
```

### Fleet Public URL and Certificate setup

Fleet agent requires direct access to the `Fleet` server instance running in the management cluster. When provisioning `Fleet` agent on the downstream cluster using the default [`manager-initiated`](https://fleet.rancher.io/cluster-registration#manager-initiated) registration, the public API server url and certificates will be taken from the current `Fleet` server configuration.

If a user installaling `Fleet` via `FleetAddonConfig` resource, there are fields which allow to configure these settings.

Field `config.server` allows to specify setting for the Fleet server configuration, such as `apiServerURL` and certificates.

Using `inferLocal: true` setting allows to use default `kubernetes` endpoint and `CA` secret to configure the Fleet instance.

```yaml
apiversion: addons.cluster.x-k8s.io/v1alpha1
kind: FleetAddonConfig
metadata:
  name: fleet-addon-config
spec:
  config:
    server:
      inferLocal: true # Uses default `kuberenetes` endpoint and secret for APIServerURL configuration
  install:
    followLatest: true
```

This scenario works well in a test setup, while using CAPI docker provider and docker clusters.

Here is an example of a manulal `API` server `URL` configuration with a reference to certificates `ConfigMap` or `Secret`, which contains a `ca.crt` data key for the `Fleet` helm chart:

```yaml
apiversion: addons.cluster.x-k8s.io/v1alpha1
kind: FleetAddonConfig
metadata:
  name: fleet-addon-config
spec:
  config:
    server:
      apiServerUrl: "https://public-url.io"
      apiServerCaConfigRef:
        apiVersion: v1
        kind: ConfigMap
        name: kube-root-ca.crt
        namespace: default
  install:
    followLatest: true # Installs current latest version of fleet from https://github.com/rancher/fleet-helm-charts
```

### Cluster Import Strategy

-> [Import Strategy](../04_reference/01_import-strategy.md)

### Fleet Feature Flags

Fleet includes experimental features that can be enabled or disabled using feature gates in the `FleetAddonConfig` resource. These flags are configured under .spec.config.featureGates.

To enable experimental features such as OCI storage support and `HelmOp` support, update the FleetAddonConfig as follows:

```yaml
apiVersion: addons.cluster.x-k8s.io/v1alpha1
kind: FleetAddonConfig
metadata:
  name: fleet-addon-config
spec:
  config:
    featureGates:
      experimentalOciStorage: true   # Enables experimental OCI storage support
      experimentalHelmOps: true      # Enables experimental Helm operations support
```

**By default, if the `featureGates` field is not present, these feature gates are *enabled*. To disable these need to explicitly be set to `false`.**

Optionally, the `featureGates` flags can be synced to a `ConfigMap` object.  
This is useful when Fleet is installed and managed by Rancher.  
When a `ConfigMap` reference is defined, the controller will just sync the `featureGates` to it, without making any changes to the Fleet helm chart.  

```yaml
apiVersion: addons.cluster.x-k8s.io/v1alpha1
kind: FleetAddonConfig
metadata:
  name: fleet-addon-config
spec:
  config:
    featureGates:
      experimentalOciStorage: true   # Enables experimental OCI storage support
      experimentalHelmOps: true      # Enables experimental Helm operations support
      configMap:
        ref:
          apiVersion: v1
          kind: ConfigMap
          name: rancher-config
          namespace: cattle-system
```
