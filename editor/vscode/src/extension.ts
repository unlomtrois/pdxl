import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";
import {
  Game,
  detectGame,
  resolveGamePath,
  installServer,
  isNewerVersion,
  latestReleaseVersion,
  resolveServer,
} from "./install";

let client: LanguageClient | undefined;
let output: vscode.OutputChannel | undefined;
let statusBar: vscode.StatusBarItem | undefined;
let installPromptShown = false;
const unresolvedDocDecoration = vscode.window.createTextEditorDecorationType({
  opacity: "0.45",
});
/** Applies presentation to ranges the server marked with semantic modifier
 * bit 1 (`unresolved`). Resolution remains entirely server-owned. */
function decorateUnresolvedSemanticTokens(
  document: vscode.TextDocument,
  tokens: vscode.SemanticTokens,
): void {
  const ranges: vscode.Range[] = [];
  let line = 0;
  let character = 0;
  const data = tokens.data;
  for (let i = 0; i + 4 < data.length; i += 5) {
    const deltaLine = data[i];
    line += deltaLine;
    character = deltaLine === 0 ? character + data[i + 1] : data[i + 1];
    const length = data[i + 2];
    const modifiers = data[i + 4];
    if ((modifiers & (1 << 1)) !== 0) {
      ranges.push(
        new vscode.Range(line, character, line, character + length),
      );
    }
  }
  for (const editor of vscode.window.visibleTextEditors) {
    if (editor.document.uri.toString() === document.uri.toString()) {
      editor.setDecorations(unresolvedDocDecoration, ranges);
    }
  }
}

/** The game this workspace targets (detected once at activation). */
let game: Game = "ck3";

function log(msg: string): void {
  console.log(`[pdxl] ${msg}`);
  output?.appendLine(`[${new Date().toISOString()}] ${msg}`);
}

/** Updates the status-bar button's icon, tooltip, and error colouring.
 *  `version` (from the server's initialize `serverInfo`) is shown when known.
 *  The `failed` state's click action is "install" when no binary was found at
 *  all, "show logs" when a binary crashed. */
function setStatus(
  state: "starting" | "running" | "failed",
  version?: string,
  failedAction: "install" | "logs" = "logs",
): void {
  if (!statusBar) return;
  const errorBg = new vscode.ThemeColor("statusBarItem.errorBackground");
  const label = version ? `pdxl ${game} v${version}` : `pdxl ${game}`;
  statusBar.command = "pdxl.showServerLog";
  switch (state) {
    case "starting":
      statusBar.text = "$(loading~spin) pdxl";
      statusBar.tooltip = "pdxl language server starting… — click for logs";
      statusBar.backgroundColor = undefined;
      break;
    case "running":
      statusBar.text = `$(check) ${label}`;
      statusBar.tooltip = `${label} language server running — click for server logs`;
      statusBar.backgroundColor = undefined;
      break;
    case "failed":
      statusBar.text = "$(error) pdxl";
      statusBar.backgroundColor = errorBg;
      if (failedAction === "install") {
        statusBar.tooltip =
          "pdxl language server not found — click to install it";
        statusBar.command = "pdxl.installServer";
      } else {
        statusBar.tooltip =
          "pdxl language server failed to start — click for logs";
      }
      break;
  }
}

/** The server version this extension is pinned to (package.json version —
 *  kept in lockstep with the workspace/release version). */
function pinnedVersion(context: vscode.ExtensionContext): string {
  return (context.extension.packageJSON as { version: string }).version;
}

/** Builds and starts the language client against `command`. `source` is how
 *  the command was resolved — update suggestions are skipped for explicit
 *  `setting` paths (the user manages those). */
