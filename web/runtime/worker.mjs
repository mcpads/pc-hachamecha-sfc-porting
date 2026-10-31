import { convert, sha256 } from "./convert.mjs";
import { inputError } from "./errors.mjs";

let started = false;
self.onmessage = async ({ data }) => {
  if (started) return;
  started = true;
  try {
    const base = new URL("./", self.location.href);
    const manifest = await fetch(new URL("manifest.json", base)).then((r) => {
      if (!r.ok) throw Error("변환 파일을 읽지 못했습니다.");
      return r.json();
    });
    const asset = async (path) => {
      const expected = manifest.files[path];
      if (!expected) throw Error("알 수 없는 변환 파일");
      const response = await fetch(new URL(path, base));
      if (!response.ok) throw Error(`변환 파일 로딩 실패: ${path}`);
      const bytes = new Uint8Array(await response.arrayBuffer());
      if (
        bytes.length !== expected.bytes ||
        (await sha256(bytes)) !== expected.sha256
      ) {
        throw Error(`변환 파일 검증 실패: ${path}`);
      }
      return bytes;
    };
    const result = await convert(new Uint8Array(data.input), {
      asset,
      progress: (percent, message) =>
        self.postMessage({ type: "progress", percent, message }),
    });
    result.receipt.site_build = manifest.build;
    self.postMessage(
      { type: "complete", rom: result.rom, receipt: result.receipt },
      [result.rom.buffer],
    );
  } catch (error) {
    self.postMessage({
      type: "error",
      message: inputError(error.message || String(error)),
    });
  }
};
