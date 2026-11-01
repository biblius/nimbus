# nimbus-cli

Small utility for interacting with `nimbus-server` via the CLI.

Comes in handy if you have a host that already has some files
and want to directly upload them to Nimbus.

Can also be run directly on the Nimbus host, in which case you should set
the server URL to point to the local address Nimbus is listening
on (if you have followed the hosting guide).

## How to use it

Build the release version for your target architecture.

```bash
cargo build --release -p nimbus-cli
```

Put the resulting binary in `/usr/local/bin`.

To see the available commands, run

```bash
nimbus
```
