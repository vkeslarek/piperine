const path = require("path");
const fs = require("fs");

const vscode = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

let client;
let outputChannel;

// ---------------------------------------------------------------------------
// Language Server client
// ---------------------------------------------------------------------------

async function startClient(context) {
    const config = vscode.workspace.getConfiguration("piperine.server");
    let serverBaseName = config.get("path", "piperine-lang-server");

    let serverPath = serverBaseName;

    if (!path.isAbsolute(serverPath)) {
        const candidates = [
            // Workspace target directories
            ...(vscode.workspace.workspaceFolders || []).map(
                (f) => path.join(f.uri.fsPath, "target", "release", serverBaseName),
            ),
            ...(vscode.workspace.workspaceFolders || []).map(
                (f) => path.join(f.uri.fsPath, "target", "debug", serverBaseName),
            ),
            // Embedded binary inside the extension itself (build-vsix.sh copies
            // plain, unsuffixed binaries into bin/ — each .vsix is already
            // platform-specific, so no OS-suffix candidate is needed here)
            path.join(context.extensionPath, "bin", serverBaseName),
            // Cargo global install
            path.join(process.env.HOME || process.env.USERPROFILE || "", ".cargo", "bin", serverBaseName),
        ];

        for (const candidate of candidates) {
            if (fs.existsSync(candidate)) {
                serverPath = candidate;
                break;
            }
        }
    }

    outputChannel.appendLine(`piperine-lang-server path: ${serverPath}`);

    const serverOptions = {
        command: serverPath,
        transport: TransportKind.stdio,
    };

    const clientOptions = {
        documentSelector: [
            { scheme: "file", language: "vams" },
            { scheme: "untitled", language: "vams" },
        ],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher("**/*.{vams,va}"),
        },
        outputChannel,
        traceOutputChannel: outputChannel,
    };

    client = new LanguageClient(
        "piperineLangServer",
        "Piperine Language Server",
        serverOptions,
        clientOptions,
    );

    await client.start();
    outputChannel.appendLine("piperine-lang-server ready");
}

// ---------------------------------------------------------------------------
// CLI path resolution  (for `piperine test`)
// ---------------------------------------------------------------------------

function getPiperineCliPath(context) {
    const config = vscode.workspace.getConfiguration("piperine.cli");
    let cliBaseName = config.get("path", "piperine");

    let cliPath = cliBaseName;

    if (!path.isAbsolute(cliPath)) {
        const candidates = [
            // Workspace target directories
            ...(vscode.workspace.workspaceFolders || []).map(
                (f) => path.join(f.uri.fsPath, "target", "release", cliBaseName),
            ),
            ...(vscode.workspace.workspaceFolders || []).map(
                (f) => path.join(f.uri.fsPath, "target", "debug", cliBaseName),
            ),
            // Embedded binary inside the extension itself (build-vsix.sh copies
            // plain, unsuffixed binaries into bin/ — each .vsix is already
            // platform-specific, so no OS-suffix candidate is needed here)
            path.join(context.extensionPath, "bin", cliBaseName),
            // Cargo global install
            path.join(process.env.HOME || process.env.USERPROFILE || "", ".cargo", "bin", cliBaseName),
        ];

        for (const candidate of candidates) {
            if (fs.existsSync(candidate)) {
                return candidate;
            }
        }
    }
    return cliPath;
}

// ---------------------------------------------------------------------------
// Extension lifecycle
// ---------------------------------------------------------------------------

async function activate(context) {
    outputChannel = vscode.window.createOutputChannel("Piperine Language Server");
    context.subscriptions.push(outputChannel);

    // Commands
    context.subscriptions.push(
        vscode.commands.registerCommand("piperine.restartServer", async () => {
            if (client) {
                await client.stop();
            }
            outputChannel.appendLine("Restarting piperine-lang-server...");
            await startClient(context);
        }),
        vscode.commands.registerCommand("piperine.test", async (fileUri) => {
            const cliPath = getPiperineCliPath(context);
            const terminal =
                vscode.window.activeTerminal || vscode.window.createTerminal("Piperine Bench");
            terminal.show();
            if (fileUri) {
                terminal.sendText(`"${cliPath}" test "${fileUri}"`);
            } else {
                terminal.sendText(`"${cliPath}" test`);
            }
        }),
    );

    await startClient(context);
}

function deactivate() {
    if (client) {
        return client.stop();
    }
    return undefined;
}

module.exports = { activate, deactivate };
