# Protocol evidence and safety gate

Read this reference before adding or changing any vendor report, state-changing
operation, profile write, or hardware test.

## Evidence levels

- **Confirmed locally:** repeated isolated captures or descriptor data from the
  target hardware and firmware revision.
- **Confirmed publicly:** an attributable implementation or technical source
  names the exact target model, interface, report type, length, and bytes.
- **Hypothesis:** analogy, a single unexplained diff, sibling-model behavior, or
  a report with unknown fields.

Read operations need confirmed local or public evidence. Volatile writes need
confirmed evidence plus a golden packet and mock transport test. Persistent
writes need repeated local captures, a power-cycle experiment, explicit CLI
intent, and review of all changed bytes. Hypotheses must never reach the real
transport.

## Packet review

For every report record:

- exact model, VID/PID, release/firmware if known;
- HID interface, usage page/usage, report type and report ID;
- total size including any report-ID padding;
- meaning and allowed range of every non-constant field;
- constant bytes and zero-fill requirements;
- request/response ordering, delays, timeout and retry behavior;
- volatile/persistent effect and rollback or power-cycle result;
- evidence URL or capture fixture.

Reject malformed lengths and unsupported values before opening the device.
Trace complete TX/RX reports at trace level, but do not log them by default.
Treat capture files as potentially sensitive.

## Hardware test sequence

1. Close NGENUITY and other RGB/peripheral writers.
2. Confirm `hyperx-cli devices` still selects the expected vendor collection.
3. Record the current setting and ensure the mouse can be unplugged/replugged.
4. Send one confirmed volatile operation.
5. Verify cursor, all buttons, reconnect, and expected timeout/revert behavior.
6. Test boundary values only when the encoder rejects all out-of-range inputs.
7. Test persistent writes separately and only with explicit user authorization.

Stop immediately on disconnect loops, cursor/button failure, unexpected profile
changes, a changed USB identity, or any bootloader/DFU indication. Do not retry
the same packet automatically after an ambiguous failure.

## Captures required for unknown commands

Capture one UI change at a time and repeat it. Include a no-op capture to remove
keepalive/background traffic. Export complete ordered reports as hex; retain
direction, interface, transfer/report type, and timing. Follow
`docs/reverse-engineering.md` for the experiment matrix and fixture format.
