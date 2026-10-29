// The clean foundation receipts that bind the current bytes of a Lean model.
//
// A differential lane compares an implementation against expectations written from a Lean model,
// and its receipt should name the kernel checks that cover that model. A hard-coded receipt path
// goes stale as soon as the model changes. These helpers find every retained clean-source
// `verification/foundation/receipts/*/foundation-check.json` whose `sources_sha256` matches the
// model files byte for byte, and refuse when none does.

import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const receiptsDirectory = 'verification/foundation/receipts';

const digest = (root, file) =>
  crypto.createHash('sha256').update(fs.readFileSync(path.join(root, file))).digest('hex');

/**
 * @param {string} root repository root
 * @param {string[]} modelFiles repository-relative Lean files the lane's expectations come from
 * @param {string} namespace theorem-name prefix of the model, without the trailing dot
 */
export function receiptsBindingModel(root, modelFiles, namespace) {
  const current = Object.fromEntries(modelFiles.map((file) => [file, digest(root, file)]));
  const directory = path.join(root, receiptsDirectory);
  const receipts = [];
  for (const name of fs.readdirSync(directory).sort()) {
    const file = path.join(directory, name, 'foundation-check.json');
    if (!fs.existsSync(file)) continue;
    const receipt = JSON.parse(fs.readFileSync(file, 'utf8'));
    if (receipt.clean_source !== true) continue;
    if (!modelFiles.every((model) => receipt.sources_sha256?.[model] === current[model])) continue;
    const theorems = (receipt.theorems ?? []).filter((theorem) => theorem.name.startsWith(`${namespace}.`));
    receipts.push({
      receipt: path.relative(root, file),
      source_commit: receipt.source_commit,
      model_theorems: theorems.length,
    });
  }
  if (receipts.length === 0) {
    throw new Error(`no clean foundation receipt binds the current bytes of ${modelFiles.join(', ')}; ` +
      'run verification/foundation/check.mjs and retain its receipt before recording this lane');
  }
  return { model_files_sha256: current, receipts };
}
