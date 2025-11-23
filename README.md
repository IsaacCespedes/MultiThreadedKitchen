# README

Author: `Isaac Cespedes`

## How to run

The `Dockerfile` defines a self-contained Rust reference environment.
Build and run the program using [Docker](https://docs.docker.com/get-started/get-docker/):
```bash
$ docker build -t challenge .
$ docker run --rm -it challenge --auth=<token>
```


If rust `1.89` or later is locally installed, run the program directly for convenience:
```bash
$ cargo run -- --auth=<your token>
```

Additional command-line options are available:
- `--endpoint <url>`: Challenge server endpoint
- `--name <name>`: Problem name (optional)
- `--seed <seed>`: Problem seed (optional)
- `--rate <ms>`: Order placement rate in milliseconds (default: 500)
- `--min <seconds>`: Minimum pickup time in seconds (default: 4)
- `--max <seconds>`: Maximum pickup time in seconds (default: 8)

## Discard Criteria

When the shelf is full and a new order must be placed, the system selects the order to discard using a priority queue (minheap) ordered by expiration time. The order that expires earliest (or has already expired) is discarded.
