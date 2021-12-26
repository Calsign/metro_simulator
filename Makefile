
bootstrap:
	cd cargo && cargo raze
	bazel run //:gen_rust_project

.PHONY: update
