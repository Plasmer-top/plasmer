FROM rust:1.95-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends mingw-w64 && rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-pc-windows-gnu

ENV CARGO_TERM_COLOR=never
ENV SOURCE_DATE_EPOCH=1700000000
ENV CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc

WORKDIR /src

COPY Cargo.toml Cargo.lock build.rs ./
COPY assets ./assets
COPY src ./src

RUN cargo build --release --target x86_64-pc-windows-gnu

RUN mkdir -p /out && cp target/x86_64-pc-windows-gnu/release/Plasmer.exe /out/Plasmer.exe

FROM debian:bookworm-slim AS artifact

WORKDIR /out

COPY --from=builder /out/Plasmer.exe ./Plasmer.exe

CMD ["sh"]
