- [ ] add a tiny http server with a blank page to hold the canvas

- [ ] add typescript pipeline
    - [ ] check out xtask for compiling the typescript source?
    - [ ] look at `tsify` (alternate `ts-rs`) for exporting Rust types as typescript definitions
        - NOTE: Not exporting any functions or wasm, just vanilla JS created by a typescript compiler, communicating with the web server via plain JSON payloads

- [ ] add features to the visualizer
    - [ ] display the bucket-spigot network
    - [ ] add hover menus to display information about specific nodes
    - [ ] add UI to modify the network
