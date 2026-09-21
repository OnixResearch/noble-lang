// The CLI embeds these same fragments with concat!/include_str!; no bundler,
// source transformation, guest interpreter or runtime filesystem import is used there.
import fs from 'node:fs';

const source = ['preamble', 'engine', 'protocol'].map(name =>
  fs.readFileSync(new URL(`./${name}.mjs`, import.meta.url), 'utf8')).join('');
const loaded = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
export const CoreEngine = loaded.CoreEngine;
