import { MemoryFiles, runWasi } from "./wasi.mjs";

const text = new TextDecoder();

export async function sha256(bytes) {
  return [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
    .map((x) => x.toString(16).padStart(2, "0"))
    .join("");
}

export async function convert(input, { asset, progress = () => {} }) {
  if (!(input instanceof Uint8Array) || input.length > 128 * 1024 * 1024) {
    throw new Error("ZIP은 128 MiB 이하로 선택해 주세요.");
  }
  const start = performance.now();
  progress(1, "원본 파일 확인");
  const files = new MemoryFiles();
  files.put("game.zip", input);
  const builder = await WebAssembly.compile(await asset("builder.wasm"));
  const validation = JSON.parse(
    await runWasi(builder, ["check-kr", "/game.zip"], files),
  );
  const inputHash = await sha256(input);
  progress(8, "음악 변환 준비");
  const [compiler, contract, music] = await Promise.all([
    asset("tad.wasm").then((x) => WebAssembly.compile(x)),
    asset("audio.json").then((x) => JSON.parse(text.decode(x))),
    asset("music.json").then((x) => JSON.parse(text.decode(x))),
  ]);
  await runWasi(builder, ["prepare-audio", "/game.zip", "/audio"], files);
  for (const [name, hash] of Object.entries(contract.modules)) {
    const bytes = await asset(name);
    if ((await sha256(bytes)) !== hash)
      throw new Error("사운드 엔진 파일이 일치하지 않습니다.");
    files.put("audio/" + name, bytes);
  }
  let count = 0;
  for (const song of contract.songs) {
    const project = "/audio/" + song.name + ".terrificaudio";
    const songId = "song_" + song.name.replaceAll("-", "_");
    for (const kind of ["common", "song"]) {
      const path = "/audio/" + song[kind].file;
      await runWasi(
        compiler,
        [kind, project, ...(kind === "song" ? [songId] : []), "--output", path],
        files,
      );
      if ((await sha256(files.get(path))) !== song[kind].sha256) {
        throw new Error(`음악 변환 결과 불일치: ${song.name} ${kind}`);
      }
    }
    count++;
    progress(
      10 + Math.round((count / contract.songs.length) * 40),
      `음악 변환 ${count}/${contract.songs.length}`,
    );
  }
  files.put("font.ttf", await asset("neodgm.ttf"));
  files.put("dialogue.ttf", await asset("Galmuri11.ttf"));
  progress(55, "게임 데이터와 ROM 생성");
  await runWasi(
    builder,
    [
      "build-kr-tad",
      "--game",
      "/game.zip",
      "--audio",
      "/audio",
      "--output",
      "/output",
      "--font",
      "/font.ttf",
      "--dialogue-font",
      "/dialogue.ttf",
    ],
    files,
  );
  const rom = files.get("/output/madou-hirom.sfc");
  const report = JSON.parse(
    text.decode(files.get("/output/madou-hirom-build.json")),
  );
  const hash = await sha256(rom);
  if (
    rom.length !== 4194304 ||
    hash !== report.rom_sha256 ||
    report.b1.sound.external_assets
  ) {
    throw new Error("ROM 결과 검증에 실패했습니다.");
  }
  progress(100, "완료");
  return {
    rom,
    receipt: {
      schema: 1,
      complete: true,
      input_sha256: inputHash,
      input_contract: validation.contract.id,
      executable_sha256: validation.executable_sha256,
      required_files: validation.files,
      rom_sha256: hash,
      rom_bytes: rom.length,
      backend: report.b1.sound.backend,
      music_baseline: music.id,
      music_contract_sha256: report.b1.sound.contract_sha256,
      elapsed_seconds: Math.round((performance.now() - start) / 100) / 10,
      scope: "Local browser conversion; no original files uploaded.",
    },
  };
}
