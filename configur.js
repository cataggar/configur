#!/usr/bin/env -S node --no-warnings

// https://nodejs.org/api/wasi.html

import { readFile } from "node:fs/promises";
import { WASI } from "wasi";
import { argv, env } from "node:process";
import { parseArgs } from "node:util";
import { spawn } from  "node:child_process";

// get the index for the first argument that ends in main.js
let index = argv.findIndex((arg) => arg.endsWith("--"));
if (index === -1) {
  index = argv.findIndex((arg) => arg.endsWith("configur.js"));
}
if (index === -1) {
  index = argv.findIndex((arg) => arg.endsWith("configur"));
}
const args = argv.slice(index + 1);

const options = {
  help: {
    short: "h",
    type: "boolean",
  },
  ev2: {
    type: "string",
  },
  wasmtime: {
    short: "w",
    type: "boolean",
  },
};

try {
  const pargs = parseArgs({ args, options });
  const ev2 = pargs.values.ev2;
  if (!ev2) {
    throw new Error("--ev2 must be set");
  }

  // check if we should run wasmtime
  let wasmtime = pargs.values.wasmtime;
  if (wasmtime === undefined) {
    // use wasmtime on Windows by default
    // Node.js for Windows does is missing some WASI functionality
    // https://github.com/nodejs/help/issues/4231
    wasmtime = process.platform === 'win32';
  }

  if (wasmtime) {

    // wasmtime run --dir $EV2 target\wasm32-wasi\release\configur.wasm -- --ev2 $EV2
    let childArgs = ["run", "--dir", ev2, "target/wasm32-wasi/release/configur.wasm", "--", "--ev2", ev2];
    const child = spawn("wasmtime", childArgs);
    child.stdout.pipe(process.stdout);
    child.stderr.pipe(process.stderr);
    child.on('exit', function() {
      process.exit();
    })

  } else {

    const wargs = [
      "configur",
      "--ev2",
      ev2,
      "--source",
      "/ev2/environments",
      "--target",
      "/ev2/scratch",
    ];
    console.log(wargs);
  
    const wasi = new WASI({
      version: "preview1",
      args: wargs,
      env,
      preopens: {
        "/ev2": ev2,
      },
    });
  
    const wasm = await WebAssembly.compile(
      await readFile(
        new URL("target/wasm32-wasi/release/configur.wasm", import.meta.url)
      )
    );
    const instance = await WebAssembly.instantiate(wasm, wasi.getImportObject());
  
    wasi.start(instance);
  }


} catch (e) {
  console.error(e.message);
}
