const path = require("path");
const fs = require("fs");

const vscode = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

let client;

async function activate(context) {
    const config = vscode.workspace.getConfiguration("phdl.server");
    let serverPath = config.get("path", "piperine-lang-server");

    // Resolve relative paths against known locations.
    if (!path.isAbsolute(serverPath)) {
        const candidates = [
            // Extension parent dir (../../ -> workspace root when extension is in .vscode/extension/)
            path.resolve(context.extensionPath, "..", "..", serverPath),
            // Workspace root (may be test-fixtures or the actual workspace)
            ...(vscode.workspace.workspaceFolders || []).map(
                (f) => path.join(f.uri.fsPath, serverPath),
            ),
        ];

        for (const candidate of candidates) {
            if (fs.existsSync(candidate)) {
                serverPath = candidate;
                break;
            }
        }
    }

    console.log(`piperine-lang-server path: ${serverPath}`);

    const serverOptions = {
        command: serverPath,
        transport: TransportKind.stdio,
    };

    const clientOptions = {
        documentSelector: [{ scheme: "file", language: "phdl" }],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher("**/*.phdl"),
        },
    };

    client = new LanguageClient(
        "piperineLangServer",
        "Piperine Language Server",
        serverOptions,
        clientOptions,
    );

    await client.start();
    console.log("piperine-lang-server ready");
}

function deactivate() {
    if (client) {
        return client.stop();
    }
    return undefined;
}

module.exports = { activate, deactivate };
