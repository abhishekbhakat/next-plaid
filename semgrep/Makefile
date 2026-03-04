# semgrep Makefile
# Simple semantic code search tool

BINARY_NAME = semgrep
CARGO_TARGET = target/release/$(BINARY_NAME)
INSTALL_DIR = /usr/local/bin
CARGO_BIN = $(HOME)/.cargo/bin

# Default target
.PHONY: all build install uninstall clean test fmt lint

all: build

build:
	@echo "Building $(BINARY_NAME)..."
	cargo build --release

install: build
	@echo "Installing $(BINARY_NAME)..."
	@if [ -w $(INSTALL_DIR) ]; then \
		cp $(CARGO_TARGET) $(INSTALL_DIR)/$(BINARY_NAME); \
		chmod 755 $(INSTALL_DIR)/$(BINARY_NAME); \
		echo "Installed to $(INSTALL_DIR)/$(BINARY_NAME)"; \
	else \
		mkdir -p $(CARGO_BIN); \
		cp $(CARGO_TARGET) $(CARGO_BIN)/$(BINARY_NAME); \
		chmod 755 $(CARGO_BIN)/$(BINARY_NAME); \
		echo "Installed to $(CARGO_BIN)/$(BINARY_NAME)"; \
		echo "Make sure $(CARGO_BIN) is in your PATH"; \
	fi

uninstall:
	@echo "Uninstalling $(BINARY_NAME)..."
	@rm -f $(INSTALL_DIR)/$(BINARY_NAME)
	@rm -f $(CARGO_BIN)/$(BINARY_NAME)
	@echo "$(BINARY_NAME) uninstalled"

clean:
	cargo clean

test:
	cargo test

fmt:
	cargo fmt

lint:
	cargo clippy -- -D warnings
