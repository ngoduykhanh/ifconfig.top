FROM rust:1.57.0-alpine3.14 as builder
LABEL maintainer="Khanh Ngo <k@ndk.name"

WORKDIR /build

# build depenedencies
RUN apk add build-base

# copy source code
COPY . /build

# build
RUN cargo clean && cargo build --release

FROM alpine:3.14

# create user and group
RUN addgroup -S ifconfig && \
    adduser -S -D -G ifconfig ifconfig

WORKDIR /app

# copy binary file from builder stage
COPY --from=builder --chown=ifconfig:ifconfig /build/static /app/static/
COPY --from=builder --chown=ifconfig:ifconfig /build/templates /app/templates/
COPY --from=builder --chown=ifconfig:ifconfig /build/GeoLite2-Country.mmdb /app
COPY --from=builder --chown=ifconfig:ifconfig /build/target/release/ifconfig_dot_top /app

RUN chmod +x ifconfig_dot_top

EXPOSE 5000/tcp
ENTRYPOINT ["/app/ifconfig_dot_top"]
