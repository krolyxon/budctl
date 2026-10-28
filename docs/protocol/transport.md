# Transport: Linux Bluetooth sockets, SDP and BlueZ

This file covers the parts of the link that are the same for every device: finding paired devices, finding the RFCOMM channel and opening sockets. Device protocols are in `<device>.md`.

All of this is public, standard material:
- the Bluetooth Core Specification (SDP: Vol 3, Part B);
- the Linux kernel's Bluetooth socket headers (`include/net/bluetooth/{bluetooth,l2cap,rfcomm}.h`);
- BlueZ's D-Bus API documentation (`doc/org.bluez.Device.rst`, `doc/org.freedesktop.DBus.ObjectManager`).



<a id="paired-devices"></a>
## 1. Paired devices (BlueZ over D-Bus)

- **Connection.** budctl talks to BlueZ only over the **system bus**. The D-Bus specification gives its default address as `unix:path=/var/run/dbus/system_bus_socket`. budctl connects to that path itself and hands the stream to zbus. It never reads `DBUS_SYSTEM_BUS_ADDRESS`, which could name a TCP address.
- **Listing devices.** Call `org.freedesktop.DBus.ObjectManager.GetManagedObjects` on service `org.bluez`, path `/`. Each object that has the interface `org.bluez.Device1` is a known device.
- **Device1 properties used:**

  | Property | Type | Use |
  |---|---|---|
  | `Address` | string `"AA:BB:CC:DD:EE:FF"` | the device to connect to |
  | `Name` | string | the name the device reports. Recognition uses this, because the user can rename `Alias` |
  | `Alias` | string | shown to the user |
  | `Paired` | bool | only paired devices are listed |
  | `Connected` | bool | budctl only opens a control link when the audio link is already up |
  | `UUIDs` | array of strings | service UUIDs from the device's SDP record, as BlueZ caches them (lowercase) |

- **Shutdown.** Drop the D-Bus connection before opening Bluetooth sockets. zbus runs an internal thread while a connection is alive.

<a id="sockets"></a>
## 2. Linux Bluetooth sockets

**Constants** (from `<sys/socket.h>` and the kernel Bluetooth headers):

| Name | Value |
|---|---|
| `AF_BLUETOOTH` | 31 |
| `BTPROTO_L2CAP` | 0 (used with `SOCK_SEQPACKET`, for SDP) |
| `BTPROTO_RFCOMM` | 3 (used with `SOCK_STREAM`, for the control link) |

**Address byte order.** A `bdaddr_t` holds the address **reversed**: `AA:BB:CC:DD:EE:FF` is stored as bytes `FF EE DD CC BB AA`.

`struct sockaddr_rc` is 10 bytes:

| Offset | Field | Type |
|---|---|---|
| 0 | `rc_family` | u16, `AF_BLUETOOTH` |
| 2 | `rc_bdaddr` | 6 bytes, reversed address |
| 8 | `rc_channel` | u8 |
| 9 | padding | — |

`struct sockaddr_l2` is 14 bytes:

| Offset | Field | Type |
|---|---|---|
| 0 | `l2_family` | u16 |
| 2 | `l2_psm` | u16, little endian |
| 4 | `l2_bdaddr` | 6 bytes, reversed address |
| 10 | `l2_cid` | u16, little endian, 0 |
| 12 | `l2_bdaddr_type` | u8, 0 = BR/EDR |
| 13 | padding | — |

The SDP server always listens on **PSM 1**.

<a id="connect-timeout"></a>
**Connect timeout.**
- For Bluetooth sockets, a blocking `connect()` waits at most the socket's `SO_SNDTIMEO`.
- When that time runs out, the kernel returns **`EINPROGRESS`** (`bt_sock_wait_state`), not `ETIMEDOUT`. budctl treats both as a timeout.

<a id="errno"></a>
**What budctl makes of errno.** This mapping is our own choice. It decides the exit code.

