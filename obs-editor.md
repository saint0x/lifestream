# Vanta OBS Remaining Production Gates

Everything completed and verified has been removed from this document.

Remaining work:

- Execute production signing and notarization for macOS helper installers with Vanta Developer ID credentials, including the native audio helper.
- Verify signed and stapled macOS helper distribution artifacts from the production signing/notarization run.
- Execute production Authenticode signing for Windows helper binaries and installer artifacts on a Windows signing host.
- Verify signed Windows helper distribution artifacts from the production signing run.
- Produce and verify the permission-granted ScreenCaptureKit system-audio validation artifact from the signed macOS audio helper.

Optional vendored OBS track, only if Vanta explicitly chooses to ship copied or linked OBS/libobs code:

- Secure legal approval for GPL obligations and explicit approval to enable the isolated optional vendor track.
- Secure a signed-off open-source distribution posture if Vanta ships a derivative.
