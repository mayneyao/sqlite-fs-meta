import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { chmod, mkdir, mkdtemp, readFile, readdir, rm, stat, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const site = fileURLToPath(new URL("../", import.meta.url));
const windows = process.platform === "win32";
const fixtureVersion = "1.2.3";
const nativeCase = windows
  ? { target: "x86_64-pc-windows-msvc", kernel: "Windows", arch: "AMD64" }
  : { target: "aarch64-apple-darwin", kernel: "Darwin", arch: "arm64" };

function command(executable, args, env) {
  return new Promise((resolve, reject) => {
    const child = spawn(executable, args, { env, stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", reject);
    child.on("close", (code) => resolve({ code, stdout, stderr }));
  });
}

async function fixture(
  t,
  {
    platform = nativeCase,
    checksum = true,
    binaryVersion = fixtureVersion,
    missingChecksum = false,
  } = {},
) {
  const root = await mkdtemp(path.join(tmpdir(), "xmeta-installer-test-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const installDir = path.join(root, "user tools", "bin");
  const bin = path.join(root, "mock-bin");
  await mkdir(bin);
  let payload;
  if (windows) {
    const executable = path.join(root, "fixture.exe");
    const source = `public static class Fixture { public static void Main() { System.Console.WriteLine("xmeta ${binaryVersion}"); } }`;
    const built = await command(
      "powershell.exe",
      [
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        `Add-Type -TypeDefinition '${source}' -OutputAssembly $env:XMETA_FIXTURE_BINARY -OutputType ConsoleApplication`,
      ],
      { ...process.env, XMETA_FIXTURE_BINARY: executable },
    );
    assert.equal(built.code, 0, built.stderr);
    payload = await readFile(executable);
  } else {
    payload = Buffer.from(`#!/bin/sh\nprintf 'xmeta ${binaryVersion}\\n'\n`);
    const uname = path.join(bin, "uname");
    await writeFile(
      uname,
      `#!/bin/sh\ncase "$1" in -s) printf '%s\\n' '${platform.kernel}' ;; -m) printf '%s\\n' '${platform.arch}' ;; esac\n`,
    );
    await chmod(uname, 0o755);
  }
  const binary = `xmeta-${platform.target}${windows ? ".exe" : ""}`;
  const digest = createHash("sha256").update(payload).digest("hex");
  const files = new Map([
    ["/latest.txt", Buffer.from(`${fixtureVersion}\n`)],
    [`/v${fixtureVersion}/${binary}`, payload],
    [
      `/v${fixtureVersion}/SHA256SUMS`,
      Buffer.from(missingChecksum ? "" : `${checksum ? digest : "0".repeat(64)}  ${binary}\n`),
    ],
  ]);
  const requests = [];
  const server = createServer((req, res) => {
    requests.push(req.url);
    const content = files.get(req.url);
    res.writeHead(content ? 200 : 404, {
      "Content-Type": req.url?.endsWith(".txt") ? "text/plain" : "application/octet-stream",
    });
    res.end(content ?? "Not found");
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  t.after(() => new Promise((resolve) => server.close(resolve)));
  const base = `http://127.0.0.1:${server.address().port}`;
  const env = {
    ...process.env,
    PATH: `${bin}${path.delimiter}${process.env.PATH}`,
    XMETA_LATEST_URL: `${base}/latest.txt`,
    XMETA_DOWNLOAD_BASE: base,
    XMETA_INSTALL_DIR: installDir,
    XMETA_NO_MODIFY_PATH: "1",
    XMETA_VERSION: "",
  };
  return {
    installDir,
    requests,
    payload,
    installed: path.join(installDir, windows ? "xmeta.exe" : "xmeta"),
    run: (extraEnv = {}) =>
      windows
        ? command(
            "powershell.exe",
            [
              "-NoProfile",
              "-NonInteractive",
              "-ExecutionPolicy",
              "Bypass",
              "-File",
              path.join(site, "public/install.ps1"),
            ],
            { ...env, ...extraEnv },
          )
        : command(
            "sh",
            ["-c", 'cat "$1" | sh', "installer-test", path.join(site, "public/install.sh")],
            { ...env, ...extraEnv },
          ),
  };
}

test("the installer channel and site match the released package version", async () => {
  const cargo = await readFile(path.join(site, "../Cargo.toml"), "utf8");
  const version = cargo.match(/^version = "([^"]+)"/m)[1];
  assert.equal((await readFile(path.join(site, "public/cli/latest.txt"), "utf8")).trim(), version);
  const project = await readFile(path.join(site, "src/data/project.ts"), "utf8");
  assert.match(
    project,
    new RegExp(`export const version = ["']${version.replaceAll(".", "\\.")}["']`),
  );
});

test("piped installer resolves the channel, verifies the download, and installs to a path with spaces", async (t) => {
  const f = await fixture(t);
  const result = await f.run();
  assert.equal(result.code, 0, result.stdout + result.stderr);
  assert.deepEqual(await readFile(f.installed), f.payload);
  assert.equal(
    (await command(f.installed, ["--version"], process.env)).stdout.trim(),
    `xmeta ${fixtureVersion}`,
  );
  if (!windows) assert.equal((await stat(f.installed)).mode & 0o111, 0o111);
  assert.deepEqual(await readdir(f.installDir), [windows ? "xmeta.exe" : "xmeta"]);
  assert.ok(f.requests.includes("/latest.txt"));
});

test("a checksum mismatch leaves an existing installation intact", async (t) => {
  const f = await fixture(t, { checksum: false });
  await mkdir(f.installDir, { recursive: true });
  await writeFile(f.installed, "existing installation");
  const result = await f.run();
  assert.notEqual(result.code, 0);
  assert.match(result.stdout + result.stderr, /checksum mismatch/i);
  assert.equal(await readFile(f.installed, "utf8"), "existing installation");
});

test("a missing checksum entry never installs a binary", async (t) => {
  const f = await fixture(t, { missingChecksum: true });
  const result = await f.run();
  assert.notEqual(result.code, 0);
  assert.match(result.stdout + result.stderr, /no unique entry/i);
  await assert.rejects(stat(f.installed), { code: "ENOENT" });
});

test("a wrong binary version leaves an existing installation intact", async (t) => {
  const f = await fixture(t, { binaryVersion: "9.9.9" });
  await mkdir(f.installDir, { recursive: true });
  await writeFile(f.installed, "existing installation");
  const result = await f.run();
  assert.notEqual(result.code, 0);
  assert.match(result.stdout + result.stderr, /unexpected binary version/i);
  assert.equal(await readFile(f.installed, "utf8"), "existing installation");
});

test("an explicit version skips the channel lookup and can update an installation", async (t) => {
  const f = await fixture(t);
  await mkdir(f.installDir, { recursive: true });
  await writeFile(f.installed, "old version");
  const result = await f.run({ XMETA_VERSION: fixtureVersion });
  assert.equal(result.code, 0, result.stdout + result.stderr);
  assert.deepEqual(await readFile(f.installed), f.payload);
  assert.ok(!f.requests.includes("/latest.txt"));
});

test("download failures preserve the previous installation", async (t) => {
  const f = await fixture(t);
  await mkdir(f.installDir, { recursive: true });
  await writeFile(f.installed, "existing installation");
  const result = await f.run({ XMETA_VERSION: "9.8.7" });
  assert.notEqual(result.code, 0);
  assert.equal(await readFile(f.installed, "utf8"), "existing installation");
});

for (const platform of [
  { kernel: "Darwin", arch: "arm64", target: "aarch64-apple-darwin" },
  { kernel: "Darwin", arch: "x86_64", target: "x86_64-apple-darwin" },
  { kernel: "Linux", arch: "aarch64", target: "aarch64-unknown-linux-gnu" },
  { kernel: "Linux", arch: "x86_64", target: "x86_64-unknown-linux-gnu" },
]) {
  test(`Unix installer selects ${platform.target}`, { skip: windows }, async (t) => {
    const f = await fixture(t, { platform });
    const result = await f.run();
    assert.equal(result.code, 0, result.stdout + result.stderr);
    assert.ok(f.requests.includes(`/v${fixtureVersion}/xmeta-${platform.target}`));
  });
}

test("unsupported Unix platforms fail before downloading", { skip: windows }, async (t) => {
  const f = await fixture(t, {
    platform: { kernel: "FreeBSD", arch: "x86_64", target: "unsupported" },
  });
  const result = await f.run();
  assert.notEqual(result.code, 0);
  assert.match(result.stderr, /unsupported platform/);
  assert.deepEqual(f.requests, []);
});

test("malformed versions fail before downloading", async (t) => {
  const f = await fixture(t);
  const result = await f.run({ XMETA_VERSION: "../0.5.0" });
  assert.notEqual(result.code, 0);
  assert.match(result.stdout + result.stderr, /invalid version/i);
  assert.deepEqual(f.requests, []);
});
