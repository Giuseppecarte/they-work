# Native Rust development or an optional Docker toolchain.
IMAGE ?= they-work:local
DEV_IMAGE ?= they-work-dev:local
SHOT_DIR ?= docs/shots
IMAGE_FRAME_DIR ?=
IMAGE_FRAME ?=
# scripts/cargo runs the toolchain container as the invoking user, so build
# output is owned by you and not by root. Contributors need no local Rust.
CARGO = THEYWORK_DEV_IMAGE=$(DEV_IMAGE) ./scripts/cargo

# The shared launcher quotes host paths, skips missing sources, and chooses a
# TTY only for interactive runs. Demo mode never mounts source data.
ARGS ?=
DOCKER_RUN = THEYWORK_SKIP_PULL=1 THEYWORK_IMAGE="$(IMAGE)" sh docs/install.sh

.PHONY: help build run demo native install shot fetch test fmt fmt-check lint check clean
.NOTPARALLEL: check

help: ## Show this help
	@grep -hE '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) \
	  | awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-8s\033[0m %s\n",$$1,$$2}'

build: ## Build the container image
	docker build -f docker/Dockerfile -t $(IMAGE) .

run: build ## Watch your real agents (read-only)
	$(DOCKER_RUN) $(ARGS)

demo: build ## Watch an imaginary company; reads nothing
	$(DOCKER_RUN) --demo $(ARGS)

native: ## Build a native executable (requires local Rust)
	THEYWORK_TOOLCHAIN=native ./scripts/cargo build --release --locked --bin they-work

install: ## Install native executable into Cargo user bin (requires local Rust)
	THEYWORK_TOOLCHAIN=native ./scripts/cargo install --locked --path crates/theywork-tui

shot: ## Export labelled cell/image frames and contact sheet
	python3 scripts/shot.py --view "$(VIEW)" --light "$(LIGHT)" --out-dir "$(SHOT_DIR)" --image-frame-dir "$(IMAGE_FRAME_DIR)" --captured-image "$(IMAGE_FRAME)"

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
