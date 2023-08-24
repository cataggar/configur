// https://nodejs.org/api/wasi.html

import { readFile } from "node:fs/promises";
import { WASI } from "wasi";
import { argv, env } from "node:process";

// get the index for the first argument that ends in main.js
const index = argv.findIndex((arg) => arg.endsWith("main.js"));
const args = argv.slice(index + 1);

const wasi = new WASI({
  version: "preview1",
  args,
  env,
  preopens: {
    "/ev2": "C:/Users/cataggar/ev2",
  },
});

const wasm = await WebAssembly.compile(
  await readFile(
    new URL("target/wasm32-wasi/release/configur.wasm", import.meta.url)
  )
);
const instance = await WebAssembly.instantiate(wasm, wasi.getImportObject());

wasi.start(instance);
