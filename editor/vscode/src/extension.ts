import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;
let output: vscode.OutputChannel | undefined;

function log(msg: string): void {
  console.log(`[pdxl] ${msg}`);
  output?.appendLine(`[${new Date().toISOString()}] ${msg}`);
}

export function activate(context: vscode.ExtensionContext): void {
  output = vscode.window.createOutputChannel("pdxl (client)", { log: true });
  context.subscriptions.push(output);

  log("pdxl extension activating...");

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
    () => log("language client started successfully"),
    (err) => {
      log(`failed to start language client: ${err}`);
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
