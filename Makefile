.PHONY: linux windows deps test clean help nsis-dir

# Detect the host: "Windows_NT" is set by Windows (make via Git Bash/MSYS/Chocolatey).
ifeq ($(OS),Windows_NT)
  HOST := windows
else
  HOST := $(shell uname -s | tr A-Z a-z)
endif

help:
	@echo "make linux    - build the Debian package + AppImage  (run on Linux)"
	@echo "make windows  - build the Windows installer          (native on Windows, cross-compiled on Linux)"
	@echo "make test     - run the Rust core tests and frontend checks"

deps:
	npm install

test:
	cargo test -p vp-core
	npm run check

# ---------------------------------------------------------------- Linux
# Needs: rustup (Rust >= 1.90), node >= 20 and
#   sudo apt install build-essential pkg-config libwebkit2gtk-4.1-dev libxdo-dev \
#        libssl-dev libayatana-appindicator3-dev librsvg2-dev mpv
# Output: target/release/bundle/{deb,appimage}/
linux: deps
ifneq ($(HOST),linux)
	$(error "make linux" must be run on Linux (use WSL or a Debian machine))
endif
	npm run tauri build

# -------------------------------------------------------------- Windows
# Native (on Windows 11): needs MSVC Build Tools (C++ workload), Rust >= 1.90, node >= 20.
#   Output: target/release/bundle/{nsis,msi}/
#
# Cross-compiled (on Linux, experimental): needs
#   sudo apt install nsis llvm clang lld
#   cargo install --locked cargo-xwin
#   rustup target add x86_64-pc-windows-msvc
#   Output: target/x86_64-pc-windows-msvc/release/bundle/nsis/
# The installer does not include mpv: put mpv.exe on PATH, next to the app, or set it in Settings.
windows: deps
ifeq ($(HOST),windows)
	npm run tauri build
else
	@missing=""; \
	for c in cargo-xwin makensis clang ld.lld llvm-rc; do command -v $$c >/dev/null 2>&1 || missing="$$missing $$c"; done; \
	if [ -n "$$missing" ]; then \
	  echo "Missing tools:$$missing"; \
	  echo "Install with:  sudo apt install nsis llvm clang lld   &&   cargo install --locked cargo-xwin"; \
	  echo "Then:          rustup target add x86_64-pc-windows-msvc"; \
	  exit 1; \
	fi
	$(MAKE) nsis-dir
	PATH="$(CURDIR)/target/nsis-bin:$$PATH" npm run tauri build -- --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis
endif

# Debian 12 ships NSIS 3.08, which lacks Include/Win/RestartManager.nsh (needed by Tauri's
# installer script). Use a private copy of NSIS's data dir with that header added (no sudo).
NSIS_SRC ?= /usr/share/nsis
nsis-dir:
	rm -rf target/nsis-dir && mkdir -p target
	cp -r $(NSIS_SRC) target/nsis-dir
	cp packaging/nsis/Include/Win/RestartManager.nsh target/nsis-dir/Include/Win/
	# Wrapper first in PATH: Tauri calls `makensis`, which then uses the patched data dir.
	mkdir -p target/nsis-bin
	printf '#!/bin/sh\nNSISDIR="%s" exec "%s" "$$@"\n' "$(CURDIR)/target/nsis-dir" "$$(command -v makensis)" > target/nsis-bin/makensis
	chmod +x target/nsis-bin/makensis

clean:
	cargo clean
	rm -rf dist
