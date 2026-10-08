// Encode a JS string as one Noble Text literal. Noble uses \u{...}, not
// JSON's \uXXXX spelling, and forbids raw Unicode line separators.
export function literal(value) {
  if (typeof value !== 'string' || Buffer.byteLength(value) > 4096)
    throw Error('text field exceeds 4096 UTF-8 bytes');
  let encoded = '"';
  for (const character of value) {
    const scalar = character.codePointAt(0);
    if (scalar >= 0xd800 && scalar <= 0xdfff)
      throw Error('text field contains an unpaired surrogate');
    if (character === '"' || character === '\\') encoded += `\\${character}`;
    else if (character === '\n') encoded += '\\n';
    else if (character === '\r') encoded += '\\r';
    else if (character === '\t') encoded += '\\t';
    else if (scalar < 32 || scalar === 0x85 || scalar === 0x2028 || scalar === 0x2029)
      encoded += `\\u{${scalar.toString(16)}}`;
    else encoded += character;
  }
  return `${encoded}"`;
}
