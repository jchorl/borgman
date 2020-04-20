.DEFAULT_GOAL := run

rust:
	docker run -it --rm \
		-v $(PWD)/.cache:/usr/local/cargo/registry \
		-v $(PWD):/borgman \
		-w /borgman \
		-u 1000:1000 \
		rust:1.42 \
		bash

run:
	docker run -it --rm \
		-v $(PWD)/.cache:/usr/local/cargo/registry \
		-v $(PWD):/borgman \
		-w /borgman \
		-u 1000:1000 \
		rust:1.42 \
		cargo run -- $(RUN_ARGS)
