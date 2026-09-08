# Adding a device

Add support in narrow, reviewable stages.

1. Create `crates/hyperx-devices/src/<model>.rs` with the verified VID/PID,
   exact configuration collection selector and manufacturer-backed
   capabilities.
2. Export its descriptor and add it to `SUPPORTED_DEVICES` in
   `crates/hyperx-devices/src/lib.rs`.
3. Add identity and selector tests. Do not derive IDs from a related model.
4. Run `hyperx-cli --trace devices` on Windows and paste sanitized observations
   into `docs/research.md` with date, hardware release and evidence source.
5. Add read-only protocol work first. Keep encoders/decoders in
   `hyperx-protocol` or the model module and access HID only through
   `HidTransport`.
6. Add each mutable capability after its own captures, golden packets and mock
   transport test.

Never open every collection for a matching VID/PID. Select the exact
interface/usage tuple and leave standard mouse/keyboard collections alone.
Hardware capability declarations do not make CLI operations available; the
driver must separately advertise implemented and verified operations.

If a new model shares a packet family, extract the common codec only after two
devices demonstrate that commonality. Avoid a speculative “universal HyperX”
protocol.
