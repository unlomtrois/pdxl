// Extension-managed server binary: resolve / download / install.
//
// The low-level helpers (targetTriple, verifySha256, extractSingleFileTarGz)
// are pure and vscode-free so they can be exercised with plain node. The
// release contract they rely on (see .github/workflows/release.yml): assets
// named `pdxl-v<VERSION>-<TARGET>.tar.gz` with a `.sha256` sidecar, tar.gz on
// every platform, containing exactly one file (`pdxl` / `pdxl.exe`) at the
// archive root.

import * as crypto from "crypto";
import * as fs from "fs";
import * as path from "path";
import * as zlib from "zlib";
import { spawnSync } from "child_process";
import type * as vscode from "vscode";

const OWNER = "unlomtrois";
const REPO = "pdxl";
const RELEASES_API = `https://api.github.com/repos/${OWNER}/${REPO}/releases`;

export interface ResolvedServer {
  command: string;
  source: "setting" | "managed" | "path";
}

function exeName(): string {
  return process.platform === "win32" ? "pdxl.exe" : "pdxl";
}

/** The managed binary's path for one server version. The per-version
 *  directory doubles as the version marker: an extension update looks for its
 *  own `v<version>` dir, misses, and reinstalls. */
export function managedBinaryPath(
  ctx: vscode.ExtensionContext,
  version: string,
): string {
  return path.join(ctx.globalStorageUri.fsPath, "bin", `v${version}`, exeName());
}

/** The installed managed binary: the exact pinned version when present,
 *  otherwise any other installed version (a fallback install of the latest
 *  release lives in its own `v<actual>` dir and must still be found on the
 *  next activation). */
function findManagedBinary(
  ctx: vscode.ExtensionContext,
  version: string,
): string | undefined {
  const exact = managedBinaryPath(ctx, version);
  if (fs.existsSync(exact)) return exact;
  const binDir = path.join(ctx.globalStorageUri.fsPath, "bin");
  let entries: string[];
  try {
    entries = fs.readdirSync(binDir);
  } catch {
    return undefined;
  }
  for (const entry of entries.sort().reverse()) {
    if (!entry.startsWith("v")) continue;
    const candidate = path.join(binDir, entry, exeName());
    if (fs.existsSync(candidate)) return candidate;
  }
  return undefined;
}

/** Whether `name` (a bare command, no separators) is runnable from PATH. */
function onPath(name: string): boolean {
  const probe = spawnSync(name, ["--version"], { timeout: 5000 });
  return probe.error === undefined && probe.status === 0;
}

/** Resolves the server command: explicit setting → managed binary → PATH.
 *  A non-empty setting always wins: absolute/relative paths must exist on
 *  disk; a bare name is treated as a PATH lookup. Returns `undefined` when
 *  nothing is runnable (the caller offers to install). */
export function resolveServer(
  ctx: vscode.ExtensionContext,
  serverPathSetting: string,
  version: string,
): ResolvedServer | undefined {
  const setting = serverPathSetting.trim();
  if (setting.length > 0) {
    if (setting.includes(path.sep) || setting.includes("/")) {
      return fs.existsSync(setting)
        ? { command: setting, source: "setting" }
        : undefined;
    }
    return onPath(setting) ? { command: setting, source: "setting" } : undefined;
  }
  const managed = findManagedBinary(ctx, version);
  if (managed) {
    return { command: managed, source: "managed" };
  }
  if (onPath("pdxl")) {
    return { command: "pdxl", source: "path" };
  }
  return undefined;
}

/** Maps a node platform/arch pair onto a release target triple, or
 *  `undefined` for combinations the release workflow does not build. */
export function targetTriple(
  platform: NodeJS.Platform = process.platform,
  arch: string = process.arch,
): string | undefined {
  switch (`${platform}-${arch}`) {
    case "linux-x64":
      return "x86_64-unknown-linux-gnu";
    case "linux-arm64":
      return "aarch64-unknown-linux-gnu";
    case "darwin-x64":
      return "x86_64-apple-darwin";
    case "darwin-arm64":
      return "aarch64-apple-darwin";
    case "win32-x64":
      return "x86_64-pc-windows-msvc";
    default:
      return undefined;
  }
}

