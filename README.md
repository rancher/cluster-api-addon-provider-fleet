# Cluster API Add-on Provider for Fleet

> NOTE: The project is looking for more contributors.

## What is Cluster API Add-on Provider for Fleet (CAAPF)?

Cluster API Add-on Provider for Fleet (CAAPF) is a Cluster API (CAPI) provider that provides integration with [Fleet](https://github.com/rancher/fleet) to enable the easy deployment of applications to a CAPI provisioned cluster.

It provides the following functionality:

- Addon provider automatically installs `Fleet` in your management cluster.
- The provider will register a newly provisioned CAPI cluster with `Fleet` so that applications can be automatically deployed to the created cluster via GitOps, `Bundle` or `HelmOp`.
- The provider will automatically create a [Fleet Cluster Group](https://fleet.rancher.io/cluster-group) for every [CAPI ClusterClass](https://cluster-api.sigs.k8s.io/tasks/experimental-features/cluster-class/). This enables you to deploy the same applications to all clusters created from the same ClusterClass.
- `CAPI` `Cluster`, `ControlPlane` resources are automatically added to the `Fleet` `Cluster` resource templates, allowing to perform per-cluster configuration templating for `Helm` based installations.

## Getting started

You can refer to the provider [documentation](https://rancher.github.io/cluster-api-addon-provider-fleet/) and the official `Fleet` [documentation](https://fleet.rancher.io/).

## Installation

Refer to the book [installation](./docs/book/02_getting_started/01_installation) section.

## Configuration

Refer to the book [configuration](./docs/book/03_tutorials/02_configuration) section

## Demo

[![asciicast](https://asciinema.org/a/659626.svg)](https://asciinema.org/a/659626)

## Calico CNI installation demo

[![asciicast](https://asciinema.org/a/700924.svg)](https://asciinema.org/a/700924)

## Get in contact

You can get in contact with us via the [#cluster-api](https://rancher-users.slack.com/archives/C060L985ZGC) channel on the [Rancher Users Slack](https://slack.rancher.io/).
