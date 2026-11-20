.PHONY: linux windows deps test clean help

# Detect the host: "Windows_NT" is set by Windows (make via Git Bash/MSYS/Chocolatey).
ifeq ($(OS),Windows_NT)
  HOST := windows
else
  HOST := $(shell uname -s | tr A-Z a-z)
endif

help:
	@echo "make linux    - build the standalone Linux executable    (run on Linux)"
	@echo "make windows  - build the standalone Windows executable  (native on Windows, cross-compiled on Linux)"
	@echo "make test     - run the Rust core tests and frontend checks"
	@echo "No installers are produced: only the executable (mpv/ffmpeg are not included)."

deps:
	pnpm install

test:
	cargo test -p vp-core
	pnpm check

# ---------------------------------------------------------------- Linux
# Needs: rustup (Rust >= 1.90), node >= 20 and
#   sudo apt install build-essential pkg-config libwebkit2gtk-4.1-dev libxdo-dev \
#        libssl-dev libayatana-appindicator3-dev librsvg2-dev mpv
# Output: target/release/kadence-media
linux: deps
ifneq ($(HOST),linux)
	$(error "make linux" must be run on Linux (use WSL or a Debian machine))
endif
	pnpm tauri build --no-bundle
	@echo "Built: target/release/kadence-media"

# -------------------------------------------------------------- Windows
# Native (on Windows 11): needs MSVC Build Tools (C++ workload), Rust >= 1.90, node >= 20.
#   Output: target/release/kadence-media.exe
#
# Cross-compiled (on Linux, experimental): needs
#   sudo apt install llvm clang lld
#   cargo install --locked cargo-xwin
#   rustup target add x86_64-pc-windows-msvc
#   Output: target/x86_64-pc-windows-msvc/release/kadence-media.exe
# mpv is not included: put mpv.exe on PATH, next to the app, or set it in Settings.
windows: deps
ifeq ($(HOST),windows)
	pnpm tauri build --no-bundle
	@echo "Built: target/release/kadence-media.exe"
else
	@missing=""; \
	for c in cargo-xwin clang ld.lld llvm-rc; do command -v $$c >/dev/null 2>&1 || missing="$$missing $$c"; done; \
	if [ -n "$$missing" ]; then \
	  echo "Missing tools:$$missing"; \
	  echo "Install with:  sudo apt install llvm clang lld   &&   cargo install --locked cargo-xwin"; \
	  echo "Then:          rustup target add x86_64-pc-windows-msvc"; \
	  exit 1; \
	fi
	pnpm tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --no-bundle
	@echo "Built: target/x86_64-pc-windows-msvc/release/kadence-media.exe"
endif

clean:
	cargo clean
	rm -rf dist
