# FlowTorrent macOS multicast correction

Upstream: librqbit-lsd 9.0.1, rqbit commit
`a499d2f243d124e144aef137afe7cb304a6e3f36`, `crates/librqbit_lsd`.
The original Apache-2.0 license is in LICENSE. FlowTorrent modifications dated
2026-10-21 add a separate IPv4 socket on macOS and retain the IPv6 receiver.
Other platforms retain the upstream single dualstack socket.

Both Apple Silicon and Intel macOS CI demonstrate that IPv4 multicast on a
native AF_INET socket sends and receives on loopback, while mapped IPv4 on
AF_INET6 fails with EHOSTUNREACH using IPV6_MULTICAST_IF and EINVAL using
IP_MULTICAST_IF. Therefore an observer-only correction cannot fix discovery.

The correction uses the existing socket/interface enumeration, parser,
private-torrent gating, cancellation and rate limiting. It changes neither
the torrent protocol nor privacy defaults. IPv6 startup failure on macOS
leaves IPv4 discovery active and is logged.

Remove this override after an upstream version passes both announcement and
peer-discovery controls on both Mac architectures. No upstream issue or
message has been posted on the user's behalf.
