#!/usr/bin/env python3
"""Turn the firmware's defmt 'PCM [...]' log lines into a playable WAV.

The firmware (src/bin/minimal.rs) emits one line per I2S DMA buffer:

    <ts> INFO  PCM [12, -34, 56, ...]

each holding PCM_CHUNK decimated, mono, 16-bit samples. This script reads the
defmt text stream on stdin, extracts those samples in order, and writes a WAV.

Usage:
    cargo run --bin minimal 2>&1 | python3 tools/pcm_to_wav.py out.wav
    # or convert an already-captured log:
    python3 tools/pcm_to_wav.py out.wav < session.log

Stop the capture with Ctrl-C; the WAV is finalized on exit.

The sample rate MUST match the firmware: 32000 / DECIMATE. With DECIMATE = 4
that is 8000 Hz. Override with --rate if you change DECIMATE.
"""

import argparse
import re
import sys
import wave

# Matches the bracketed integer list in a "PCM [...]" defmt line.
PCM_RE = re.compile(r"PCM\s*\[([^\]]*)\]")


def parse_samples(line):
    m = PCM_RE.search(line)
    if not m:
        return None
    body = m.group(1).strip()
    if not body:
        return []
    out = []
    for tok in body.split(","):
        tok = tok.strip()
        if tok:
            out.append(int(tok))
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("output", help="output .wav path")
    ap.add_argument("--rate", type=int, default=8000,
                    help="sample rate in Hz; must equal 32000 / DECIMATE (default 8000)")
    args = ap.parse_args()

    total = 0
    lines = 0
    with wave.open(args.output, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)   # 16-bit
        w.setframerate(args.rate)
        try:
            for line in sys.stdin:
                samples = parse_samples(line)
                if samples is None:
                    continue
                lines += 1
                # Clamp into i16 range defensively, then pack little-endian.
                frames = bytearray()
                for s in samples:
                    if s < -32768:
                        s = -32768
                    elif s > 32767:
                        s = 32767
                    frames += int(s).to_bytes(2, "little", signed=True)
                w.writeframes(bytes(frames))
                total += len(samples)
                if lines % 125 == 0:
                    secs = total / args.rate
                    print(f"\r{total} samples ({secs:.1f}s) @ {args.rate} Hz",
                          end="", file=sys.stderr, flush=True)
        except KeyboardInterrupt:
            pass

    secs = total / args.rate if args.rate else 0
    print(f"\nWrote {total} samples ({secs:.1f}s) to {args.output}",
          file=sys.stderr)


if __name__ == "__main__":
    main()
