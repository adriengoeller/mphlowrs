# Local CD: build the mphlowrs wheel and a mechaphlowers wheel that depends on it
# (`rust` extra), then check them in a clean venv. See docs/wheels.md.

.DEFAULT_GOAL := help
SHELL := /bin/bash
export PATH := /opt/homebrew/opt/rustup/bin:$(PATH)

VENDOR := vendor/mechaphlowers
DIST := $(CURDIR)/dist
# Throwaway venv where the wheels are installed (build/ is git-ignored)
CHECK_VENV := $(CURDIR)/build/wheel-check
PY ?= 3.13
# Same numpy as upstream (see docs/decisions/log.md)
NUMPY ?= 2.2.5
CHECK_PYTHON := $(CHECK_VENV)/bin/python

.PHONY: help  ## Show the targets
help:
	@grep -E '^\.PHONY: [a-z-]+  ## ' $(MAKEFILE_LIST) | sed -E 's/\.PHONY: ([a-z-]+)  ## /  \1\t/' | expand -t 22

.PHONY: wheel-mphlowrs  ## Build the mphlowrs wheel (release, abi3) into dist/
wheel-mphlowrs:
	uv run maturin build --release --out $(DIST)

.PHONY: wheel-mechaphlowers  ## Build the mechaphlowers wheel from the vendor checkout into dist/
wheel-mechaphlowers:
	@git -C $(VENDOR) grep -q '^rust = \["mphlowrs' -- pyproject.toml \
		|| { echo "$(VENDOR) has no 'rust' extra: checkout the rust branch"; exit 1; }
	cd $(VENDOR) && uv build --wheel --out-dir $(DIST)

.PHONY: wheels  ## Build both wheels
wheels: wheel-mphlowrs wheel-mechaphlowers
	@ls -1 $(DIST)/*.whl

.PHONY: install-wheels  ## Install the wheels from dist/ in a clean venv (build/wheel-check)
install-wheels:
	rm -rf $(CHECK_VENV)
	uv venv --python $(PY) $(CHECK_VENV)
	uv pip install --python $(CHECK_PYTHON) --find-links $(DIST) \
		"mechaphlowers[rust]" "numpy==$(NUMPY)" pytest

.PHONY: check-wheels  ## Check that the installed mechaphlowers uses the Rust kernels
check-wheels:
	cd $(CHECK_VENV) && $(CHECK_PYTHON) -c "import mechaphlowers, mphlowrs; \
		from mphlowrs.mechaphlowers_patch import is_enabled; \
		print('mechaphlowers', mechaphlowers.__version__, mechaphlowers.__file__); \
		print('mphlowrs', mphlowrs.__version__, mphlowrs.__file__); \
		assert mechaphlowers.RUST_ENABLED and is_enabled(), 'Rust kernels not enabled'; \
		print('Rust kernels enabled')"
	cd $(CHECK_VENV) && MECHAPHLOWERS_RUST=0 $(CHECK_PYTHON) -c "import mechaphlowers; \
		from mphlowrs.mechaphlowers_patch import is_enabled; \
		assert not is_enabled(), 'MECHAPHLOWERS_RUST=0 ignored'; \
		print('MECHAPHLOWERS_RUST=0: pure Python')"

# pythonpath= overrides upstream `pythonpath = ["src"]`: the tests import the installed wheel
UPSTREAM_PYTEST := $(CHECK_PYTHON) -m pytest test -q -p no:cacheprovider -o pythonpath= -o addopts= \
	-m "not benchmark and not release_test"

.PHONY: test-wheels  ## Run the upstream test suite on the installed wheels, Rust on then off
test-wheels:
	cd $(VENDOR) && $(UPSTREAM_PYTEST)
	cd $(VENDOR) && MECHAPHLOWERS_RUST=0 $(UPSTREAM_PYTEST)

.PHONY: cd  ## Build, install, check and test the wheels
cd: wheels install-wheels check-wheels test-wheels

.PHONY: clean-wheels  ## Remove dist/ and the check venv
clean-wheels:
	rm -rf $(DIST) $(CHECK_VENV)
