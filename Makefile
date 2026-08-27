# they-work — everything runs in Docker. Nothing is installed on your machine.
IMAGE ?= they-work:local
DEV_IMAGE ?= they-work-dev:local
# scripts/cargo runs the toolchain container as the invoking user, so build
# output is owned by you and not by root. Contributors need no local Rust.
CARGO = THEYWORK_DEV_IMAGE=$(DEV_IMAGE) ./scripts/cargo

DOCKER_RUN = docker run --rm -it \
  --network none \
  -e TERM -e COLORTERM \
  -v $(HOME)/.claude:/data/claude:ro \
  -v $(HOME)/.codex:/data/codex:ro \
  $(IMAGE)

.PHONY: help build run demo test fmt lint check clean

help: ## Show this help
	@grep -hE '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) \
	  | awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-8s\033[0m %s\n",$$1,$$2}'

build: ## Build the container image
	docker build -f docker/Dockerfile -t $(IMAGE) .

run: build ## Watch your real agents (read-only)
	$(DOCKER_RUN)

demo: build ## Watch an imaginary company; reads nothing
	$(DOCKER_RUN) --demo

test: ## Run the test suite
	$(CARGO) test --workspace

fmt: ## Format the code
	$(CARGO) fmt --all

lint: ## Clippy, warnings are errors
	$(CARGO) clippy --workspace --all-targets -- -D warnings

check: fmt lint test ## Everything CI runs

clean: ## Remove build output
	rm -rf target .cargo-home


