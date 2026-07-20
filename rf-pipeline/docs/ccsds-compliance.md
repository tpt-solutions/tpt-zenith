# CCSDS standard compliance notes

Notes on which CCSDS (Consultative Committee for Space Data Systems)
standards this crate takes inspiration from, what it currently implements,
and the concrete gaps to close before calling any of it "compliant". This is
a simulation-first crate (Phase 4); nothing here has been validated against
a certified CCSDS test vector suite.

## Relevant standards

| Standard | Title | Covers |
|---|---|---|
| CCSDS 133.0-B-2 | Space Packet Protocol | Packet framing: primary header, APID, sequence flags/count, data field |
| CCSDS 131.0-B-4 | TM Synchronization and Channel Coding | Downlink framing, convolutional/Reed-Solomon/LDPC/turbo coding, sync markers |
| CCSDS 231.0-B-4 | TC Synchronization and Channel Coding | Uplink framing, BCH coding, randomization |
| CCSDS 232.0-B-4 | TC Space Data Link Protocol | Command frame structure, sequencing, retransmission |

## What's implemented (`frame.rs`, `fec.rs`, `modem.rs`)

- **Framing**: `SpacePacketHeader`/`SpacePacket` implement the CCSDS
  133.0-B-2 Space Packet primary header exactly: 3-bit version, 1-bit
  type, 1-bit secondary header flag, 11-bit APID, 2-bit sequence flags,
  14-bit sequence count, 16-bit packet data length (encoded as length-1).
  Field widths, bit order, and the length-1 encoding are all per spec.
  Round-tripped and range-validated in `frame.rs`'s tests.
- **Error correction**: `Hamming74` is a generic single-error-correcting
  block code, included to give the modem chain *something* concrete to
  correct errors with and to demonstrate the encode/modulate/channel/
  demodulate/decode pipeline end-to-end (see `fec_reduces_bit_error_rate_
  on_a_noisy_channel` in `lib.rs`). It is not a CCSDS-specified code.
- **Modulation**: `Bpsk`/`Qpsk` are generic, unit-energy phase-shift-keying
  modems. CCSDS doesn't mandate a specific modulation for space packets
  (that's a link/RF-layer choice made per mission), so these aren't
  "non-compliant" so much as "one reasonable choice among several the
  standards permit."

## What's not implemented (gaps to close for a standards-track claim)

- **CCSDS-specified FEC.** 131.0-B-4 specifies convolutional coding (rate
  1/2, constraint length 7), Reed-Solomon (255,223) as an outer code, and
  more recent LDPC/turbo options. `Hamming74` is not one of these; a
  standards-track implementation needs at least the convolutional +
  Reed-Solomon concatenated scheme (the historical baseline) with a
  correctness reference (e.g. published test vectors) to validate against,
  the same way `orbital-mechanics`' SGP4 port is validated against
  `tcppver.out`.
- **Transfer Frame layer.** 131.0-B-4/231.0-B-4 define Transfer Frames
  (fixed-length, with attached sync marker, frame header error control,
  and CLTUs for uplink) that Space Packets get multiplexed into for
  transmission. This crate only implements the Space Packet layer, not
  Transfer Frames.
- **Randomization/scrambling**, used on both TM and TC links for spectral
  and bit-transition-density reasons, is not implemented.
- **CRC/frame error control** (e.g. CCSDS-specified CRC-16 or CRC-32 for
  Transfer Frame or CLTU error detection) is not implemented; `Hamming74`
  provides correction but nothing here implements the specific error-
  detection polynomials CCSDS specifies.
- **Secondary headers** are represented (`secondary_header: bool` on
  `SpacePacketHeader`) but no secondary header format (e.g. CCSDS Unsegmented
  Time Code, used for packet timestamps) is implemented.

## Suggested order if compliance work continues

1. Reed-Solomon (255,223) encoder/decoder, validated against a published
   reference (e.g. the CCSDS 131.0-B-4 test vectors or a well-known
   independent RS implementation), as a second `Encoder`/`Decoder` alongside
   `Hamming74`.
2. Rate-1/2 constraint-length-7 convolutional code + Viterbi decoder,
   concatenated with (1) as the historical CCSDS baseline.
3. TM Transfer Frame framing (fixed length, attached sync marker, frame
   header) wrapping `SpacePacket`s, mirroring how `frame.rs` wraps payload
   bytes today.
4. A CCSDS Unsegmented Time Code secondary header, so packets carry a
   standards-compliant timestamp.
