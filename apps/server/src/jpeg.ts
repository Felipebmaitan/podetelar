import {
  PREVIEW_MAX_BYTES,
  PREVIEW_MAX_WIDTH,
  PREVIEW_MAX_HEIGHT,
} from "@freedomstream/contracts";

/** Validates one small baseline JPEG without decoding/transcoding any pixels. */
export function jpegDimensions(
  bytes: Buffer,
): { width: number; height: number } | null {
  if (
    bytes.length < 16 ||
    bytes.length > PREVIEW_MAX_BYTES ||
    bytes[0] !== 0xff ||
    bytes[1] !== 0xd8
  )
    return null;
  let position = 2;
  let dimensions: { width: number; height: number } | null = null;
  let components = 0;
  while (position + 1 < bytes.length) {
    if (bytes[position++] !== 0xff) return null;
    while (bytes[position] === 0xff) position++;
    const marker = bytes[position++];
    if (marker === undefined || position + 2 > bytes.length) return null;
    const length = bytes.readUInt16BE(position);
    if (length < 2 || position + length > bytes.length) return null;
    if (marker === 0xc0) {
      if (dimensions || length < 11 || bytes[position + 2] !== 8) return null;
      const height = bytes.readUInt16BE(position + 3);
      const width = bytes.readUInt16BE(position + 5);
      components = bytes[position + 7]!;
      if (
        ![1, 3].includes(components) ||
        length !== 8 + 3 * components ||
        width < 1 ||
        width > PREVIEW_MAX_WIDTH ||
        height < 1 ||
        height > PREVIEW_MAX_HEIGHT
      )
        return null;
      dimensions = { width, height };
    } else if (marker === 0xda) {
      if (
        !dimensions ||
        length !== 6 + 2 * components ||
        bytes[position + 2] !== components ||
        bytes[position + length - 3] !== 0 ||
        bytes[position + length - 2] !== 63 ||
        bytes[position + length - 1] !== 0
      )
        return null;
      position += length;
      // One baseline scan only: no later SOF can change the validated dimensions,
      // and no concatenated image or trailing payload is accepted after EOI.
      while (position < bytes.length) {
        if (bytes[position++] !== 0xff) continue;
        while (bytes[position] === 0xff) position++;
        const entropyMarker = bytes[position++];
        if (
          entropyMarker === 0 ||
          (entropyMarker !== undefined &&
            entropyMarker >= 0xd0 &&
            entropyMarker <= 0xd7)
        )
          continue;
        return entropyMarker === 0xd9 && position === bytes.length
          ? dimensions
          : null;
      }
      return null;
    } else if (!(
      [0xc4, 0xdb, 0xdd, 0xfe].includes(marker) ||
      (marker >= 0xe0 && marker <= 0xef)
    ))
      return null;
    position += length;
  }
  return null;
}
