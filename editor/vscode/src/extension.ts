import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";
import { installServer, resolveServer } from "./install";

let client: LanguageClient | undefined;
let output: vscode.OutputChannel | undefined;
let statusBar: vscode.StatusBarItem | undefined;
let installPromptShown = false;

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
  const label = version ? `pdxl v${version}` : "pdxl";
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

/** Builds and starts the language client against `command`. */
async function startClient(
  context: vscode.ExtensionContext,
  command: string,
): Promise<void> {
  const config = vscode.workspace.getConfiguration("pdxl");
  const gamePath = config.get<string>("gamePath", "");
  const logLevel = config.get<string>("logLevel", "info");

  log(`server command: ${command}`);
  log(`gamePath: ${gamePath || "(not set)"}`);
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
    // Surface server stderr (slog) in the "pdxl (server)" output channel.
    outputChannelName: "pdxl (server)",
  };

  client = new LanguageClient("pdxl", "pdxl PDXScript", serverOptions, clientOptions);

  setStatus("starting");
  log("starting language client...");
  try {
    await client.start();
    // The binary reports its version in the initialize handshake's
    // serverInfo (baked in from Cargo at compile time).
    const version = client.initializeResult?.serverInfo?.version;
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
  } catch (err) {
    log(`failed to start language client: ${err}`);
    setStatus("failed");
    void promptInstall(
      context,
      `pdxl: the language server failed to start (${command}).`,
    );
  }
}

/** Restarts (or first-starts) the client against a freshly resolved command. */
async function restartClient(
  context: vscode.ExtensionContext,
  command: string,
): Promise<void> {
  if (client) {
    log("stopping language client for restart...");
    await client.stop().catch(() => undefined);
    client = undefined;
  }
  await startClient(context, command);
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
): Promise<string | undefined> {
  const version = pinnedVersion(context);
  try {
    return await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: `Installing pdxl v${version}`,
        cancellable: false,
      },
      async (progress) => {
        let lastPercent = 0;
        return installServer(context, version, (message, percent) => {
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
      if (path) await restartClient(context, path);
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
  const download = `Download pdxl v${version} from GitHub`;
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
    if (path) await restartClient(context, path);
  }
}

export function activate(context: vscode.ExtensionContext): void {
  output = vscode.window.createOutputChannel("pdxl (client)", { log: true });
  context.subscriptions.push(output);

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
      if (path) await restartClient(context, path);
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

  // Resolve the server: explicit setting → managed binary → PATH. When
  // nothing is runnable, offer to download the pinned release.
  const serverPath = vscode.workspace
    .getConfiguration("pdxl")
    .get<string>("serverPath", "");
  const resolved = resolveServer(context, serverPath, pinnedVersion(context));
  if (resolved) {
    log(`resolved server from ${resolved.source}: ${resolved.command}`);
    void startClient(context, resolved.command);
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
