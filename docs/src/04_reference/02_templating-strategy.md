# Templating strategy

The **Cluster API Addon Provider Fleet** automates application [templating][] for imported CAPI clusters based on matching cluster state.

[templating]: https://fleet.rancher.io/ref-fleet-yaml#templating

## Functionality

The **Addon Provider Fleet** ensures that the state of a CAPI cluster and resources is always up-to-date in the `spec.templateValues.ClusterValues` field of the Fleet cluster resource. This allows users to:

- Reference specific parts of CAPI cluster directly or via **Helm substitution patterns** referencing `.ClusterValues.Cluster` data.
- Substiture based on the state of the control plane resource via `.ClusterValues.ControlPlane` field.
- Substiture based on the state of the infrastructure cluster resource via `.ClusterValues.InfrastructureCluster` field.
- Maintain a consistent application state across different clusters.
- Use the same template for multiple matching clusters to simplify deployment and management.

## Example - templating withing HelmOp

-> [Installing Calico](../03_tutorials/03_installing_calico.md#deploying-calico-cni)