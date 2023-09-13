#!/usr/bin/env -S node --no-warnings

// https://nodejs.org/api/wasi.html

import { readFile } from "node:fs/promises";
import { WASI } from "wasi";
import { argv, env } from "node:process";
import { parseArgs } from "node:util";
import { spawn } from  "node:child_process";
import { dirname, join, isAbsolute } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));

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
  verbose: {
    short: "v",
    type: "boolean",
  },
  wasmtime: {
    short: "w",
    type: "boolean",
  },
  ev2: {
    type: "string",
  },
  environments: {
    short: "e",
    type: "string",
  },
  scratch: {
    short: "s",
    type: "string",
  }
};

try {
  const pargs = parseArgs({ args, options });
  const ev2 = pargs.values.ev2;
  const full_ev2 = !ev2 || isAbsolute(ev2) ? ev2 : join(process.cwd(), ev2);

  // check if we should run wasmtime
  let wasmtime = pargs.values.wasmtime;
  if (!wasmtime) {
    // use wasmtime on Windows by default
    // Node.js for Windows does is missing some WASI functionality
    // https://github.com/nodejs/help/issues/4231
    wasmtime = process.platform === 'win32';
  }

  const configurArgs = !ev2 ? ["--help"] : ["--ev2", full_ev2];
  const environmentsArgs = pargs.values.environments ? ["--environments", pargs.values.environments] : [];
  const scratchArgs = pargs.values.scratch ? ["--scratch", pargs.values.scratch] : [];

  const wasmPath = "target/wasm32-wasi/release/configur.wasm";

  if (wasmtime) {

    // wasmtime run --dir $EV2 target\wasm32-wasi\release\configur.wasm -- --ev2 $EV2
    const dirArgs = ev2 ? ["--dir", full_ev2] : [];
    let childArgs = ["run", ...dirArgs, join(__dirname, wasmPath), "--",
      ...configurArgs, ...environmentsArgs, ...scratchArgs];
    const child = spawn("wasmtime", childArgs);
    child.stdout.pipe(process.stdout);
    child.stderr.pipe(process.stderr);
    child.on('exit', function() {
      process.exit();
    })

  } else {

    const wargs = [ "configur", ...configur_args, ...environmentsArgs, ...scratchArgs];
    const wasi = new WASI({
      version: "preview1",
      args: wargs,
      env,
      preopens: {
        "/ev2": full_ev2,
      },
    });
  
    const wasm = await WebAssembly.compile(
      await readFile(
        new URL(wasmPath, import.meta.url)
      )
    );
    const instance = await WebAssembly.instantiate(wasm, wasi.getImportObject());
  
    wasi.start(instance);
  }


} catch (e) {
  console.error(e.message);
}
