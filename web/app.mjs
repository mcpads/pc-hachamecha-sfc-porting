const $ = (t) => document.getElementById(t);
let worker = null,
  timer = null,
  urls = [];
function clearResult() {
  $("result").hidden = !0;
  for (const t of urls) URL.revokeObjectURL(t);
  urls = [];
}
function idle() {
  (worker?.terminate(),
    (worker = null),
    clearInterval(timer),
    ($("cancel").hidden = !0),
    ($("input").disabled = !1),
    ($("convert").disabled = !$("input").files.length));
}
function error(t) {
  (idle(),
    ($("status").className = "error"),
    ($("status").textContent = t),
    ($("progress").hidden = !0));
}
($("input").addEventListener("change", () => {
  (clearResult(), ($("status").className = ""));
  const t = $("input").files[0];
  (($("filename").textContent = t
    ? `${t.name} \xB7 ${(t.size / 1048576).toFixed(1)} MiB`
    : "\uC120\uD0DD\uD55C \uD30C\uC77C \uC5C6\uC74C"),
    ($("convert").disabled = !t),
    ($("status").textContent =
      "\uC6D0\uBCF8 \uD30C\uC77C\uC740 \uC11C\uBC84\uC5D0 \uC5C5\uB85C\uB4DC\uB418\uC9C0 \uC54A\uC2B5\uB2C8\uB2E4."),
    ($("progress").hidden = !0),
    ($("elapsed").textContent = ""));
}),
  $("convert").addEventListener("click", async () => {
    const t = $("input").files[0];
    if (!t) return;
    if (
      (clearResult(),
      ($("status").className = ""),
      !t.name.toLowerCase().endsWith(".zip"))
    )
      return error(
        "ZIP \uD30C\uC77C\uC744 \uC120\uD0DD\uD574 \uC8FC\uC138\uC694.",
      );
    if (t.size > 128 * 1048576)
      return error(
        "ZIP\uC740 128 MiB \uC774\uD558\uB85C \uC120\uD0DD\uD574 \uC8FC\uC138\uC694.",
      );
    (($("convert").disabled = !0),
      ($("input").disabled = !0),
      ($("cancel").hidden = !1),
      ($("progress").hidden = !1),
      ($("progress").value = 0),
      ($("status").textContent = "\uBCC0\uD658 \uC900\uBE44 \uC911"));
    const o = Date.now();
    timer = setInterval(
      () =>
        ($("elapsed").textContent =
          `\uC9C4\uD589 \uC2DC\uAC04 ${Math.floor((Date.now() - o) / 1e3)}\uCD08`),
      1e3,
    );
    const n = new Worker(new URL("worker.mjs", import.meta.url), {
      type: "module",
    });
    ((worker = n),
      (n.onerror = (e) => {
        worker === n &&
          error(
            e.message ||
              "\uBCC0\uD658 \uC911 \uC624\uB958\uAC00 \uBC1C\uC0DD\uD588\uC2B5\uB2C8\uB2E4.",
          );
      }),
      (n.onmessage = ({ data: e }) => {
        if (
          worker === n &&
          (e.type === "progress" &&
            (($("status").textContent = e.message),
            ($("progress").value = e.percent)),
          e.type === "error" && error(e.message),
          e.type === "complete")
        ) {
          const s = URL.createObjectURL(
              new Blob([e.rom], { type: "application/octet-stream" }),
            ),
            r = URL.createObjectURL(
              new Blob(
                [
                  JSON.stringify(e.receipt, null, 2) +
                    `
`,
                ],
                { type: "application/json" },
              ),
            );
          ((urls = [s, r]),
            ($("rom").href = s),
            ($("receipt").href = r),
            ($("hash").textContent = "SHA-256 " + e.receipt.rom_sha256),
            ($("result").hidden = !1),
            ($("status").textContent = "\uBCC0\uD658 \uC644\uB8CC"),
            ($("progress").value = 100),
            idle());
        }
      }));
    try {
      const e = await t.arrayBuffer();
      worker === n && n.postMessage({ input: e }, [e]);
    } catch (e) {
      error(e.message);
    }
  }),
  $("cancel").addEventListener("click", () => {
    (idle(),
      ($("progress").hidden = !0),
      ($("status").textContent =
        "\uCDE8\uC18C\uD588\uC2B5\uB2C8\uB2E4. \uC6D0\uBCF8 \uD30C\uC77C\uC740 \uBCC0\uACBD\uB418\uC9C0 \uC54A\uC558\uC2B5\uB2C8\uB2E4."));
  }));
