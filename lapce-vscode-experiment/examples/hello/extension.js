const vscode = require('vscode');
exports.activate = context => {
    context.subscriptions.push(vscode.commands.registerCommand('sandbox.hello', () => {
        const greeting = vscode.workspace.getConfiguration('sandbox').get('greeting', 'Hello');
        vscode.window.showInformationMessage(greeting);
        return greeting;
    }));
};
exports.deactivate = () => {};
