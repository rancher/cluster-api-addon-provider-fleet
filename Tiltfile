# Usage default features:
# tilt up
#
# Usage with features:
# tilt up telemetry
config.define_string("features", args=True)
cfg = config.parse()
features = cfg.get('features', "")
print("compiling with features: {}".format(features))

local_resource('compile', 'just create-out-dir compile %s' % features)
docker_build('ghcr.io/rancher/cluster-api-addon-provider-fleet', '.', dockerfile='Dockerfile')
yaml = kustomize('config/default')
k8s_yaml(yaml)
k8s_resource('caapf-controller-manager')