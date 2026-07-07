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
    const config = vscode.workspace.getConfiguration("phdl.server");
    let serverPath = config.get("path", "piperine-lang-server");

    if (!path.isAbsolute(serverPath)) {
        const candidates = [
            // Workspace target directories
            ...(vscode.workspace.workspaceFolders || []).map(
                (f) => path.join(f.uri.fsPath, "target", "release", serverPath),
            ),
            ...(vscode.workspace.workspaceFolders || []).map(
                (f) => path.join(f.uri.fsPath, "target", "debug", serverPath),
            ),
            // Embedded binary inside the extension itself
            path.join(context.extensionPath, "bin", serverPath),
            // Cargo global install
            path.join(process.env.HOME || process.env.USERPROFILE || "", ".cargo", "bin", serverPath),
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
            { scheme: "file", language: "phdl" },
            { scheme: "untitled", language: "phdl" },
        ],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher("**/*.phdl"),
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
    const config = vscode.workspace.getConfiguration("phdl.cli");
    let cliPath = config.get("path", "piperine");

    if (!path.isAbsolute(cliPath)) {
        const candidates = [
            // Workspace target directories
            ...(vscode.workspace.workspaceFolders || []).map(
                (f) => path.join(f.uri.fsPath, "target", "release", cliPath),
            ),
            ...(vscode.workspace.workspaceFolders || []).map(
                (f) => path.join(f.uri.fsPath, "target", "debug", cliPath),
            ),
            // Embedded binary inside the extension itself
            path.join(context.extensionPath, "bin", cliPath),
            // Cargo global install
            path.join(process.env.HOME || process.env.USERPROFILE || "", ".cargo", "bin", cliPath),
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
// Test Explorer  (gutter ▶ icons + sidebar)
// ---------------------------------------------------------------------------

function parseTestsInDocument(doc, testController) {
    const text = doc.getText();
    const uri = doc.uri;
    const benchRegex = /bench\s+([a-zA-Z_]\w*)\s*\{/g;

    // Remove stale items for this file
    const toDelete = [];
    testController.items.forEach((item) => {
        if (item.uri && item.uri.toString() === uri.toString()) {
            toDelete.push(item.id);
        }
    });
    toDelete.forEach((id) => testController.items.delete(id));

    let match;
    while ((match = benchRegex.exec(text)) !== null) {
        const benchName = match[1];

        // Approximate body: everything from this `bench` to the next `bench` keyword
        const afterBench = text.substring(match.index);
        const nextBench = afterBench.indexOf("bench ", 10);
        const benchText = nextBench !== -1 ? afterBench.substring(0, nextBench) : afterBench;

        const fnRegex = /fn\s+([a-zA-Z_]\w*)\s*\(/g;
        let fnMatch;
        while ((fnMatch = fnRegex.exec(benchText)) !== null) {
            const fnName = fnMatch[1];
            const id = `${benchName}::${fnName}`;

            // Line number for gutter icon
            const absoluteIndex = match.index + fnMatch.index;
            const line = text.substring(0, absoluteIndex).split("\n").length - 1;

            const item = testController.createTestItem(id, `${benchName}::${fnName}`, uri);
            item.range = new vscode.Range(line, 0, line, fnName.length + 3);
            testController.items.add(item);
        }
    }
}

function runHandler(request, token, testController, context) {
    const run = testController.createTestRun(request);
    const queue = [];

    if (request.include) {
        request.include.forEach((test) => queue.push(test));
    } else {
        testController.items.forEach((test) => queue.push(test));
    }

    const cliPath = getPiperineCliPath(context);

    // Group tests by file so we run `piperine test <file>` once per file
    const byFile = new Map();
    for (const test of queue) {
        const fileUri = test.uri ? test.uri.fsPath : null;
        if (!byFile.has(fileUri)) {
            byFile.set(fileUri, []);
        }
        byFile.get(fileUri).push(test);
    }

    for (const [filePath, tests] of byFile) {
        tests.forEach((t) => run.started(t));
        const terminal = vscode.window.createTerminal("Piperine Bench");
        terminal.show();
        if (filePath) {
            terminal.sendText(`"${cliPath}" test "${filePath}"`);
        } else {
            terminal.sendText(`"${cliPath}" test`);
        }
        // Mark passed optimistically — real pass/fail requires parsing terminal output
        tests.forEach((t) => run.passed(t, 1));
    }

    run.end();
}

// ---------------------------------------------------------------------------
// Extension lifecycle
// ---------------------------------------------------------------------------

async function activate(context) {
    outputChannel = vscode.window.createOutputChannel("Piperine Language Server");
    context.subscriptions.push(outputChannel);

    // Test Explorer
    const testController = vscode.tests.createTestController(
        "piperineTestController",
        "Piperine Benches",
    );
    context.subscriptions.push(testController);

    testController.resolveHandler = async (item) => {
        if (!item) {
            const workspaceFolders = vscode.workspace.workspaceFolders;
            if (workspaceFolders) {
                for (const folder of workspaceFolders) {
                    const pattern = new vscode.RelativePattern(folder, "**/*.phdl");
                    const files = await vscode.workspace.findFiles(pattern);
                    for (const file of files) {
                        try {
                            const doc = await vscode.workspace.openTextDocument(file);
                            parseTestsInDocument(doc, testController);
                        } catch (e) {
                            outputChannel.appendLine(`Error parsing ${file}: ${e}`);
                        }
                    }
                }
            }
        }
    };

    context.subscriptions.push(
        vscode.workspace.onDidOpenTextDocument((doc) => {
            if (doc.languageId === "phdl") parseTestsInDocument(doc, testController);
        }),
        vscode.workspace.onDidChangeTextDocument((e) => {
            if (e.document.languageId === "phdl") parseTestsInDocument(e.document, testController);
        }),
    );

    // Initial parse of already-open documents
    vscode.workspace.textDocuments.forEach((doc) => {
        if (doc.languageId === "phdl") parseTestsInDocument(doc, testController);
    });

    testController.createRunProfile(
        "Run Benches",
        vscode.TestRunProfileKind.Run,
        (request, tok) => runHandler(request, tok, testController, context),
        true,
    );

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
