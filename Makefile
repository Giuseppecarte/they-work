# they-work — everything runs in Docker. Nothing is installed on your machine.
IMAGE ?= they-work:local
RUST  ?= they-work-dev:local

# Run cargo in a throwaway container as *you*, so build output is owned by you
# and not by root. CARGO_HOME is kept in the project so the container user has
# somewhere writable to put the registry cache.
# `-it` only when there is a terminal, so CI and scripted runs work too.
TTY := $(shell [ -t 0 ] && echo -it)

CARGO = docker run --rm $(TTY) \
  --user $(shell id -u):$(shell id -g) \
  -e CARGO_HOME=/src/.cargo-home \
  -v $(PWD):/src -w /src $(RUST)

DOCKER_RUN = docker run --rm -it \
  --network none \
  -e TERM -e COLORTERM \
  -v $(HOME)/.claude:/data/claude:ro \
  -v $(HOME)/.codex:/data/codex:ro \
  $(IMAGE)

.PHONY: help build run demo test fmt lint check clean dev-image

help: ## Show this help
	@grep -hE '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) \
	  | awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-8s\033[0m %s\n",$$1,$$2}'

build: ## Build the container image
	docker build -f docker/Dockerfile -t $(IMAGE) .

run: build ## Watch your real agents (read-only)
	$(DOCKER_RUN)

demo: build ## Watch an imaginary company; reads nothing
	$(DOCKER_RUN) --demo

test: dev-image ## Run the test suite
	$(CARGO) cargo test --workspace

fmt: dev-image ## Format the code
	$(CARGO) cargo fmt --all

lint: dev-image ## Clippy, warnings are errors
	$(CARGO) cargo clippy --workspace --all-targets -- -D warnings

check: fmt lint test ## Everything CI runs

clean: ## Remove build output
	rm -rf target .cargo-home

dev-image: ## Build the toolchain image used by test/fmt/lint
	@docker image inspect $(RUST) >/dev/null 2>&1 \
	  || docker build -f docker/Dockerfile.dev -t $(RUST) .
