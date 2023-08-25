#!/usr/bin/env -S node --no-warnings

// https://nodejs.org/api/wasi.html

import { readFile } from "node:fs/promises";
import { WASI } from "wasi";
import { argv, env } from "node:process";
import { parseArgs } from "node:util";
// import path from 'node:path';

// get the index for the first argument that ends in main.js
// console.log(argv);
let index = argv.findIndex((arg) => arg.endsWith("--"));
if (index === -1) {
  index = argv.findIndex((arg) => arg.endsWith("configur.js"));
}
if (index === -1) {
  index = argv.findIndex((arg) => arg.endsWith("configur"));
}
// console.log(index);
const args = argv.slice(index + 1);
// console.log(args);

const options = {
  help: {
    short: "h",
    type: "boolean",
  },
  ev2: {
    type: "string",
  },
  // source: {
  //   short: 's',
  //   type: 'string',
  //   default: 'environments',
  // },
  // target: {
  //   short: 't',
  //   type: 'string',
  //   default: 'scratch',
  // },
};

try {
  const pargs = parseArgs({ args, options });
  // console.log(pargs);
  // console.log(pargs.values.source);
  const ev2 = pargs.values.ev2;
  // const source = path.join("/ev2", pargs.values.source);
  // const target = path.join("/ev2", pargs.values.target);
  if (!ev2) {
    throw new Error("--ev2 must be set");
  }
  // console.log(ev2);
  // console.log(source);
  // console.log(target);

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
} catch (e) {
  console.error(e.message);
}
