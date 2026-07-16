# IMG2PB2

GUI tool for inserting images into PB2 XML files. Runs natively (desktop) or in the browser via WebAssembly.

## Native (desktop)

```sh
cargo run --release
```

## WebAssembly

### One-time setup

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli
```

### Build

```sh
./build-web.sh
```

### Serve

```sh
python -m http.server --directory web 8080
```

Then open http://localhost:8080/.
