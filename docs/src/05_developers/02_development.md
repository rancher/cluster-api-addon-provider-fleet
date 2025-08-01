# Development

## Development setup

### Prerequisites

- [kind](https://kind.sigs.k8s.io/)
- [helm](https://helm.sh/)
- [just](https://github.com/casey/just)

Alternatively:

- [nix](https://nixos.org/download/)

To enter the environment with prerequisites:

```bash
nix-shell
```

#### Common prerequisite

- [docker](https://docs.docker.com/engine/install/)

### Create a local development environment

1. Clone the [CAAPF][] repository locally.
2. The project provides an easy way of starting your own development environment. You can take some time to study the [justfile][] that includes a number of pre-configured commands to set up and build your own CAPI management cluster and install the addon provider for Fleet.
3. Run the following:
```
just start-dev
```
This command will create a kind cluster and manage the installation of the fleet provider and all dependencies.
4. Once the installation is complete, you can inspect the current state of your development cluster.

[CAAPF]: https://github.com/rancher/cluster-api-addon-provider-fleet/
[justfile]: https://github.com/rancher/cluster-api-addon-provider-fleet/blob/main/justfile