async function startClient(
  context: vscode.ExtensionContext,
  command: string,
  source: "setting" | "managed" | "path" = "managed",
): Promise<void> {
  const config = vscode.workspace.getConfiguration("pdxl");
  // Per-game vanilla dir: explicit setting → legacy pdxl.gamePath (CK3 only,
  // it predates multi-game) → Steam auto-discovery.
  const resolved = resolveGamePath(
    game,
    config.get<string>(game === "ck3" ? "gamePathCk3" : "gamePathEu5", ""),
    config.get<string>("gamePath", ""),
  );
  const gamePath = resolved?.path ?? "";
  const logLevel = config.get<string>("logLevel", "info");

  log(`server command: ${command}`);
  log(
    `gamePath (${game}): ${gamePath || "(not found)"}` +
      (resolved ? ` [${resolved.source}]` : ""),
  );
  if (!gamePath) {
    void vscode.window.showWarningMessage(
      `pdxl: no ${game} game directory configured or found — only mod files ` +
        `will be analyzed. Set pdxl.gamePath${game === "ck3" ? "Ck3" : "Eu5"}.`,
    );
  }
  log(`logLevel: ${logLevel}`);

  const serverArgs = ["lsp", "--log-level", logLevel];
  const serverOptions: ServerOptions = {
    run: { command, args: serverArgs, transport: TransportKind.stdio },
    debug: { command, args: serverArgs, transport: TransportKind.stdio },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", pattern: "**/*.txt" },
      // Localization files: find-references / hover on loc keys from the yml
      // side. Narrow pattern — random workspace .yml (CI configs) stays out.
      { scheme: "file", pattern: "**/localization/**/*.yml" },
      // Interface scripts: template/type navigation + datafunction typing.
      { scheme: "file", pattern: "**/*.gui" },
    ],
    initializationOptions: { gamePath },
    middleware: {
      provideDocumentSemanticTokens: async (document, token, next) => {
        const result = await next(document, token);
        if (result) decorateUnresolvedSemanticTokens(document, result);
        return result;
      },
    },
    // Surface server stderr (slog) in the "pdxl (server)" output channel.
    outputChannelName: "pdxl (server)",
  };

  client = new LanguageClient("pdxl", "pdxl PDXScript", serverOptions, clientOptions);

  setStatus("starting");
  log("starting language client...");
  try {
    await client.start();
    // The binary reports its version in the initialize handshake's
    // serverInfo (baked in from Cargo since v0.58.1); older binaries don't,
    // so fall back to the version encoded in the managed install path.
    const version =
      client.initializeResult?.serverInfo?.version ??
      /[/\\]v(\d+\.\d+\.\d+)[/\\]/.exec(command)?.[1];
    log(
      `language client started successfully (server ${version ?? "unknown version"})`,
    );
    const pinned = pinnedVersion(context);
    if (version && version !== pinned) {
      log(
        `note: server version ${version} differs from the extension's pinned ${pinned}`,
      );
    }
    setStatus("running", version);
    // Old binaries without a reported version are exactly the ones that
    // need updating — check regardless of whether the version is known.
    if (source !== "setting") {
      void checkForUpdate(context, version);
    }
  } catch (err) {
    log(`failed to start language client: ${err}`);
    setStatus("failed");
    void promptInstall(
      context,
      `pdxl: the language server failed to start (${command}).`,
    );
  }
}

/** Once per session: compares the running server version against the newest
 *  GitHub release and offers to update. Fire-and-forget; network failures are
 *  logged and ignored. "Skip this version" is remembered per version. */
let updateCheckDone = false;
async function checkForUpdate(
  context: vscode.ExtensionContext,
  runningVersion: string | undefined,
): Promise<void> {
  if (updateCheckDone) return;
  updateCheckDone = true;
  let latest: string | undefined;
  try {
    latest = await latestReleaseVersion();
  } catch (err) {
    log(`update check failed: ${err}`);
    return;
  }
  // An unknown running version (a pre-0.58.1 binary from PATH) is treated
  // as outdated: those are exactly the binaries that need updating.
  if (!latest || (runningVersion && !isNewerVersion(latest, runningVersion))) {
    log(
      `update check: running v${runningVersion ?? "?"}, latest v${latest ?? "?"} — up to date`,
    );
    return;
  }
  const skipKey = `pdxl.skipUpdate.${latest}`;
  if (context.globalState.get<boolean>(skipKey)) {
    log(`update check: v${latest} available but skipped by user`);
    return;
  }
  log(`update check: v${latest} available (running v${runningVersion ?? "?"})`);
  const running = runningVersion ? `v${runningVersion}` : "an unknown version";
  const update = `Update to v${latest}`;
  const skip = "Skip this version";
  const choice = await vscode.window.showInformationMessage(
    `pdxl v${latest} is available (running ${running}).`,
    update,
    skip,
  );
  if (choice === update) {
    const installed = await runInstallWithProgress(context, latest);
    if (installed) await restartClient(context, installed);
  } else if (choice === skip) {
    await context.globalState.update(skipKey, true);
  }
}

/** Restarts (or first-starts) the client against a freshly resolved command. */
async function restartClient(
  context: vscode.ExtensionContext,
  command: string,
  source: "setting" | "managed" | "path" = "managed",
): Promise<void> {
  if (client) {
    log("stopping language client for restart...");
    await client.stop().catch(() => undefined);
    client = undefined;
  }
  await startClient(context, command, source);
}

/** Opens a file dialog and persists the chosen binary as pdxl.serverPath. */
async function pickServerPath(): Promise<string | undefined> {
  const picked = await vscode.window.showOpenDialog({
    canSelectFiles: true,
    canSelectFolders: false,
    canSelectMany: false,
    openLabel: "Use as pdxl server",
    title: "Pick the pdxl language-server binary",
  });
  const fsPath = picked?.[0]?.fsPath;
  if (!fsPath) return undefined;
  await vscode.workspace
    .getConfiguration("pdxl")
    .update("serverPath", fsPath, vscode.ConfigurationTarget.Global);
  log(`serverPath set to ${fsPath}`);
  return fsPath;
}

/** Downloads and installs the pinned release under a progress notification.
 *  Returns the installed path, or undefined on failure (already reported). */
