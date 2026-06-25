# faucet-zcash Makefile. Wraps all project commands behind a consistent
# interface. Run `make help` to list targets.

.PHONY: help
help: ## Ask for help!
	@grep -E '^[a-zA-Z0-9_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | \
		awk 'BEGIN {FS = ":.*?## "}; \
		{printf "\033[36m%-28s\033[0m %s\n", $$1, $$2}'

## --- Rust workspace (native crates: faucet-core, signer) ---

.PHONY: build
build: ## Build native workspace crates (debug)
	cargo build

.PHONY: build-release
build-release: ## Build native workspace crates (release)
	cargo build --release

.PHONY: check
check: ## Type-check native workspace crates
	cargo check

.PHONY: check-format
check-format: ## Check Rust formatting
	cargo fmt --all --check

.PHONY: format
format: ## Format Rust code
	cargo fmt --all

.PHONY: lint
lint: ## Run clippy on native crates
	cargo clippy --all-targets -- -D warnings

.PHONY: lint-wasm
lint-wasm: ## Run clippy on the wasm crates (wasm32 target)
	cargo clippy -p faucet-addr-wasm -p faucet-worker \
		--target wasm32-unknown-unknown -- -D warnings

.PHONY: test
test: ## Run native tests
	cargo test

.PHONY: deny
deny: ## Run cargo-deny (advisories, licenses, bans, sources)
	cargo deny check

## --- wasm crates (faucet-addr-wasm, worker) ---

.PHONY: build-wasm
build-wasm: ## Build both wasm crates (wasm32 target)
	cargo build -p faucet-addr-wasm -p faucet-worker \
		--target wasm32-unknown-unknown

.PHONY: build-wasm-addr
build-wasm-addr: ## Build the address-validator wasm into the frontend
	cd crates/faucet-addr-wasm && wasm-pack build --target web --release \
		--out-dir ../../frontend/src/lib/wasm

.PHONY: build-worker
build-worker: ## Build the Cloudflare Worker (wasm32)
	cargo build -p faucet-worker --target wasm32-unknown-unknown

## --- Frontend (SvelteKit) ---

.PHONY: frontend-install
frontend-install: ## Install frontend dependencies
	cd frontend && npm ci

.PHONY: frontend-dev
frontend-dev: ## Run the SvelteKit dev server
	cd frontend && npm run dev

.PHONY: frontend-build
frontend-build: build-wasm-addr ## Build the frontend for production
	cd frontend && npm run build

.PHONY: frontend-check
frontend-check: build-wasm-addr ## Type-check the frontend (svelte-check)
	cd frontend && npm run check

.PHONY: frontend-check-format
frontend-check-format: ## Check frontend formatting (prettier)
	cd frontend && npm run check-format

## --- Shell / docs lint ---

.PHONY: lint-shell
lint-shell: ## Lint all tracked shell scripts with shellcheck
	@scripts=$$(git ls-files '*.sh'); \
	if [ -z "$$scripts" ]; then \
		echo "No shell scripts to lint."; \
	else \
		shellcheck $$scripts; \
	fi

.PHONY: check-format-md
check-format-md: ## Check markdown/yaml formatting
	npx prettier --check "**/*.md" "**/*.yaml" "**/*.yml"

## --- Local dev stack ---

.PHONY: stack-up
stack-up: ## Start zcashd + lightwalletd + signer locally
	cd deploy && docker compose up -d

.PHONY: stack-down
stack-down: ## Stop the local stack
	cd deploy && docker compose down

## --- Aggregate ---

.PHONY: ci
ci: check-format lint lint-wasm test build-wasm deny ## Run the core CI checks locally

.PHONY: clean
clean: ## Remove build artifacts
	cargo clean
	rm -rf frontend/.svelte-kit frontend/build crates/faucet-addr-wasm/pkg
