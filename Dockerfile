FROM rust:1.42 as borgman
WORKDIR /build
COPY . .
RUN cargo build --release

FROM alpine:3.11 as rclone
RUN apk add curl unzip
WORKDIR /rclone
RUN curl -O https://downloads.rclone.org/rclone-current-linux-amd64.zip
RUN unzip -j rclone-current-linux-amd64.zip

FROM ubuntu:19.10
RUN apt-get update && apt-get install -y \
    borgbackup \
    ca-certificates
COPY --from=rclone /rclone/rclone /bin/
COPY --from=borgman /build/target/release/borgman /bin/
ENTRYPOINT ["/bin/borgman"]
