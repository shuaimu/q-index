.PHONY: all build release run test bench clean install fmt clippy check help

# Default target
all: build

# Build in debug mode
build:
	cargo build
	@echo "✅ Debug build complete: target/debug/qindex"

# Build in release mode (optimized)
release:
	cargo build --release
	@echo "✅ Release build complete: target/release/qindex"

# Run the application
run: build
	cargo run -- calculate

# Run with custom arguments
run-venues: build
	cargo run -- venues --top 30

run-scholars: build
	cargo run -- scholars --top 30

run-stats: build
	cargo run -- stats

run-search: build
	@read -p "Enter search query: " query; \
	cargo run -- search "$$query"

# Run tests
test:
	cargo test

test-verbose:
	cargo test -- --nocapture

# Run benchmarks
bench:
	cargo bench

# Clean build artifacts
clean:
	cargo clean
	rm -f *.json *.csv

# Install locally
install:
	cargo install --path .

# Format code
fmt:
	cargo fmt

fmt-check:
	cargo fmt -- --check

# Run clippy linter
clippy:
	cargo clippy -- -D warnings

# Check code (compile without building)
check:
	cargo check

# Run all checks (format, clippy, test)
ci: fmt-check clippy test
	@echo "✅ All checks passed!"

# Development mode with auto-reload (requires cargo-watch)
watch:
	@if command -v cargo-watch >/dev/null 2>&1; then \
		cargo watch -x run; \
	else \
		echo "cargo-watch not installed. Install with: cargo install cargo-watch"; \
	fi

# Run web server in development mode with auto-reload
web-dev:
	@if command -v cargo-watch >/dev/null 2>&1; then \
		echo "🚀 Starting development web server with auto-reload..."; \
		echo "🌐 Server will be at: http://localhost:8080"; \
		echo "📝 Watching for changes in src/, bib/, and static/..."; \
		cargo watch -c -w src -w bib -w static -x "run -- web --port 8080"; \
	else \
		echo "cargo-watch not installed. Install with: cargo install cargo-watch"; \
		echo "Falling back to regular web server..."; \
		cargo run -- web --port 8080; \
	fi

# Run web server in production mode
web:
	cargo run --release -- web --port 8080

# Run web server with custom port
web-custom:
	@read -p "Enter port number: " port; \
	cargo run -- web --port $$port

# Generate documentation
doc:
	cargo doc --open

# Update dependencies
update:
	cargo update

# Show outdated dependencies
outdated:
	@if command -v cargo-outdated >/dev/null 2>&1; then \
		cargo outdated; \
	else \
		echo "cargo-outdated not installed. Install with: cargo install cargo-outdated"; \
	fi

# Profile the application (requires perf on Linux)
profile-linux: release
	perf record --call-graph=dwarf target/release/qindex calculate
	perf report

# Profile on macOS (requires Instruments)
profile-macos: release
	instruments -t "Time Profiler" target/release/qindex calculate

# Docker build
docker-build:
	docker build -t qindex:latest .

docker-run:
	docker run --rm -v $(PWD)/bib:/app/bib qindex:latest

# Help
help:
	@echo "QIndex - Rust Build System"
	@echo ""
	@echo "Available targets:"
	@echo "  make build         - Build in debug mode"
	@echo "  make release       - Build in release mode (optimized)"
	@echo "  make run           - Run the calculate command"
	@echo "  make run-venues    - Show top venues"
	@echo "  make run-scholars  - Show top scholars"
	@echo "  make run-stats     - Show statistics"
	@echo "  make run-search    - Search interactively"
	@echo "  make test          - Run tests"
	@echo "  make bench         - Run benchmarks"
	@echo "  make clean         - Clean build artifacts"
	@echo "  make install       - Install locally"
	@echo "  make fmt           - Format code"
	@echo "  make clippy        - Run clippy linter"
	@echo "  make check         - Check code compilation"
	@echo "  make ci            - Run all CI checks"
	@echo "  make watch         - Run with auto-reload (dev mode)"
	@echo "  make doc           - Generate and open documentation"
	@echo "  make update        - Update dependencies"
	@echo "  make outdated      - Show outdated dependencies"
	@echo "  make docker-build  - Build Docker image"
	@echo "  make docker-run    - Run in Docker"
	@echo "  make help          - Show this help message"