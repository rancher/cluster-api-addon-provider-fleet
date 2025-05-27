use crate::api::capi_clusterclass::ClusterClass;

use crate::api::fleet_addon_config::{ClusterClassConfig, FleetAddonConfig};
use crate::api::fleet_clustergroup::ClusterGroup;

use kube::api::PatchParams;

use kube::runtime::controller::Action;

use std::sync::Arc;

use super::controller::{
    fetch_config, get_or_create, patch, Context, FleetBundle, FleetController,
};
use super::{BundleResult, GroupSyncResult};

pub struct FleetClusterClassBundle {
    fleet_group: ClusterGroup,
    config: FleetAddonConfig,
}

impl FleetBundle for FleetClusterClassBundle {
    #[allow(refining_impl_trait)]
    async fn sync(&mut self, ctx: Arc<Context>) -> GroupSyncResult<Action> {
        if self.config.cluster_class_patch_enabled() {
            patch(
                ctx,
                &mut self.fleet_group,
                &PatchParams::apply("addon-provider-fleet"),
            )
            .await?
        } else {
            get_or_create(ctx.clone(), &self.fleet_group).await?
        };

        Ok(Action::await_change())
    }
}

impl FleetController for ClusterClass {
    type Bundle = FleetClusterClassBundle;

    async fn to_bundle(&self, ctx: Arc<Context>) -> BundleResult<Option<FleetClusterClassBundle>> {
        let config = fetch_config(ctx.client.clone()).await?;
        if !config.cluster_class_operations_enabled() {
            return Ok(None);
        }

        let mut fleet_group: ClusterGroup = self.into();
        if let Some(ClusterClassConfig {
            set_owner_references: Some(true),
            ..
        }) = config.spec.cluster_class
        {
        } else {
            fleet_group.metadata.owner_references = None;
        }

        Ok(Some(FleetClusterClassBundle {
            fleet_group,
            config,
        }))
    }
}
