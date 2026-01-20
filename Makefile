.PHONY: help test lint check clean all install

# Default target
help:
	@echo "Available targets:"
	@echo "  make check     - Run formatting check, linting, and tests"
	@echo "  make test      - Run all tests"
	@echo "  make lint      - Run clippy and formatting check"
	@echo "  make install   - Build release binary and run install script"
	@echo "  make clean     - Clean build artifacts"
	@echo "  make all       - Format, lint, test, and build release"

# Run all checks (formatting, linting, tests)
check:
	@echo "=== Checking code formatting ==="
	@cargo fmt --check
	@echo "\n=== Running clippy ==="
	@cargo clippy --all-targets --all-features -- -D warnings
	@echo "\n=== Running tests ==="
	@cargo test
	@echo "\n✓ All checks passed!"

# Run tests only
test:
	@cargo test

# Run linting (clippy + format check)
lint:
	@echo "=== Checking code formatting ==="
	@cargo fmt --check
	@echo "\n=== Running clippy ==="
	@cargo clippy --all-targets --all-features -- -D warnings
	@echo "\n✓ Linting passed!"

# Build release and run install script
install:
	@echo "=== Building release binary ==="
	@cargo build --release
	@echo "\n=== Running install script ==="
	@./install.sh

# Clean build artifacts
clean:
	@cargo clean
	@rm -rf target/

# Full CI pipeline - format, lint, test, and build
all:
	@echo "=== Formatting code ==="
	@cargo fmt
	@echo "\n=== Running clippy ==="
	@cargo clippy --all-targets --all-features -- -D warnings
	@echo "\n=== Running tests ==="
	@cargo test
	@echo "\n=== Building release binary ==="
	@cargo build --release
	@echo "\n✓ All steps completed successfully!"
