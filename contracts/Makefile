SUBDIRS = adapters router deployer
default: build

all: test

# One workspace-wide build first: adapter and deployer test suites import the
# .wasm artifacts of OTHER crates, so every artifact must exist before any
# suite compiles. Per-crate builds in sequence cannot satisfy that.
build:
	stellar contract build

test: build
	cargo test --workspace

fmt:
	@for dir in $(SUBDIRS) ; do \
		$(MAKE) -C $$dir fmt || break; \
	done

clean:
	cargo clean
