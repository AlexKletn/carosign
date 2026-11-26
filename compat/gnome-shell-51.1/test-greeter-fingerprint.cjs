// Exercises two stock and two patched GNOME Shell 51.1 methods.
// Usage: node test-greeter-fingerprint.cjs STOCK.js PATCHED.js
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');

if (process.argv.length !== 4)
    throw new Error('нужны исходный и патченный authServicesLegacy.js');
const original = fs.readFileSync(process.argv[2], 'utf8');
const patched = fs.readFileSync(process.argv[3], 'utf8');
const hash = crypto.createHash('sha256').update(original).digest('hex');
assert.equal(hash, '3b0c729d4993f2d41e19f7a5e7b6436b7bf0226fec84f75929679934ee9d9952');

function method(source, name) {
    const start = source.search(new RegExp(`^    ${name}\\(`, 'm'));
    assert.notEqual(start, -1);
    const end = source.indexOf('\n    }', start);
    assert.notEqual(end, -1);
    return source.slice(start, end + '\n    }'.length).trim();
}

function methods(source, manager) {
    const factory = new Function('FingerprintManager', 'FINGERPRINT_SERVICE_NAME',
        `return ({${method(source, '_connectFingerprintManager')},\n${method(source, '_handleCanStartService')}})`);
    return factory({getFingerprintManager: () => manager}, 'gdm-fingerprint');
}

const manager = {connectObject() { this.connected = true; }};
const originalMethods = methods(original, manager);
const patchedMethods = methods(patched, manager);
const state = {
    _reauthOnly: false,
    _userName: null,
    _selectedMechanism: {serviceName: 'gdm-password'},
    _enabledMechanisms: [{serviceName: 'gdm-fingerprint'}],
    _hasAnyCredentialManagerToken: () => false,
};
originalMethods._connectFingerprintManager.call(state);
assert.equal(state._fingerprintManager, undefined);
patchedMethods._connectFingerprintManager.call(state);
assert.equal(state._fingerprintManager, manager);
assert.equal(manager.connected, true);
assert.equal(patchedMethods._handleCanStartService.call(state, 'gdm-password'), true);
assert.equal(patchedMethods._handleCanStartService.call(state, 'gdm-fingerprint'), false);
state._selectedMechanism = {serviceName: 'gdm-fingerprint'};
assert.equal(patchedMethods._handleCanStartService.call(state, 'gdm-fingerprint'), false);
state._hasAnyCredentialManagerToken = () => true;
state._credentialManagers = {};
assert.equal(patchedMethods._handleCanStartService.call(state, 'gdm-fingerprint'), false);
state._hasAnyCredentialManagerToken = () => false;
state._userName = 'alex';
assert.equal(patchedMethods._handleCanStartService.call(state, 'gdm-fingerprint'), true);
state._selectedMechanism = {serviceName: 'gdm-password'};
assert.equal(patchedMethods._handleCanStartService.call(state, 'gdm-password'), true);
console.log('Greeter подключает менеджер; fingerprint требует пользователя, пароль сохранён.');
