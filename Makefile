MACHINE := rancher
DEFAULT_PLATFORMS := linux/amd64,linux/arm64

REPO ?= rancher
IMAGE ?= cluster-api-addon-provider-fleet
TAG ?= dev
IMAGE_NAME ?= $(REPO)/$(IMAGE):$(TAG)

.PHONY: buildx-machine
buildx-machine: ## Create rancher buildx machine targeting DEFAULT_PLATFORMS.
	@docker buildx ls | grep $(MACHINE) || \
	  docker buildx create --name=$(MACHINE) --platform=$(DEFAULT_PLATFORMS)

.PHONY: push-image
push-image: ## Build and push multiarch image via docker buildx (called by publish-image action).
	docker buildx build \
	  $(IID_FILE_FLAG) \
	  $(BUILDX_ARGS) \
	  --platform=$(TARGET_PLATFORMS) \
	  --tag $(IMAGE_NAME) \
	  --push \
	  .

.PHONY: push-prime-image
push-prime-image: ## Build and push multiarch image to prime registry with SBOM and provenance attestations.
	BUILDX_ARGS="--sbom=true --attest type=provenance,mode=max" \
	$(MAKE) push-image
