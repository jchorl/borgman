rust:
	docker run -it --rm \
		-v $(PWD):/borgman \
		-w /borgman \
		-u 1000:1000 \
		rust:1.42 \
		bash
