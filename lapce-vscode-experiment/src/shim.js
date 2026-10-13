// Capability-free JS shim. Rust only consumes a bounded JSON message result.
(() => {
    'use strict';
    const manifest = __manifest;
    delete globalThis.__manifest;
    const allowed = new Set(manifest.contributes.commands.map(c => c.command));
    const handlers = new Map();
    const messages = [];
    const defaults = Object.create(null);
    const config = manifest.contributes.configuration;
    const sections = Array.isArray(config) ? config : [config || {}];
    for (const section of sections) {
        for (const [key, value] of Object.entries(section.properties || {})) {
            if (Object.prototype.hasOwnProperty.call(value, 'default')) defaults[key] = value.default;
        }
    }
    const invoke = (id, args) => {
        if (!handlers.has(id)) throw new Error('Command is not registered: ' + id);
        return handlers.get(id)(...args);
    };
    const message = (kind, text) => {
        if (typeof text !== 'string' || text.length > 2048 || messages.length >= 64)
            throw new Error('Message limit exceeded');
        messages.push({kind, text});
        return Promise.resolve(undefined);
    };
    const vscode = Object.freeze({
        commands: Object.freeze({
            registerCommand(id, callback) {
                if (!allowed.has(id) || typeof callback !== 'function' || handlers.has(id))
                    throw new Error('Only unique declared commands may be registered');
                handlers.set(id, callback);
                return Object.freeze({dispose() { handlers.delete(id); }});
            },
            executeCommand(id, ...args) { return Promise.resolve(invoke(id, args)); },
            getCommands() { return Promise.resolve([...handlers.keys()]); }
        }),
        window: Object.freeze({
            showInformationMessage(text) { return message('info', text); },
            showWarningMessage(text) { return message('warning', text); },
            showErrorMessage(text) { return message('error', text); }
        }),
        workspace: Object.freeze({
            getConfiguration(section) {
                const key = name => section ? section + '.' + name : name;
                return Object.freeze({
                    get(name, fallback) {
                        return Object.prototype.hasOwnProperty.call(defaults, key(name)) ? defaults[key(name)] : fallback;
                    },
                    has(name) { return Object.prototype.hasOwnProperty.call(defaults, key(name)); }
                });
            },
            isTrusted: false
        })
    });
    globalThis.require = id => {
        if (id !== 'vscode') throw new Error('Module not available: ' + id);
        return vscode;
    };
    globalThis.module = {exports: {}};
    globalThis.exports = globalThis.module.exports;
    globalThis.__run = async (id, args) => {
        const extension = module.exports;
        if (typeof extension.activate !== 'function') throw new Error('Missing activate export');
        const subscriptions = [];
        await extension.activate(Object.freeze({subscriptions, extensionMode: 2}));
        const value = await invoke(id, args);
        globalThis.__result = JSON.stringify({value: value === undefined ? null : value, messages});
        // This one-command runtime is discarded. No extension cleanup runs on interrupt.

    };
})();
