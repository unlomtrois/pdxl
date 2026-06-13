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

// PDXScript lives in generic .txt files, so we attach by directory rather than
// claiming the .txt language globally.
const documentSelector = [
  "common",
  "events",
  "history",
  "gfx",
  "gui",
  "localization",
].map((dir) => ({
  scheme: "file",
  pattern: `**/${dir}/**/*.txt`,
}));

export function activate(context: vscode.ExtensionContext): void {
  output = vscode.window.createOutputChannel("pdxl", { log: true });
  context.subscriptions.push(output);

  log("pdxl extension activating...");

  const config = vscode.workspace.getConfiguration("pdxl");
  const serverPath = config.get<string>("serverPath", "pdxl");
  const gamePath = config.get<string>("gamePath", "");

  log(`serverPath: ${serverPath}`);
  log(`gamePath: ${gamePath || "(not set)"}`);
  log(`documentSelector patterns: ${documentSelector.map((s) => s.pattern).join(", ")}`);

  const serverOptions: ServerOptions = {
    run: { command: serverPath, args: ["lsp"], transport: TransportKind.stdio },
    debug: { command: serverPath, args: ["lsp"], transport: TransportKind.stdio },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector,
    initializationOptions: { gamePath },
    // Surface server stderr (slog) in the "pdxl" output channel.
    outputChannelName: "pdxl",
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
