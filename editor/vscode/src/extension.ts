import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;
let output: vscode.OutputChannel | undefined;
let statusBar: vscode.StatusBarItem | undefined;

function log(msg: string): void {
  console.log(`[pdxl] ${msg}`);
  output?.appendLine(`[${new Date().toISOString()}] ${msg}`);
}

/** Updates the status-bar button's icon, tooltip, and error colouring. */
function setStatus(state: "starting" | "running" | "failed"): void {
  if (!statusBar) return;
  const errorBg = new vscode.ThemeColor("statusBarItem.errorBackground");
  switch (state) {
    case "starting":
      statusBar.text = "$(loading~spin) pdxl";
      statusBar.tooltip = "pdxl language server starting… — click for logs";
      statusBar.backgroundColor = undefined;
      break;
    case "running":
      statusBar.text = "$(check) pdxl";
      statusBar.tooltip = "pdxl language server running — click for server logs";
      statusBar.backgroundColor = undefined;
      break;
    case "failed":
      statusBar.text = "$(error) pdxl";
      statusBar.tooltip = "pdxl language server failed to start — click for logs";
      statusBar.backgroundColor = errorBg;
      break;
  }
}

export function activate(context: vscode.ExtensionContext): void {
  output = vscode.window.createOutputChannel("pdxl (client)", { log: true });
  context.subscriptions.push(output);

  log("pdxl extension activating...");

  // Status-bar button: shows server health at a glance and reveals the
  // "pdxl (server)" log channel on click.
  statusBar = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    100,
  );
  statusBar.command = "pdxl.showServerLog";
  setStatus("starting");
  statusBar.show();
  context.subscriptions.push(statusBar);

  context.subscriptions.push(
    vscode.commands.registerCommand("pdxl.showServerLog", () => {
      // The LanguageClient's output channel carries the server's stderr (slog).
      client?.outputChannel.show(true);
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

  const config = vscode.workspace.getConfiguration("pdxl");
  const serverPath = config.get<string>("serverPath", "pdxl");
  const gamePath = config.get<string>("gamePath", "");
  const logLevel = config.get<string>("logLevel", "info");

  log(`serverPath: ${serverPath}`);
  log(`gamePath: ${gamePath || "(not set)"}`);
  log(`logLevel: ${logLevel}`);

  const serverArgs = ["lsp", "--log-level", logLevel];

  const serverOptions: ServerOptions = {
    run: { command: serverPath, args: serverArgs, transport: TransportKind.stdio },
    debug: {
      command: serverPath,
      args: serverArgs,
      transport: TransportKind.stdio,
    },
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

  client = new LanguageClient(
    "pdxl",
    "pdxl PDXScript",
    serverOptions,
    clientOptions,
  );

  log("starting language client...");

  client.start().then(
    () => {
      log("language client started successfully");
      setStatus("running");
    },
    (err) => {
      log(`failed to start language client: ${err}`);
      setStatus("failed");
      vscode.window.showErrorMessage(
        `pdxl: failed to start the language server (${serverPath}). ` +
          `Is the 'pdxl' binary installed and on PATH? ${err}`,
      );
    },
  );

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
