# they-work — everything runs in Docker. Nothing is installed on your machine.
IMAGE ?= they-work:local
DEV_IMAGE ?= they-work-dev:local
SHOT_DIR ?= docs/shots
# scripts/cargo runs the toolchain container as the invoking user, so build
# output is owned by you and not by root. Contributors need no local Rust.
CARGO = THEYWORK_DEV_IMAGE=$(DEV_IMAGE) ./scripts/cargo

THEYWORK_CLAUDE_HOME ?= /data/claude
THEYWORK_CODEX_HOME ?= /data/codex

# Run as you. Agent transcripts are private to their owner (Claude writes them
# 0600), so a container with its own uid can list the directories and open
# nothing. Reading them as yourself is also the honest posture: they-work sees
# exactly what you can see, and no more.
DOCKER_SECURITY = \
  --user $(shell id -u):$(shell id -g) \
  --network none \
  --read-only \
  --cap-drop ALL \
  --security-opt no-new-privileges

DOCKER_ENV = \
  -e TERM \
  -e COLORTERM \
  -e THEYWORK_CLAUDE_HOME=$(THEYWORK_CLAUDE_HOME) \
  -e THEYWORK_CODEX_HOME=$(THEYWORK_CODEX_HOME) \
  -e THEYWORK_COLOR \
  -e NO_COLOR

DOCKER_VOLUMES = \
  -v $(HOME)/.claude:/data/claude:ro \
  -v $(HOME)/.codex:/data/codex:ro

DOCKER_RUN = docker run --rm -it \
  $(DOCKER_SECURITY) \
  $(DOCKER_ENV) \
  $(DOCKER_VOLUMES) \
  $(IMAGE)

DOCKER_DEMO_RUN = docker run --rm -it \
  $(DOCKER_SECURITY) \
  $(DOCKER_ENV) \
  $(IMAGE) --demo

.PHONY: help build run demo shot fetch test fmt fmt-check lint check clean
.NOTPARALLEL: check

help: ## Show this help
	@grep -hE '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) \
	  | awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-8s\033[0m %s\n",$$1,$$2}'

build: ## Build the container image
	docker build -f docker/Dockerfile -t $(IMAGE) .

run: build ## Watch your real agents (read-only)
	$(DOCKER_RUN)

demo: build ## Watch an imaginary company; reads nothing
	$(DOCKER_DEMO_RUN)

shot: ## Export labelled resolution-rung PNG/SVG frames and contact sheet
	python3 scripts/shot.py --view "$(VIEW)" --light "$(LIGHT)" --out-dir "$(SHOT_DIR)"

fetch: ## Populate the locked Cargo cache (networked)
	THEYWORK_CARGO_NETWORK=bridge $(CARGO) fetch --locked

test: ## Run the test suite
	$(CARGO) test --workspace

fmt: ## Format the code
	$(CARGO) fmt --all

fmt-check: ## Check formatting without changing files
	$(CARGO) fmt --all -- --check

lint: ## Clippy, warnings are errors
	$(CARGO) clippy --workspace --all-targets -- -D warnings

check: fetch fmt-check lint test ## Bootstrap and run every local code check

clean: ## Remove build output
	rm -rf target .cargo-home
