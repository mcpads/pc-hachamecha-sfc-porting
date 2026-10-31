import {
  WASI,
  File,
  Directory,
  OpenFile,
  ConsoleStdout,
  PreopenDirectory,
  wasi as constants,
} from "@bjorn3/browser_wasi_shim";
export class MemoryRoot extends PreopenDirectory {
  path_readlink(path) {
    const { ret, filestat } = this.path_filestat_get(0, path);
    return { ret: filestat ? constants.ERRNO_INVAL : ret, data: null };
  }
}
export class MemoryFiles {
  constructor() {
    this.root = new Map();
  }
  directory(path) {
    let entries = this.root;
    for (const part of path.split("/").filter(Boolean)) {
      if (part === "." || part === "..") throw new Error("Unsafe virtual path");
      if (!entries.has(part)) entries.set(part, new Directory(new Map()));
      const child = entries.get(part);
      if (!(child instanceof Directory)) throw new Error("Expected directory");
      entries = child.contents;
    }
    return entries;
  }
  put(path, bytes) {
    const parts = path.split("/").filter(Boolean);
    const name = parts.pop();
    if (!name || name === "." || name === "..")
      throw new Error("Unsafe virtual filename");
    this.directory(parts.join("/")).set(name, new File(bytes));
  }
  get(path) {
    let entry = { contents: this.root };
    for (const part of path.split("/").filter(Boolean))
      entry = entry?.contents?.get(part);
    if (!(entry instanceof File)) throw new Error(`Missing output: ${path}`);
    return entry.data;
  }
}
export async function runWasi(module, args, files, log = () => {}) {
  let output = "";
  const append = (line) => {
    output += line + "\n";
    log(line);
  };
  const wasi = new WASI(
    ["converter", ...args],
    [],
    [
      new OpenFile(new File([])),
      ConsoleStdout.lineBuffered(append),
      ConsoleStdout.lineBuffered(append),
      new MemoryRoot("/", files.root),
    ],
    { debug: false },
  );
  const instance = await WebAssembly.instantiate(module, {
    wasi_snapshot_preview1: wasi.wasiImport,
  });
  const code = wasi.start(instance);
  if (code !== 0)
    throw new Error(output.trim() || `Conversion failed (${code})`);
  return output;
}
