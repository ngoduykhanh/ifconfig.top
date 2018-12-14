FROM rust:1.31

WORKDIR /ifconfig
COPY . /ifconfig

RUN cargo clean && cargo build --release

CMD ["target/release/ifconfig_dot_top"]
