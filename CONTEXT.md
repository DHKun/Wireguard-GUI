# WireGuard Console

This context covers configuring local WireGuard interfaces and applying those configurations to the running Linux host.

## Language

**Interface Configuration**:
The persisted desired state of one WireGuard interface, including interface settings and its Peers.
_Avoid_: Config file, profile

**Runtime Interface**:
The active state of a WireGuard interface currently managed by the Linux kernel.
_Avoid_: Live config, running config

**Peer**:
A remote WireGuard participant identified by its public key and permitted network ranges.
_Avoid_: Client, node

**Apply Mode**:
The requested scope of a change: update only the Runtime Interface, or persist the Interface Configuration and synchronize it to the Runtime Interface.
_Avoid_: Sync flag, save option

**Apply Outcome**:
The observable result of applying a change, recording whether persistence and runtime synchronization each completed and any warning from a partial success.
_Avoid_: Success flag, command result
