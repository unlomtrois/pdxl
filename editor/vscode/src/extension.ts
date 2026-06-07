import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

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
  const config = vscode.workspace.getConfiguration("pdxl");
  const serverPath = config.get<string>("serverPath", "pdxl");
  const gamePath = config.get<string>("gamePath", "");

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

  client.start().catch((err) => {
    vscode.window.showErrorMessage(
      `pdxl: failed to start the language server (${serverPath}). ` +
        `Is the 'pdxl' binary installed and on PATH? ${err}`,
    );
  });

  context.subscriptions.push({
    dispose: () => {
      void client?.stop();
    },
  });
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
