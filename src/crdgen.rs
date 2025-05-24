use ::controller::api::fleet_addon_config::FleetAddonConfig;
use kube::CustomResourceExt;

fn main() {
    print!(
        "{}",
        serde_yaml::to_string(&FleetAddonConfig::crd()).unwrap()
    );
}
