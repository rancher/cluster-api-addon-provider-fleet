# FleetAddonConfig Reference

The `FleetAddonConfig` Custom Resource Definition (CRD) is used to configure the behavior of the Cluster API Addon Provider for Fleet.

## Spec

The `spec` field of the `FleetAddonConfig` CRD contains the configuration options.
It is a required field and provides a config for fleet addon functionality.

-   `config`
    -   **Description:** An object that holds various configuration settings.
    -   **Type:** `object`
    -   **Optional:** Yes

    -   `config.bootstrapLocalCluster`
        -   **Description:** Enable auto-installation of a fleet agent in the local cluster.
        -   **Type:** `boolean`
        -   **Optional:** Yes

        When set to `true`, the provider will automatically install a Fleet agent in the cluster where the provider is running. This is useful for bootstrapping a local development or management cluster to be managed by Fleet.

        **Example:**

        ```yaml
        spec:
          config:
            bootstrapLocalCluster: true
        ```

    -   `config.featureGates`
        -   **Description:** Feature gates controlling experimental features.
        -   **Type:** `object`
        -   **Optional:** Yes

        This section allows enabling or disabling experimental features within the provider.

        -   `config.featureGates.configMap`
            -   **Description:** References a ConfigMap where to apply feature flags. If a ConfigMap is referenced, the controller will update it instead of upgrading the Fleet chart.
            -   **Type:** `object` (ObjectReference)
            -   **Optional:** Yes

            **Example:**

            ```yaml
            spec:
              config:
                featureGates:
                  configMap:
                    ref:
                      apiVersion: v1
                      kind: ConfigMap
                      name: fleet-feature-flags
                      namespace: fleet-system
            ```

        -   `config.featureGates.experimentalHelmOps`
            -   **Description:** Enables experimental Helm operations support.
            -   **Type:** `boolean`
            -   **Optional:** No (Required within `featureGates`)

            **Example:**

            ```yaml
            spec:
              config:
                featureGates:
                  experimentalHelmOps: true
            ```

        -   `config.featureGates.experimentalOciStorage`
            -   **Description:** Enables experimental OCI storage support.
            -   **Type:** `boolean`
            -   **Optional:** No (Required within `featureGates`)

            **Example:**

            ```yaml
            spec:
              config:
                featureGates:
                  experimentalOciStorage: true
            ```

    -   `config.server`
        -   **Description:** Fleet server URL configuration options.
        -   **Type:** `object` (oneOf `inferLocal` or `custom`)
        -   **Optional:** Yes

        This section configures how the provider connects to the Fleet server. You must specify either `inferLocal` or `custom`.

        -   `config.server.inferLocal`
            -   **Description:** Infer the local cluster's API server URL as the Fleet server URL.
            -   **Type:** `boolean`
            -   **Optional:** No (Required if `custom` is not set)

            **Example:**

            ```yaml
            spec:
              config:
                server:
                  inferLocal: true
            ```

        -   `config.server.custom`
            -   **Description:** Custom configuration for the Fleet server URL.
            -   **Type:** `object`
            -   **Optional:** No (Required if `inferLocal` is not set)

            -   `config.server.custom.apiServerCaConfigRef`
                -   **Description:** Reference to a ConfigMap containing the CA certificate for the API server.
                -   **Type:** `object` (ObjectReference)
                -   **Optional:** Yes

                **Example:**

                ```yaml
                spec:
                  config:
                    server:
                      custom:
                        apiServerCaConfigRef:
                          apiVersion: v1
                          kind: ConfigMap
                          name: fleet-server-ca
                          namespace: fleet-system
                ```

            -   `config.server.custom.apiServerUrl`
                -   **Description:** The custom URL for the Fleet API server.
                -   **Type:** `string`
                -   **Optional:** Yes

                **Example:**

                ```yaml
                spec:
                  config:
                    server:
                      custom:
                        apiServerUrl: https://fleet.example.com
                ```