/** Checks `data` against a `<hex>  <filename>` sha256 sidecar; throws on
 *  mismatch or an unparsable sidecar. */
export function verifySha256(data: Buffer, sidecar: string): void {
  const expected = sidecar.trim().split(/\s+/)[0]?.toLowerCase();
  if (!expected || !/^[0-9a-f]{64}$/.test(expected)) {
    throw new Error(`unparsable sha256 sidecar: ${JSON.stringify(sidecar.slice(0, 80))}`);
  }
  const actual = crypto.createHash("sha256").update(data).digest("hex");
  if (actual !== expected) {
    throw new Error(`sha256 mismatch: expected ${expected}, got ${actual}`);
  }
}

/** Extracts the single regular file from a tar.gz produced by the release
 *  workflow. Minimal ustar walk (name at 0–100, octal size at 124–136,
 *  typeflag at 156); pax/dir/extended entries are skipped. */
export function extractSingleFileTarGz(archive: Buffer): {
  name: string;
  data: Buffer;
} {
  const tar = zlib.gunzipSync(archive);
  let off = 0;
  while (off + 512 <= tar.length) {
    const header = tar.subarray(off, off + 512);
    if (header.every((b) => b === 0)) {
      break; // end-of-archive zero blocks
    }
    const name = header.subarray(0, 100).toString("utf8").replace(/\0.*$/, "");
    const size = parseInt(
      header.subarray(124, 136).toString("ascii").replace(/\0.*$/, "").trim(),
      8,
    );
    const typeflag = String.fromCharCode(header[156]);
    if (Number.isNaN(size)) {
      throw new Error(`corrupt tar header at offset ${off}`);
    }
    const dataStart = off + 512;
    // '0' and '\0' are regular files; everything else (dirs, pax headers,
    // long-name entries) is skipped.
    if (typeflag === "0" || typeflag === "\0") {
      return {
        name: path.posix.basename(name),
        data: Buffer.from(tar.subarray(dataStart, dataStart + size)),
      };
    }
    off = dataStart + Math.ceil(size / 512) * 512;
  }
  throw new Error("no regular file found in archive");
}

/** The version of the newest published release (`"0.58.1"`), or `undefined`
 *  when there is none or its tag isn't a plain `vX.Y.Z`. */
export async function latestReleaseVersion(): Promise<string | undefined> {
  const release = await githubJson(`${RELEASES_API}/latest`);
  const tag = release?.tag_name;
  return tag && /^v\d+\.\d+\.\d+$/.test(tag) ? tag.slice(1) : undefined;
}

/** Whether semver-ish `a` is strictly newer than `b` (numeric x.y.z parts;
 *  missing parts count as 0). */
export function isNewerVersion(a: string, b: string): boolean {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const da = pa[i] ?? 0;
    const db = pb[i] ?? 0;
    if (da !== db) return da > db;
  }
  return false;
}

/** A GitHub release as far as the installer cares. */
interface Release {
  tag_name: string;
  assets: { name: string; browser_download_url: string }[];
}

/** GETs a GitHub API JSON resource; `undefined` on 404, throws otherwise. */
async function githubJson(url: string): Promise<Release | undefined> {
  const res = await fetch(url, {
    headers: {
      Accept: "application/vnd.github+json",
      "User-Agent": "pdxl-vscode-extension",
    },
  });
  if (res.status === 404) return undefined;
  if (!res.ok) {
    throw new Error(`GitHub API ${res.status} ${res.statusText} for ${url}`);
  }
  return (await res.json()) as Release;
}

