# This Makefile is consumed by the publish-image action.
# For any other workflow, continue using justfile.
TAG ?= dev
REPO ?= rancher
IMAGE ?= cluster-api-addon-provider-fleet
IMAGE_NAME ?= $(REPO)/$(IMAGE):$(TAG)

.PHONY: push-image
push-image:
	docker buildx build \
		$(IID_FILE_FLAG) \
		$(BUILDX_ARGS) \
		--platform=$(TARGET_PLATFORMS) \
		--tag $(IMAGE_NAME) \
		--push \
		.