| errno | Meaning for budctl | Exit |
|---|---|---|
| `EBUSY`, `EADDRINUSE`, `ECONNREFUSED` on the RFCOMM connect | another client holds the control channel | 3 |
| `EHOSTDOWN`, `EHOSTUNREACH`, `ENOTCONN`, `ECONNRESET`, `ECONNABORTED`, `EPIPE`, `ENODEV`, `ENETDOWN` | buds out of range, adapter off, or link dropped (for example, the case lid closed) | 2 |
| `ETIMEDOUT`, `EINPROGRESS`, `EAGAIN` | no answer in time | 4 |
| anything else | OS error, reported with its text | 2 |

**Reads** use `poll(POLLIN)` with the time left until the deadline. A `read()` that returns 0 bytes means the buds closed the link.

**Writes** use `send(..., MSG_NOSIGNAL)`, so a dead link gives `EPIPE` instead of killing the process with `SIGPIPE`.

<a id="sdp"></a>
## 3. Finding the RFCOMM channel with SDP

The buds publish their control service in their SDP record. budctl asks for that record over L2CAP PSM 1 and reads the RFCOMM channel number out of it. It never uses a fixed channel.

<a id="sdp-pdu"></a>
**PDU header** (Core Spec Vol 3 Part B §4.2). All multi-byte fields are **big endian**.

| Field | Size |
|---|---|
| PDU id | 1 |
| transaction id | 2 |
| parameter length | 2 |
| parameters | n |

| PDU id | Name |
|---|---|
| `0x01` | ErrorResponse; parameters: error code (u16) |
| `0x06` | ServiceSearchAttributeRequest |
| `0x07` | ServiceSearchAttributeResponse |

<a id="sdp-request"></a>
**ServiceSearchAttributeRequest parameters** (§4.7.1):
1. ServiceSearchPattern: a data-element sequence of UUIDs. budctl sends one 128-bit UUID: `35 11 1C <16 UUID bytes>`.
2. MaximumAttributeByteCount (u16): `FF FF`. The server still splits a long answer with continuation state.
3. AttributeIDList: a data-element sequence of attribute ids. budctl asks only for `0x0004`, the ProtocolDescriptorList: `35 03 09 00 04`.
4. ContinuationState: 1 length byte followed by that many bytes. It is `00` on the first request, and on each later request it is copied from the previous response.

<a id="sdp-response"></a>
**ServiceSearchAttributeResponse parameters** (§4.7.2):
1. AttributeListsByteCount (u16).
2. AttributeLists: that many bytes. This is one part of a data element. Joined across all responses, it forms the complete data element.
3. ContinuationState. If its length byte is not 0, send the request again with this state. budctl gives up after 16 rounds.

<a id="sdp-data-elements"></a>
**Data elements** (§3).
- **Header byte:** `type = byte >> 3`, `size index = byte & 7`.
- **Types used here:** 0 nil, 1 unsigned int, 2 signed int, 3 UUID, 4 text, 5 bool, 6 sequence, 7 alternative, 8 URL.
- **Size index:**
  - 0 → 1 byte of data (nil: 0 bytes)
  - 1 → 2 bytes
  - 2 → 4 bytes
  - 3 → 8 bytes
  - 4 → 16 bytes
  - 5, 6, 7 → the data length is in the next 1, 2 or 4 bytes (big endian), then the data follows.
- **UUIDs** are 2, 4 or 16 bytes. A short UUID `xxxx` or `xxxxxxxx` stands for `xxxxxxxx-0000-1000-8000-00805F9B34FB` (the Bluetooth base UUID).

<a id="sdp-rfcomm-channel"></a>
**Reading the channel.**
- The joined AttributeLists is a sequence with one item per matching service record. Each item is a sequence of (attribute id, value) pairs.
- For attribute `0x0004`, the value is a sequence of protocol descriptors, each itself a sequence that starts with a UUID.
- The descriptor whose UUID is **RFCOMM (`0x0003`)** has the channel number next, as an unsigned 8-bit integer.
- Example:
  `35 0C  35 03 19 01 00  35 05 19 00 03 08 0F`
  - `35 03 19 01 00` is L2CAP (`0x0100`).
  - `35 05 19 00 03 08 0F` is RFCOMM, channel 15.
- budctl walks the data elements properly. It does not search for a byte pattern.
