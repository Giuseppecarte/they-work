# they-work — everything runs in Docker. Nothing is installed on your machine.
IMAGE ?= they-work:local
DOCKER_RUN = docker run --rm -it \
  --network none \
  -e TERM -e COLORTERM \
  -v $(HOME)/.claude:/data/claude:ro \
  -v $(HOME)/.codex:/data/codex:ro \
  $(IMAGE)

.PHONY: help build run demo test fmt lint clean

help: ## Show this help
	@grep -hE '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) \
	  | awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-8s\033[0m %s\n",$$1,$$2}'

build: ## Build the container image
	docker build -f docker/Dockerfile -t $(IMAGE) .

run: build ## Watch your real agents (read-only)
	$(DOCKER_RUN)

demo: build ## Watch an imaginary company; reads nothing
	$(DOCKER_RUN) --demo

test: ## Run the test suite in a throwaway container
	docker run --rm -v $(PWD):/src -w /src rust:1.83-slim-bookworm \
	  sh -c 'apt-get update >/dev/null && apt-get install -y --no-install-recommends gcc libc6-dev >/dev/null && cargo test --workspace'

fmt: ## Format the code
	docker run --rm -v $(PWD):/src -w /src rust:1.83-slim-bookworm cargo fmt --all

lint: ## Clippy, warnings are errors
	docker run --rm -v $(PWD):/src -w /src rust:1.83-slim-bookworm \
	  sh -c 'apt-get update >/dev/null && apt-get install -y --no-install-recommends gcc libc6-dev >/dev/null && rustup component add clippy >/dev/null && cargo clippy --workspace --all-targets -- -D warnings'

clean: ## Remove build output
	rm -rf target
