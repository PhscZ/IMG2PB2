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
cargo build --release --target wasm32-unknown-unknown -p img2pb2-web
wasm-bindgen --target web --out-dir web\pkg --out-name img2pb2_web target\wasm32-unknown-unknown\release\img2pb2_web.wasm
```

### Serve

```sh
python -m http.server --directory web 8080
```

Then open http://localhost:8080/.