async function runInstallWithProgress(
  context: vscode.ExtensionContext,
  versionOverride?: string,
): Promise<string | undefined> {
  const version = versionOverride ?? pinnedVersion(context);
  try {
    return await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: `Installing pdxl v${version}`,
        cancellable: false,
      },
      async (progress) => {
        let lastPercent = 0;
        return installServer(context, game, version, (message, percent) => {
          const increment =
            percent !== undefined ? percent - lastPercent : undefined;
          if (percent !== undefined) lastPercent = percent;
          progress.report({ message, increment });
        });
      },
    );
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    log(`install failed: ${msg}`);
    const pick = await vscode.window.showErrorMessage(
      `pdxl: install failed — ${msg}`,
      "Pick path…",
    );
    if (pick === "Pick path…") {
      const path = await pickServerPath();
      if (path) await restartClient(context, path, "setting");
    }
    return undefined;
  }
}

/** One-time-per-session offer to download or locate the server binary. */
async function promptInstall(
  context: vscode.ExtensionContext,
  reason: string,
): Promise<void> {
  if (installPromptShown) return;
  installPromptShown = true;
  const version = pinnedVersion(context);
  const download = `Download pdxl (${game}) v${version} from GitHub`;
  const pick = "Pick path…";
  const choice = await vscode.window.showInformationMessage(
    reason,
    download,
    pick,
  );
  if (choice === download) {
    const installed = await runInstallWithProgress(context);
    if (installed) await restartClient(context, installed);
  } else if (choice === pick) {
    const path = await pickServerPath();
    if (path) await restartClient(context, path, "setting");
  }
}

export function activate(context: vscode.ExtensionContext): void {
  output = vscode.window.createOutputChannel("pdxl (client)", { log: true });
  context.subscriptions.push(output, unresolvedDocDecoration);

  log("pdxl extension activating...");

  // Status-bar button: shows server health at a glance and reveals the
  // "pdxl (server)" log channel on click (or offers install when missing).
  statusBar = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    100,
  );
  setStatus("starting");
  statusBar.show();
  context.subscriptions.push(statusBar);

  context.subscriptions.push(
    vscode.commands.registerCommand("pdxl.showServerLog", () => {
      // The LanguageClient's output channel carries the server's stderr (slog).
      client?.outputChannel.show(true);
    }),
    // Force (re-)download of the pinned release, then restart the client.
    vscode.commands.registerCommand("pdxl.installServer", async () => {
      const installed = await runInstallWithProgress(context);
      if (installed) await restartClient(context, installed);
    }),
    vscode.commands.registerCommand("pdxl.pickServerPath", async () => {
      const path = await pickServerPath();
      if (path) await restartClient(context, path, "setting");
    }),
  );

  // Bridge for the reference-count CodeLens. The server emits a
  // `pdxl.showReferences` command carrying protocol JSON; VS Code's built-in
  // `editor.action.showReferences` validates its arguments with `instanceof`
  // and rejects raw JSON, so convert here into native objects first.
  context.subscriptions.push(
    vscode.commands.registerCommand(
      "pdxl.showReferences",
      (
        uri: string,
        position: { line: number; character: number },
        locations: { uri: string; range: vscode.Range }[],
      ) => {
        const target = vscode.Uri.parse(uri);
        const pos = new vscode.Position(position.line, position.character);
        const locs = locations.map(
          (l) =>
            new vscode.Location(
              vscode.Uri.parse(l.uri),
              new vscode.Range(
                l.range.start.line,
                l.range.start.character,
                l.range.end.line,
                l.range.end.character,
              ),
            ),
        );
        return vscode.commands.executeCommand(
          "editor.action.showReferences",
          target,
          pos,
          locs,
        );
      },
    ),
  );

  // Which game is this workspace? Explicit pdxl.game setting wins;
  // "auto" inspects the workspace layout (EU5-era .metadata/metadata.json
  // or in_game/main_menu module roots → eu5; anything else → ck3).
  const gameSetting = vscode.workspace
    .getConfiguration("pdxl")
    .get<string>("game", "auto");
  game =
    gameSetting === "ck3" || gameSetting === "eu5"
      ? gameSetting
      : detectGame(vscode.workspace.workspaceFolders?.[0]?.uri.fsPath);
  log(`game: ${game} (setting: ${gameSetting})`);

  // Resolve the server: explicit setting → managed binary → PATH. When
  // nothing is runnable, offer to download the pinned release.
  const serverPath = vscode.workspace
    .getConfiguration("pdxl")
    .get<string>("serverPath", "");
  const resolved = resolveServer(context, game, serverPath, pinnedVersion(context));
  if (resolved) {
    log(`resolved server from ${resolved.source}: ${resolved.command}`);
    void startClient(context, resolved.command, resolved.source);
  } else {
    setStatus("failed", undefined, "install");
    void promptInstall(
      context,
      "pdxl: language server binary not found (not configured, not installed, not on PATH).",
    );
  }

  context.subscriptions.push({
    dispose: () => {
      log("disposing language client...");
      void client?.stop();
    },
  });

  log("pdxl extension activated");
}

export function deactivate(): Thenable<void> | undefined {
  log("pdxl extension deactivating...");
  return client?.stop();
}