async function fetchBuffer(
  url: string,
  onChunk?: (received: number, total: number | undefined) => void,
): Promise<Buffer> {
  const res = await fetch(url);
  if (!res.ok || !res.body) {
    throw new Error(`download failed (${res.status} ${res.statusText}): ${url}`);
  }
  const total = Number(res.headers.get("content-length")) || undefined;
  const chunks: Uint8Array[] = [];
  let received = 0;
  const reader = res.body.getReader();
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
    received += value.length;
    onChunk?.(received, total);
  }
  return Buffer.concat(chunks);
}

/** Downloads and installs the release binary for `version` into the managed
 *  location. Returns the installed binary path; throws an `Error` whose
 *  message is user-readable. */
export async function installServer(
  ctx: vscode.ExtensionContext,
  version: string,
  progress?: (message: string, percent?: number) => void,
): Promise<string> {
  const triple = targetTriple();
  if (!triple) {
    throw new Error(
      `no prebuilt pdxl binary for ${process.platform}-${process.arch}; ` +
        `build from source and set pdxl.serverPath`,
    );
  }

  progress?.(`looking up release v${version}…`);
  // Two unauthenticated JSON GETs against the public API — a full GitHub
  // client would add ~95 KB to the bundle for exactly this.
  let release = await githubJson(`${RELEASES_API}/tags/v${version}`);
  if (!release) {
    // The pinned tag may not exist (an extension build published between
    // server releases). Fall back to the newest release rather than failing.
    progress?.(`release v${version} not found — using the latest release…`);
    release = await githubJson(`${RELEASES_API}/latest`);
    if (!release) {
      throw new Error(`no releases found on github.com/${OWNER}/${REPO}`);
    }
    const tag = release.tag_name;
    if (!/^v\d+\.\d+\.\d+$/.test(tag)) {
      throw new Error(`latest release has unexpected tag ${JSON.stringify(tag)}`);
    }
    version = tag.slice(1);
  }

  const archiveName = `pdxl-v${version}-${triple}.tar.gz`;
  const assets = release.assets;
  const archiveAsset = assets.find((a) => a.name === archiveName);
  const sidecarAsset = assets.find((a) => a.name === `${archiveName}.sha256`);
  if (!archiveAsset) {
    throw new Error(`release v${version} has no asset ${archiveName}`);
  }

  progress?.(`downloading ${archiveName}…`, 0);
  const archive = await fetchBuffer(
    archiveAsset.browser_download_url,
    (received, total) => {
      if (total) {
        progress?.(
          `downloading ${archiveName}…`,
          Math.round((received / total) * 100),
        );
      }
    },
  );
  if (sidecarAsset) {
    const sidecar = await fetchBuffer(sidecarAsset.browser_download_url);
    verifySha256(archive, sidecar.toString("utf8"));
  }

  progress?.("extracting…");
  const { data } = extractSingleFileTarGz(archive);

  // Atomic install: write next to the final path, chmod, rename.
  const finalPath = managedBinaryPath(ctx, version);
  fs.mkdirSync(path.dirname(finalPath), { recursive: true });
  const tmpPath = `${finalPath}.tmp-${process.pid}`;
  fs.writeFileSync(tmpPath, data);
  if (process.platform !== "win32") {
    fs.chmodSync(tmpPath, 0o755);
  }
  fs.renameSync(tmpPath, finalPath);
  cleanupOldVersions(ctx, version);
  return finalPath;
}

/** Best-effort removal of previously installed version directories. */
function cleanupOldVersions(ctx: vscode.ExtensionContext, keep: string): void {
  const binDir = path.join(ctx.globalStorageUri.fsPath, "bin");
  let entries: string[];
  try {
    entries = fs.readdirSync(binDir);
  } catch {
    return;
  }
  for (const entry of entries) {
    if (entry.startsWith("v") && entry !== `v${keep}`) {
      try {
        fs.rmSync(path.join(binDir, entry), { recursive: true, force: true });
      } catch {
        // Best-effort: a locked old binary is harmless.
      }
    }
  }
}
