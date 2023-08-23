FROM scratch
COPY target/x86_64-unknown-linux-musl/release/configur /configur
ENTRYPOINT ["/configur"]
CMD ["--help"]