-   `cluster`
    -   **Description:** Enable Cluster config functionality. This will create Fleet Cluster for each Cluster with the same name. In case the cluster specifies topology.class, the name of the ClusterClass will be added to the Fleet Cluster labels.
    -   **Type:** `object`
    -   **Optional:** Yes

    This section configures the behavior for creating Fleet Clusters from Cluster API Clusters.

    -   `cluster.agentEnvVars`
        -   **Description:** Extra environment variables to be added to the agent deployment.
        -   **Type:** `array` of `object` (EnvVar)
        -   **Optional:** Yes

        **Example:**

        ```yaml
        spec:
          cluster:
            agentEnvVars:
              - name: HTTP_PROXY
                value: http://proxy.example.com:8080
              - name: NO_PROXY
                value: localhost,127.0.0.1,.svc
        ```

    -   `cluster.agentNamespace`
        -   **Description:** Namespace selection for the fleet agent.
        -   **Type:** `string`
        -   **Optional:** Yes

        **Example:**

        ```yaml
        spec:
          cluster:
            agentNamespace: fleet-agents
        ```

    -   `cluster.agentTolerations`
        -   **Description:** Agent taint toleration settings for every cluster.
        -   **Type:** `array` of `object` (Toleration)
        -   **Optional:** Yes

        **Example:**

        ```yaml
        spec:
          cluster:
            agentTolerations:
              - key: "node.kubernetes.io/unreachable"
                operator: "Exists"
                effect: "NoExecute"
                tolerationSeconds: 600
              - key: "node.kubernetes.io/not-ready"
                operator: "Exists"
                effect: "NoExecute"
                tolerationSeconds: 600
        ```

    -   `cluster.applyClassGroup`
        -   **Description:** Apply a ClusterGroup for a ClusterClass referenced from a different namespace.
        -   **Type:** `boolean`
        -   **Optional:** Yes

        When a CAPI `Cluster` references a `ClusterClass` in a different namespace, a corresponding `ClusterGroup` is created in the **`Cluster`** namespace. This ensures that all clusters within the namespace that share the same `ClusterClass` from another namespace are grouped together.

        This `ClusterGroup` inherits `ClusterClass` labels and applies two `CAAPF`-specific labels to uniquely identify the group within the cluster scope:

        -   `clusterclass-name.fleet.addons.cluster.x-k8s.io: <class-name>`
        -   `clusterclass-namespace.fleet.addons.cluster.x-k8s.io: <class-ns>`

        Additionally, this configuration enables the creation of a `BundleNamespaceMapping`. This mapping selects all available bundles and establishes a link between the namespace of the `Cluster` and the namespace of the referenced `ClusterClass`. This allows the Fleet `Cluster` to be evaluated as a target for application sources such as `Bundles`, `HelmApps`, or `GitRepos` from the **`ClusterClass`** namespace.

        When all CAPI `Cluster` resources referencing the same `ClusterClass` are removed, both the `ClusterGroup` and `BundleNamespaceMapping` are cleaned up.

        **Note: If the `cluster` field is not set, this setting is enabled by default.**

        **Example:**

        ```yaml
        spec:
          cluster:
            applyClassGroup: true
        ```

    -   `cluster.hostNetwork`
        -   **Description:** Host network allows to deploy agent configuration using `hostNetwork: true` setting which eludes dependency on the CNI configuration for the cluster.
        -   **Type:** `boolean`
        -   **Optional:** Yes

        **Example:**

        ```yaml
        spec:
          cluster:
            hostNetwork: true
        ```

    -   `cluster.namespaceSelector`
        -   **Description:** Namespace label selector. If set, only clusters in the namespace matching label selector will be imported. This configuration defines how to select namespaces based on specific labels. The `namespaceSelector` field ensures that the import strategy applies only to namespaces that have the label `import: "true"`. This is useful for scoping automatic import to specific namespaces rather than applying it cluster-wide.
        -   **Type:** `object` (LabelSelector)
        -   **Optional:** No (Required within `cluster`)

        **Example:**

        ```yaml
        apiVersion: addons.cluster.x-k8s.io/v1alpha1
        kind: FleetAddonConfig
        metadata:
          name: fleet-addon-config
        spec:
          cluster:
            namespaceSelector:
              matchLabels:
                import: "true"
        ```

    -   `cluster.naming`
        -   **Description:** Naming settings for the fleet cluster.
        -   **Type:** `object`
        -   **Optional:** Yes

        This section allows customizing the name of the created Fleet Cluster resource.

        -   `cluster.naming.prefix`
            -   **Description:** Specify a prefix for the Cluster name, applied to created Fleet cluster.
            -   **Type:** `string`
            -   **Optional:** Yes

            **Example:**

            ```yaml
            spec:
              cluster:
                naming:
                  prefix: capi-
            ```

        -   `cluster.naming.suffix`
            -   **Description:** Specify a suffix for the Cluster name, applied to created Fleet cluster.
            -   **Type:** `string`
            -   **Optional:** Yes

            **Example:**

            ```yaml
            spec:
              cluster:
                naming:
                  suffix: -fleet
            ```

    -   `cluster.patchResource`
        -   **Description:** Allow to patch resources, maintaining the desired state. If is not set, resources will only be re-created in case of removal.
        -   **Type:** `boolean`
        -   **Optional:** Yes

        **Example:**

        ```yaml
        spec:
          cluster:
            patchResource: true
        ```

    -   `cluster.selector`
        -   **Description:** Cluster label selector. If set, only clusters matching label selector will be imported. This configuration filters clusters based on labels, ensuring that the `FleetAddonConfig` applies only to clusters with the label `import: "true"`. This allows more granular per-cluster selection across the cluster scope.
        -   **Type:** `object` (LabelSelector)
        -   **Optional:** No (Required within `cluster`)

        **Example:**

        ```yaml
        apiVersion: addons.cluster.x-k8s.io/v1alpha1
        kind: FleetAddonConfig
        metadata:
          name: fleet-addon-config
        spec:
          cluster:
            selector:
              matchLabels:
                import: "true"
        ```

    -   `cluster.setOwnerReferences`
        -   **Description:** Setting to disable setting owner references on the created resources.
        -   **Type:** `boolean`
        -   **Optional:** Yes

        **Example:**

        ```yaml
        spec:
          cluster:
            setOwnerReferences: false
        ```

