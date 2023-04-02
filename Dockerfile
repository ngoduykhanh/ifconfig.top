FROM rust:1.68.2-alpine3.17 as builder
LABEL maintainer="PaperDragon <2678885646@qq.com> && Khanh Ngo <k@ndk.name>"

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
COPY --from=builder --chown=ifconfig:ifconfig /build/target/release/ifconfig_dot_icu /app

RUN chmod +x ifconfig_dot_icu

EXPOSE 5000/tcp
ENTRYPOINT ["/app/ifconfig_dot_icu"]
