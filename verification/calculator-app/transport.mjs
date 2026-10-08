// Finite NDJSON framing. A single oversized line is discarded through its
// newline without consuming the next command or terminating the live session.
export async function* boundedLines(chunks, maximumBytes) {
  let pending = [], size = 0, oversized = false;
  for await (const chunk of chunks) {
    let offset = 0;
    while (offset < chunk.length) {
      const newline = chunk.indexOf(10, offset);
      const end = newline < 0 ? chunk.length : newline;
      const segment = chunk.subarray(offset, end);
      if (!oversized) {
        if (size + segment.length > maximumBytes) {
          oversized = true;
          pending = [];
          size = 0;
        } else {
          pending.push(segment);
          size += segment.length;
        }
      }
      if (newline >= 0) {
        yield oversized ? null : Buffer.concat(pending, size);
        pending = [];
        size = 0;
        oversized = false;
      }
      offset = end + (newline < 0 ? 0 : 1);
    }
  }
  if (oversized || size) yield oversized ? null : Buffer.concat(pending, size);
}