-   `clusterClass`
    -   **Description:** Enable clusterClass controller functionality. This will create Fleet ClusterGroups for each ClusterClaster with the same name.
    -   **Type:** `object`
    -   **Optional:** Yes

    This section configures the behavior for creating Fleet ClusterGroups from Cluster API ClusterClasses.

    -   `clusterClass.patchResource`
        -   **Description:** Allow to patch resources, maintaining the desired state. If is not set, resources will only be re-created in case of removal.
        -   **Type:** `boolean`
        -   **Optional:** Yes

        **Example:**

        ```yaml
        spec:
          clusterClass:
            patchResource: true
        ```

    -   `clusterClass.setOwnerReferences`
        -   **Description:** Setting to disable setting owner references on the created resources.
        -   **Type:** `boolean`
        -   **Optional:** Yes

        **Example:**

        ```yaml
        spec:
          clusterClass:
            setOwnerReferences: false
        ```

-   `install`
    -   **Description:** Configuration for installing the Fleet chart.
    -   **Type:** `object` (oneOf `followLatest` or `version`)
    -   **Optional:** Yes

    This section configures how the Fleet chart is installed. You must specify either `followLatest` or `version`.

    -   `install.followLatest`
        -   **Description:** Follow the latest version of the chart on install.
        -   **Type:** `boolean`
        -   **Optional:** No (Required if `version` is not set)

        **Example:**

        ```yaml
        spec:
          install:
            followLatest: true
        ```

    -   `install.version`
        -   **Description:** Use specific version to install.
        -   **Type:** `string`
        -   **Optional:** No (Required if `followLatest` is not set)

        **Example:**

        ```yaml
        spec:
          install:
            version: 0.12.0
        ```
