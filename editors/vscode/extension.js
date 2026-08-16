const path = require("path");
const fs = require("fs");

const vscode = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

let client;
let outputChannel;

// ---------------------------------------------------------------------------
// Binary resolution
// ---------------------------------------------------------------------------

/// Of the two workspace target dirs (`target/release`, `target/debug`),
/// picks whichever binary was actually built most recently, not a fixed
/// "release always wins" order. The repo's own dev workflow
/// (`.vscode/tasks.json`'s `build-lang-server` preLaunchTask) only ever
/// builds `debug` — a stale `release` binary left over from an earlier
/// `build-vsix.sh` packaging run would otherwise silently shadow every
/// fresh debug rebuild forever, with no error (found testing: a real fix
/// committed to source never reached the running server because of this
/// exact static ordering).
function newestExisting(paths) {
    let best = null;
    let bestMtime = -Infinity;
    for (const p of paths) {
        try {
            const mtime = fs.statSync(p).mtimeMs;
            if (mtime > bestMtime) {
                best = p;
                bestMtime = mtime;
            }
        } catch {
            // Doesn't exist — not a candidate.
        }
    }
    return best;
}

function resolveBinaryPath(context, baseName, configPath) {
    if (path.isAbsolute(configPath)) {
        return configPath;
    }
    const workspaceTargets = (vscode.workspace.workspaceFolders || []).flatMap((f) => [
        path.join(f.uri.fsPath, "target", "release", baseName),
        path.join(f.uri.fsPath, "target", "debug", baseName),
    ]);
    const fromWorkspace = newestExisting(workspaceTargets);
    if (fromWorkspace) {
        return fromWorkspace;
    }
    // Embedded binary inside the extension itself (build-vsix.sh copies
    // plain, unsuffixed binaries into bin/ — each .vsix is already
    // platform-specific, so no OS-suffix candidate is needed here).
    const embedded = path.join(context.extensionPath, "bin", baseName);
    if (fs.existsSync(embedded)) {
        return embedded;
    }
    // Cargo global install.
    const cargoBin = path.join(process.env.HOME || process.env.USERPROFILE || "", ".cargo", "bin", baseName);
    if (fs.existsSync(cargoBin)) {
        return cargoBin;
    }
    return configPath;
}

// ---------------------------------------------------------------------------
// Language Server client
// ---------------------------------------------------------------------------

async function startClient(context) {
    const config = vscode.workspace.getConfiguration("piperine.server");
    const serverBaseName = config.get("path", "piperine-lang-server");
    const serverPath = resolveBinaryPath(context, serverBaseName, serverBaseName);

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
    const cliBaseName = config.get("path", "piperine");
    return resolveBinaryPath(context, cliBaseName, cliBaseName);
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
