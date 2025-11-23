FROM rust:1.89-bookworm
WORKDIR /app

COPY Cargo.toml Cargo.lock ./

RUN mkdir src && \
    echo "fn main() {println!(\"hello world!\")}" > src/main.rs

RUN cargo build --release && \
    rm src/*.rs target/release/deps/challenge*

COPY . .
RUN cargo build --release

ENTRYPOINT [ "/app/target/release/challenge" ]
