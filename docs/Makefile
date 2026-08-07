# Building The Ductus Book. Requires mdbook (cargo install mdbook); the
# ductus-docs preprocessor is built from this workspace on demand.

MDBOOK ?= mdbook

.PHONY: build serve clean

build:
	$(MDBOOK) build

# Rebuild and reload on edit, at http://localhost:3000
serve:
	$(MDBOOK) serve

clean:
	rm -rf book
