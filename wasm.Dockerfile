FROM scratch
COPY target/wasm32-wasi/release/configur.wasm /configur.wasm
ENTRYPOINT ["/configur.wasm"]
CMD ["--help"]