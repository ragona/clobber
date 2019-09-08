testrun:
	cat tests/GET | cargo run --release -- -t=127.0.0.1:8000
