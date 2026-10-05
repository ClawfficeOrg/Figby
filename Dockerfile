# syntax=docker/dockerfile:1
FROM rust:bookworm AS build
WORKDIR /src
# rust-toolchain.toml pins the toolchain; rustup installs it on first use.
COPY rust-toolchain.toml ./
COPY figby-rs ./figby-rs
RUN cargo build --release --manifest-path figby-rs/Cargo.toml --bin figby

FROM debian:bookworm-slim
# figby links libfontconfig (system font discovery for TTF/OTF conversion).
RUN apt-get update && apt-get install -y --no-install-recommends libfontconfig1 \
 && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/figby-rs/target/release/figby /usr/local/bin/figby
# Default figby font dirs; compose mounts the font collections here.
RUN mkdir -p /usr/local/share/figlet /usr/share/figlet
ENV TERM=xterm-256color
WORKDIR /work
ENTRYPOINT ["figby"]
CMD ["--tui"]
