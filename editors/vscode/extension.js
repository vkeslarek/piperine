const path = require("path");
const fs = require("fs");

const vscode = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

let client;
let outputChannel;

// ---------------------------------------------------------------------------
// Binary resolution
// ---------------------------------------------------------------------------

/// Picks whichever candidate binary was actually built most recently.
///
/// **Why mtime and not a fixed candidate order** (both failure modes were
/// hit for real while testing this extension against a live editor):
///  1. `.vscode/tasks.json`'s `build-lang-server` preLaunchTask only ever
///     builds `target/debug`, but the old code checked `target/release`
///     first — one stale release binary from an earlier `build-vsix.sh`
///     run silently shadowed every fresh debug rebuild, forever, with no
///     error shown anywhere.
///  2. The packaged `bin/` binary (shipped inside the `.vsix`) shadowed
///     dev builds the same way. It races on mtime here too, so a fresh
///     `cargo build` always wins in a dev checkout while an installed
///     `.vsix` (which has no workspace target dirs at all) still resolves
///     to its own embedded binary.
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

/// Walks up from `start` looking for the Cargo workspace root (the first
/// ancestor holding a `Cargo.toml`), returning every ancestor that has
/// one — the outermost is the workspace root whose `target/` cargo
/// actually writes to.
///
/// **Why walking up is required**: `.vscode/launch.json` opens
/// `${workspaceFolder}/examples` in the Extension Development Host, so
/// `workspaceFolders[0]` is `<repo>/examples` — NOT the repo root. Only
/// looking at `<workspace>/target/...` therefore found nothing in the
/// project's own dev workflow and fell through to the stale packaged
/// `bin/` binary on every single F5. That is the whole reason
/// source-level LSP fixes appeared not to work in the editor while
/// passing their own tests against the real binary.
function cargoRootsAbove(start) {
    const roots = [];
    let dir = start;
    for (;;) {
        if (fs.existsSync(path.join(dir, "Cargo.toml"))) {
            roots.push(dir);
        }
        const parent = path.dirname(dir);
        if (parent === dir) {
            return roots;
        }
        dir = parent;
    }
}

function resolveBinaryPath(context, baseName, configPath) {
    if (path.isAbsolute(configPath)) {
        return configPath;
    }

    // Every Cargo root above the open workspace folder(s) AND above the
    // extension itself (a dev checkout runs the extension straight out of
    // `<repo>/editors/vscode`, so the repo root sits two levels up even
    // when the opened folder is somewhere else entirely).
    const searchBases = [
        ...(vscode.workspace.workspaceFolders || []).map((f) => f.uri.fsPath),
        context.extensionPath,
    ];
    const cargoRoots = [...new Set(searchBases.flatMap(cargoRootsAbove))];

    const targetDirs = cargoRoots.map((root) => path.join(root, "target"));
    // `CARGO_TARGET_DIR` redirects every build out of `<root>/target`; a
    // checkout that sets it (this repo's own does) would otherwise have
    // no discoverable build output at all.
    if (process.env.CARGO_TARGET_DIR) {
        targetDirs.push(process.env.CARGO_TARGET_DIR);
    }

    const candidates = targetDirs.flatMap((t) => [
        path.join(t, "release", baseName),
        path.join(t, "debug", baseName),
    ]);
    // The packaged binary races on mtime alongside them (see `newestExisting`).
    candidates.push(path.join(context.extensionPath, "bin", baseName));

    const best = newestExisting(candidates);
    if (best) {
        return best;
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
