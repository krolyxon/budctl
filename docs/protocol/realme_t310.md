# realme Buds T310: control protocol

This is budctl's clean-room description of how to control the realme Buds T310. Code in `crates/budctl-core/src/devices/realme_t310/` is written from this file, not from any other project's source (CLAUDE.md rules 2 and 3). Each constant in that module cites an anchor here (`Spec: realme_t310.md#anchor`).

The T310 speaks the protocol family shared by many OPPO, OnePlus and realme buds. Linux socket, SDP and BlueZ details are in [`transport.md`](transport.md).

<a id="sources"></a>
## 0. Evidence

Every fact carries an evidence level that says how far it can be trusted. A fact that is not yet *ours* is tested on the owner's buds before budctl relies on it (CLAUDE.md rule 4).

| Level | Meaning |
|---|---|
| **t310** | seen on a real T310 by other users, in a report or log. Not yet verified by budctl |
| **ours-YYYY-MM-DD** | verified by budctl on the owner's T310 (see the [verification log](#verification-log)) |
| **vendor** | how the vendor apps (realme Link, HeyMelody) behave. Not seen on a T310 |
| **buds4** | seen on a OnePlus Buds 4. The framing is shared, but the values may differ on the T310 |
| **derived** | computed by us from the facts above. Never seen on the wire |
| **guess** | unverified anywhere. budctl must not send it |

<a id="identity"></a>
## 1. Identity

- **Bluetooth name:** `realme Buds T310`. budctl recognizes the buds by BlueZ's `Name` property, compared exactly. *vendor*
- **Product id:** `063C12`. The reply to `0x0103` should be `00 12 3C 06`, which is the id in 3 bytes, little endian. The reply format is *buds4*; the T310 bytes are *derived*.
- **Not a T310.** If the product id is different, budctl stops: the device is renamed, or it is some other model.

<a id="transport"></a>
## 2. Transport

- **Link:** classic Bluetooth RFCOMM (SPP), not BLE. The connection is one long-lived byte stream, and the buds may send at any time.
- **Service UUIDs.** Try them in this order; each is resolved to a channel through SDP ([transport.md#sdp](transport.md#sdp)). The owner's T310 offers the first, on channel 6 (*ours-2026-10-27*).
  1. `0000079A-D102-11E1-9B23-00025B00A5A5`. The vendor app tries it first. *buds4/vendor*
  2. `00001107-D102-11E1-9B23-00025B00A5A5`. It is kept for other models; on Buds 4 it never connects.
- **Fixed channel.** Android clients sometimes use a fixed channel 15. budctl does not use a fixed channel.
- **One client at a time.** The buds serve one control app at a time. budctl reports a busy channel instead of retrying.
- **Lid close.** Closing the case lid ends the link. *buds4*

<a id="frame"></a>
## 3. Frame format

Every message is wrapped in one frame.

```
AA  len  slice  00  [fsn]  cmd(2, LE)  seq  paylen(2, LE)  payload
```

| Field | Size | Meaning |
|---|---|---|
| `AA` | 1 | start of frame |
| len | 1-3 | **LEB128**: 7 bits per byte, low bits first, top bit = another byte follows. It counts every byte after the length itself |
| slice | 1 | `00` = the whole message in one frame; `01` / `02` / `03` = first / middle / last slice of a long message |
| `00` | 1 | always zero |
| fsn | 1 | slice number from `00`; present only in slices (`01`-`03`) |
| cmd | 2 | command id, little endian |
| seq | 1 | sequence byte (see [§4](#seq)) |
| paylen | 2 | payload length, little endian |
| payload | paylen | message body |

<a id="frame-whole"></a>
**Whole message.** `len = 7 + paylen`. There is no checksum. *buds4*

Examples (*buds4*):

```
AA 07 00 00 00 01 01 00 00                  handshake 0x0100, seq 01, no payload
AA 0A 00 00 04 04 06 03 00 01 01 01         0x0404 seq 06, payload 01 01 01
AA 0C 00 00 04 02 FF 05 00 03 01 01 08 00   0x0204 push (seq FF), payload 03 01 01 08 00
```

<a id="frame-long"></a>
**Long lengths.** A 124-byte payload gives `len = 131`, sent as `AA 83 01`. A 250-byte reply arrives as `AA 81 02` (257). *buds4*

<a id="frame-slices"></a>
**Slices** (*vendor*):
- A slice frame is `AA len slice 00 fsn <part of cmd/seq/paylen/payload>`, with `len = 3 + part length`.
- The receiver joins the parts in order and parses the result when the `03` slice arrives. A middle or last slice with no first slice is dropped.
- Senders slice only above about 660 bytes, so budctl never sends slices.

<a id="frame-from-buds"></a>
**Frames the buds start** (*ours-2026-10-27*). On the owner's T310, every frame the buds send on their own (requests such as `0x0501` and `0x0204` pushes) has `08` in the slice byte, not `00`. The rest of the frame is laid out as a whole message, and `len = 7 + paylen` holds. Replies carry `00`. budctl therefore reads a slice byte of `00` or `08` as a whole message. Other values are not seen and are still dropped. Captured examples:

```
AA 07 08 00 01 05 0F 00 00                        0x0501 seq 0F, no payload
AA 0F 08 00 04 02 10 08 00 01 03 02 64 01 64 03 00   0x0204 seq 10, battery push 01 03 02 64 01 64 03 00
```

<a id="frame-receive"></a>
**Receiving**:
- Treat the input as a byte stream: one read may hold several frames or part of one.
- Throw away bytes before `AA`.
- If the length is implausible (more than 3 length bytes, below 4, or above 4096), drop that `AA` and look for the next one.
- If the paylen claims more bytes than the frame holds, keep the bytes that are there.

<a id="seq"></a>
## 4. Sequence numbers and replies

- **Replies.** The sender picks `seq`. The reply has the same seq and the command id with bit 15 set (`0x0106` → `0x8106`). budctl matches a reply on (seq, cmd), which is also how HeyMelody does it. *buds4/vendor*
- **budctl's seq values.** budctl uses `01`-`BF`, counting up and wrapping. Pushes from Buds 4 use `FF` (*buds4*); the T310 uses its own counter ([§3](#frame-from-buds)).
- **Status byte.** The first payload byte of a reply is a status, and `00` means success. A read reply with any other status carries no data.
- **Command groups:**

  | Range | Group |
  |---|---|
  | `0x01xx` | queries |
  | `0x02xx` | event registration and pushes |
  | `0x03xx` | firmware update (budctl never sends these; see the non-goals) |
  | `0x04xx` | writes |
  | `0x05xx` | requests from the buds |
  | `0x0Fxx` | private |

<a id="bitmap"></a>
## 5. Command bitmap

- **Reading it.** The reply to the handshake `0x0100` is `00` followed by the bitmap bytes. Read them least-significant bit first: bit *n* is bit `n % 8` of byte `n / 8`.
- **Using it.** A set bit enables a fixed group of commands. **budctl never sends a command whose bit is clear**, as the vendor apps don't. Bits beyond the end of the bitmap count as clear. *vendor*
- **Always allowed:** `0x0100`-`0x0104`, `0x0106`, `0x010B`, `0x010D`. `0x0200` belongs to no bit; the vendor app sends it on every connect. *vendor/buds4*

Gate table for every command budctl uses or documents here:

| Command(s) | Gate |
|---|---|
| `0x0100` handshake, `0x0103` product id, `0x0106` battery, `0x010D` switch read, `0x0200` offered events | always |
| `0x0105` firmware | bit 0 |
| `0x0108` gesture read, `0x0401` gesture write | bit 3 |
| `0x0109` wearing | bit 4 |
| `0x0400` find my earbuds | bit 5 |
| `0x0403` switch write | bit 7 |
| `0x010C` noise-control read, `0x0404` noise-control write | bit 8 |
| `0x0405` fit test | bit 9 |
| `0x010F` EQ read, `0x0406` EQ select | bit 10 |
| `0x0114` current codec | bit 17 |
| `0x0205` subscribe | bit 22 |
| `0x0122` custom EQ list, `0x0418` custom EQ write | bit 34 |
| `0x0124` bass level read, `0x041B` bass level write | bit 38 |

<a id="bitmap-t310"></a>
**The T310's command set** (*t310*), from a log of a real T310, as a decoded list:

`0100 0101 0102 0103 0104 0105 0106 0107 0108 0109 010B 010C 010D 010F 0112 0114 0115 0116 0122 0124 0205 0400 0401 0403 0404 0405 0406 0407 040B 040D 040E 040F 0416 0418 041B 0F00 0F03 0F04 EF02`

- **Bits that must be set** for that list (*derived*): 0, 2, 3, 4, 5, 7, 8, 9, 10, 11, 17, 19, 20, 22, 29, 34, 38, 42.
- **A bitmap reply with exactly those bits:** `00 BD 0F 5A 20 44 04` (*derived*).
- **The owner's T310 answers** `00 BF 1F 5A 20 45 04 40 00` (*ours-2026-10-27*; the unit tests use it). That is every bit above plus bits 1, 12, 32 and 54, which gate no command budctl uses. Every bit listed under "what the T310 lacks" is clear.
- **What the T310 lacks.** These commands are absent, so budctl never sends them:
  - `0x012F` batch query (bit 56): every read goes out alone.
  - `0x041E` 3D-audio source (bit 43).
  - `0x0413` phone state (bit 27).
  - `0x0130`/`0x0427` alert volume (bit 57).
  - `0x0133`/`0x042D` tap sensitivity (bit 61).
  - `0x0423` game sound (bit 49).
  - `0x012A`/`0x0422` spatial type (bit 47).

<a id="init"></a>
## 6. Session start

**The vendor sequence** (*buds4*, matching HeyMelody):
- Wait 300 ms after the socket connects, then send one message every 200 ms.
- Order: `0x0100` → `0x0103` → `0x0200` → `0x0205` → reads.
- There is no authentication or key exchange.

**budctl does:**
1. Connect, then wait 300 ms. During the wait, keep answering the buds' requests ([§7](#buds-requests)).
2. `0x0100`. Keep the bitmap.
3. `0x0103`. The id must be `063C12` ([§1](#identity)).
4. <a id="events-offered"></a>`0x0200` → `00 <count> <codes...>`: the event codes the buds can push. Buds 4 offers `01 02 03 04 08 0B F1 F2 F3` (*buds4*). The owner's T310 offers `01 02 03 04 05 06 0D F1 F4` (*ours-2026-10-27*).
5. <a id="subscribe"></a>`0x0205 <count> <codes...>` (count first) subscribes. budctl asks for these codes, keeping only the ones the buds offered (*vendor*):
   - `01` battery, `02` wearing and `03` noise control;
   - `04` (fit-test result) if `0x0405` is allowed.

   Other clients also subscribe to `08` when `0x040D` is allowed. budctl does not: this spec does not say what event `08` carries or what `0x040D` does, and budctl handles no such event. Add it only once both are documented here with a source.

   Never send `01 01 02 02`: that means "count 1, code 01". The `0x8205` ack may never arrive (*buds4*, Nord Buds 3 Pro), so budctl does not wait for it. The T310 does send it, about 50 ms later: `00 04 01 00 02 00 03 00 04 00`, a `00` result for each code (*ours-2026-10-27*).
6. Reads, one per request.

**Spacing.** budctl keeps the captured spacing: at least **200 ms** between any two frames it sends. It waits for each reply before sending the next request.

<a id="buds-requests"></a>
## 7. Requests from the buds

The buds send some requests of their own. The answer repeats the request's seq and sets bit 15 of its command. Some firmware resets the link 3-5 s after connecting if the time request goes unanswered (Nord Buds 3 Pro, *buds4*-family capture).

| From the buds | budctl answers | Meaning |
|---|---|---|
| <a id="time-request"></a>`0x0500` | `00 <Unix seconds, u32 LE>` | time request, sent on connect |
| `0x050C` | `00 01 00` | "is the foreground app on my list?" — no |
| `0x051C` | `00 00 00` | host app capabilities: fast discovery off |
| `0x051D` | `01` | the phone's spatial mode (budctl always answers `01`) |
| a command outside groups `01`-`05` and `0F` | `01` | unknown request |

- **Test vector** (*buds4*): at Unix time 1750000000, the request `AA 07 00 00 00 05 02 00 00` gets the answer `AA 0C 00 00 00 85 02 05 00 00 80 E1 4E 68`.
- **Not answered:** other `0x05xx` messages and all pushes. Neither vendor app answers them.
- **Pushes `F2` and `F4`:** other clients acknowledge these. budctl does not yet; see [open questions](#open).
- **On the T310** (*ours-2026-10-27*): right after connecting, the buds send `0x0501` with no payload (slice byte `08`, their own seq). budctl does not answer it (it is an "other `0x05xx`" message), and no link reset followed in a 5 s session. No `0x0500` time request was seen.

<a id="pushes"></a>
## 8. Pushes

budctl reads none of these yet. It skips them while waiting for replies (and prints them with `--debug`).

- **`0x0204` event push.** Seq is `FF`; classify by cmd, because one Buds 4 push used another seq. `payload[0]` is the event code:

  | Code | Event | Layout after the code |
  |---|---|---|
  | `01` | battery | `<count> (<part> <raw>)…` (as in [§9.1](#battery)) |
  | `02` | wearing | `<count> (<part> <state>)…` |
  | `03` | noise control | `01 01 <mask LE>` = new mode; `02 <kind> <mask>` = new hold cycle (not a mode) |
  | `04` | fit-test result | `(<dev> <result>)…` |
  | `05` | game mode | `<state>`. **The T310 never sends this** (*t310*) |
  | `F1` | a gesture fired | `<side> <button> <act> <fn> …`. Diagnostic only |

- **`0x0503`** `<count> (<id> <value>)…`: switch values changed (*vendor*, other realme models).
- **`0x0504`** `<id>`: the EQ preset changed.

<a id="reads"></a>
## 9. Reads

<a id="battery"></a>
### 9.1 Battery: `0x0106`

This command is always allowed. Battery reading is confirmed on a T310 (*t310*); the byte layout is *buds4*.

- **Request:** no payload.
- **Reply:** `00 <count> (<part> <raw>)…`.
  - `part` is `01` left, `02` right or `03` case.
  - `level = raw & 0x7F` (percent). `charging = raw & 0x80`.
  - A level of 0 means "not known", so leave that part out.
- **Example:** `00 03 01 64 02 64 03 50` → left 100 %, right 100 %, case 80 %.
- **No list.** A count of 0, or pairs that run past the end, mean no list.
- **Lid closed.** The case, and a bud inside it, report only while the lid is open (*buds4*).

<a id="firmware"></a>
### 9.2 Firmware: `0x0105`

Evidence: *buds4*

- **Reply:** `00 <count> <UTF-8 text>`. The text is a comma-separated list of `device,kind,version` triples, where device `1` is left, `2` right and `3` case.
- **Display.** Show the versions of kind `2`, in reply order, joined with dots. HeyMelody shows them this way. If there are none, show the raw text.
- **Example:** `1,2,138,2,2,138,3,1,01,3,2,105` → `138.138.105`.
- **On the T310** (*ours-2026-10-27*): `00 02` then `1,2,1.1.0.43,2,2,1.1.0.43`. Each version is already a full dotted version, so joining them with dots would be misleading. When any version contains a dot, budctl shows each distinct version once, joined with ` / `: here `1.1.0.43`.

<a id="codec"></a>
### 9.3 Codec in use: `0x0114`

Evidence: *buds4*

- **Reply:** `00 <codec>`.
- **Codec values:** `01` SBC, `02` AAC, `03` LDAC, `04` aptX, `05` aptX HD, `06` aptX Adaptive, `07` LHDC, `08` LHDC V5.
- The T310 offers AAC and SBC (*vendor*).

<a id="anc-read"></a>
### 9.4 Noise control: `0x010C 01 01`

- **Reply:** `00 01 01 <mask LE>`. The mask is 1-4 bytes; the T310's width is unknown, so read every byte that is there.
- **Values.** On realme buds the value read back uses the same bits as the write (*vendor*; tested on the Air7 Pro only):

  | Mask | T310 mode |
  |---|---|
  | `01` | off |
  | `02` | transparency |
  | `04` | noise cancelling, light |
  | `08` | noise cancelling, deep. Also plain "noise cancelling on": both are the same value |
  | `10` | noise cancelling, medium |

- **No Smart.** The T310 has no Smart level (`20`).
- **Decoding `08`.** budctl shows `08` as noise cancelling with level *deep*. This still needs a hardware check (see [open questions](#open)).
- Noise control as a whole is confirmed on a T310 (*t310*). The bit values above are *vendor*.

<a id="switches"></a>
### 9.5 Switches: `0x010D`

- **Request:** `<count> <ids…>`. budctl asks only for the T310's ids: `05 06 11 1A 1B 1D`.
  - `05 06 11 1B 1D` is the vendor app's query list for the T310 (*vendor*).
  - `1A` is added because the model has the wind-noise row (`windMode`).
- **Reply:** `00 <count> (<id> <value>)…`. A value of `01` means on, `00` off.

| Id | Switch | Evidence |
|---|---|---|
| `05` | unknown. The vendor apps always ask it first and ignore the value | *vendor* |
| <a id="switch-game"></a>`06` | game mode ("low latency") | confirmed on a T310, *t310* |
| <a id="switch-dual"></a>`11` | dual connection | seen in a T310 log, *t310* |
| <a id="switch-wind"></a>`1A` | wind-noise reduction | realme Link shows it for the T310 (*vendor*). The T310 answers this id (*ours-2026-10-27*) |
| <a id="switch-spatial"></a>`1B` | 3D sound | seen in a T310 log, *t310* |
| <a id="switch-bass"></a>`1D` | bass boost ("BassWave", "dynamic sound") | seen in a T310 log, *t310* |

- **The T310 answers every id it is asked, even ids it lacks** (a T310 log: all 23 ids came back). A switch being in the reply therefore proves nothing.
  - Writing ids `04 0B 18 27 35` got status `01`, "the buds lack this id" (*t310*).
  - That is why budctl asks only for the ids above.

<a id="eq-read"></a>
### 9.6 EQ preset: `0x010F`

Preset names are the vendor app's (*vendor*).

- **Reply:** `00 <id>`.
- **Built-in presets** on realme buds (*vendor*, unverified on the T310):

  | id | Preset |
  |---|---|
  | `00` | Pure raw |
  | `01` | Deep bass |
  | `02` | Pure vocals |
  | `03` | Pure bass |

- **Any other id** is a custom preset. On Buds 4, custom ids start at `04` (*buds4*).

<a id="bass-read"></a>
### 9.7 Bass level: `0x0124`

Evidence: *vendor*; the T310's command set lists it.

- **Reply:** `00 <min s8> <max s8> <level s8>`. The usual range bytes are `FB 05`, meaning -5..+5.
- **The T310 does not answer it** (*ours-2026-10-27*): an empty `0x0124` got no reply within 3 s, although bit 38 is set. budctl no longer sends it. Bass level returns at W6, from an Android HCI snoop of realme Link reading it.

<a id="gestures"></a>
### 9.8 Touch controls: `0x0108`

- **Reply:** `00 <count> (<dev> <btn> <act> <fn>)…`. Each entry is 4 bytes.
- **`dev`:** `01` left, `02` right, `04` both (on-call rows).
- **`btn`:** `01` main gestures, `06` the on-call group.
- **`act`:** `01` single tap, `02` double tap, `03` triple tap, `04` hold, `05` slide, `06` long hold.
- **`fn` values** (measured on *buds4*; realme uses the same values per *vendor*):

  | `fn` | Function |
  |---|---|
  | `00` | none |
  | `01` | play/pause |
  | `03` | voice assistant |
  | `05` | previous track |
  | `06` | next track |
  | `07` | volume (slide) |
  | `08` | noise-control cycle |
  | `09` | favourite music |
  | `0A` | switch track (slide) |
  | `0B` | volume up |
  | `0C` | volume down |
  | `0D` | switch devices |
  | `11` | game mode |
  | `1C` | decline call |
  | `1D` | answer/end call |

- **What realme Link offers for the T310** (*vendor*). Each bud is set on its own, as `dev` 01/02 with `btn` 01:

  | Gesture | Choices |
  |---|---|
  | double tap | none, play/pause, next, voice assistant |
  | triple tap | none, previous, next, voice assistant, volume up, volume down, game mode |
  | hold | none, noise-control cycle, voice assistant, game mode |

- **Table shape.** No T310 gesture table has ever been captured. Its shape is unknown, so budctl shows every entry it receives.

<a id="hold-cycle"></a>
### 9.9 Hold noise-control cycle: `0x010C 02 01`

Evidence: *vendor*

- **Reply:** `00 02 01 <mask LE>`. The mask lists which modes a hold steps through, using the noise-control bits: `01` off, `02` transparency, `08` noise cancelling.
- **Both buds.** On realme buds one setting covers both buds (kind `01`).
- **When it applies.** The cycle is used only where a bud's hold gesture is `fn 08`.

<a id="writes"></a>
## 10. Writes

**One at a time.** budctl adds a write only after the previous one passes its hardware checkpoint (see the [roadmap](#roadmap)). Sent so far: [§10.1](#anc-write) off/transparency/on (W1, verified *ours-2026-10-27*), the light/medium/deep levels (W2, verified *ours-2026-10-27*), and [§10.2](#switch-write) game mode (W3, verified *ours-2026-10-27*), and [§10.3](#eq-write) the built-in EQ presets (W4, verified *ours-2026-10-27*), the bass-boost switch `1D` (W5a, verified *ours-2026-10-27*), 3D sound `1B` (W5b, verified *ours-2026-10-27*), wind-noise reduction `1A` (W5c, verified *ours-2026-10-27*), and dual connection `11` (W5d, verified *ours-2026-10-27*). budctl reads it back at once, because the spec gives it no special timing. The others are not sent yet.

**General rule**:
- A write reply is `0x84xx <status …>`, and `00` means accepted.
- Accepted is not the same as applied. **Always read the state back** and compare it.
- <a id="write-read-back"></a>**How budctl reads back** (budctl's own policy, not a protocol fact): it reads once right after the ack. If the old value is still there, it reads again every 300 ms for up to 2 s. If the value has still not changed, the command fails with exit 4 ("accepted but not applied"). A write that has its own timing, such as game mode ([§10.2](#switch-write)), says so in its section.

<a id="anc-write"></a>
### 10.1 Noise control: `0x0404 01 01 <mask>`

- **Payload:** one bit set, in the fewest bytes, little endian. The T310 masks are the same as for [§9.4](#anc-read): off `01`, transparency `02`, light `04`, deep or "on" `08`, medium `10`.
- **Reply:** `0x8404 <status>`.
  - Status `0E` means no bud is in an ear (*buds4*). The T310 has no in-ear sensor (*t310*), so it may never send it.
- **Check:** read `0x010C 01 01` again.
- **`anc on`.** budctl writes `08` (deep), because no "on at the saved level" value is known, and budctl keeps no device state of its own. The buds do remember the level (*ours-2026-10-27*, W2): after `anc light`, `anc transparency` and a hold gesture, noise cancelling came back as light (`04`). How an app turns noise cancelling on at the saved level is unknown. Writing `08` forces deep, and the saved level cannot be read while noise cancelling is off. Finding out needs an Android HCI snoop of realme Link turning noise cancelling on after a level was chosen ([open questions](#open)).
- **Levels on the T310** (*ours-2026-10-27*, W2): `01 01 04` (light), `01 01 10` (medium) and `01 01 08` (deep) were each acked with `00` within 15-30 ms and read back on the first try.
- **On the T310** (*ours-2026-10-27*, W1): payloads `01 01 01`, `01 01 02` and `01 01 08` each got `0x8404 00` within 20-80 ms. The first read-back, about 200 ms later, already showed the new mask. The buds played their own voice prompts ("Normal Mode", "Transparent Mode", "Noise Cancelling"). They sent no event-`03` push for the change, although it was subscribed.

<a id="switch-write"></a>
### 10.2 Switches: `0x0403 <id> <01|00>`

- **Reply:** `0x8403 <status>`:
  - `00` accepted
  - `01` the buds lack this id
  - `02` or anything else: refused for now
- **Check:** read `0x010D` again.
- **Game mode on the T310.** The buds never push the new value (*t310*). On Buds 4 the change lands about 1 s after the ack, and a read sooner still returns the old value (*buds4*). So budctl reads back 1 s after the ack, then every 300 ms until 3 s after the ack.
- **Bass boost on the owner's T310** (*ours-2026-10-27*, W5a): `0x0403 1D 01` and `1D 00` were acked with `00` within 30 ms. The immediate read-back already showed the new value, and no push came. With the EQ on pure-bass, the owner heard no clear difference.
- **3D sound on the owner's T310** (*ours-2026-10-27*, W5b): `0x0403 1B 01` and `1B 00` were acked with `00` within 30 ms. The immediate read-back showed the new value. The owner heard a difference but could not name it.
- **Wind-noise reduction on the owner's T310** (*ours-2026-10-27*, W5c): `0x0403 1A 00` and `1A 01` were acked with `00` within 35 ms. The immediate read-back showed the new value. It is not audible in music.
- **Dual connection on the owner's T310** (*ours-2026-10-27*, W5d): `0x0403 11 01` and `11 00` were acked with `00` within 50 ms. The immediate read-back showed the new value. The laptop's link stayed up both ways. Only one host was connected (the phone's Bluetooth was off), so turning it off with two hosts connected is still untested.
- **Game mode on the owner's T310** (*ours-2026-10-27*, W3): `0x0403 06 01` and `06 00` were acked with `00` within 13 ms. The read 1 s later already showed the new value. No push came, and the buds played no prompt.
- **Dual connection.** Other clients also send `0x0413` afterwards. The T310's bitmap lacks it, so budctl sends only `0x0403 11`. Turning dual connection off may drop the link to one host, so it is tested last.

<a id="eq-write"></a>
### 10.3 EQ preset: `0x0406 <id>`

Evidence: *vendor*

- **Reply:** `0x8406 <status>`.
- **Check:** read `0x010F` again.
- **On the owner's T310** (*ours-2026-10-27*, W4): ids `00`-`03` were each acked with `00` within 20-40 ms and read back on the first try. The owner heard a clear difference between them. The names come from the vendor app; the owner did not check by ear that each sound matched its name.

<a id="bass-write"></a>
### 10.4 Bass level: `0x041B FB 05 <level s8>`

Evidence: *vendor*

- **Range:** -5..+5.
- **Check:** read `0x0124` again.

<a id="find"></a>
### 10.5 Find my earbuds: `0x0400 01` / `0x0400 00`

Evidence: *buds4*

- `01` starts ringing both buds and `00` stops it. The reply is `0x8400 00`.
- **It is loud**, so budctl warns before ringing.
- Only the ack can be checked: there is no state to read back.

<a id="gesture-write"></a>
### 10.6 Touch controls: `0x0401 <count> (<dev> <btn> <act> <fn>)…`

The command and layout are *buds4*; the realme rules are *vendor*.

- **Whole table.** The write carries the whole table. Start from a table read in the same session and change only the targeted entries.
- **Reply:** `0x8401 00`.
- **Check:** read `0x0108` again after about 600 ms and compare.
- **Wrong command ids fail silently.** For example, `0x0402` gets no reply at all.

<a id="hold-cycle-write"></a>
### 10.7 Hold cycle: `0x0404 02 01 <mask>`

Evidence: *vendor*

- **Mask:** the fewest bytes that hold it, and at least 2 modes (*vendor*).
- **Level bits.** The buds silently drop level bits (*buds4*).
- **Check:** read `0x010C 02 01`.

<a id="fit-test"></a>
### 10.8 Fit test: `0x0405 01` / `0x0405 00`

Evidence: *buds4*

- `01` starts the test and `00` stops it.
- **Result:** arrives as push `04 (<dev> <result>)…`, where `1` = good, `0` = average and `6` = poor. The push needs event `04` subscribed.
- **Timing:** about 8.6 s on Buds 4. Both buds must be in the ears.

<a id="custom-eq"></a>
### 10.9 Custom EQ: list `0x0122`, write `0x0418`

Evidence: *vendor*

- **List reply:** `00 <count>` then, per preset: `<selected> <min> <max> <id> <nameLen> <name> <bands> (<freq u16 LE> <gain s8>)…`.
- **Write:** `0x0418 <action> <min> <max> <id> <nameLen> <name> <bands> (<freq> <gain>)…`. The reply is `0x8418 00 <id>`.
  - `01` creates a preset; send id `00` and the buds assign one.
  - `02` saves the whole preset and selects it.
  - `03` deletes it.
- **Ids are renumbered** after a create or delete, so read the list again each time.
- **Defaults:** 6 bands at 62, 250, 1000, 4000, 8000 and 16000 Hz. Range bytes `FA 06` mean -6..+6 dB. At most 3 presets.

<a id="absent"></a>
## 11. Not on the T310

- **Wear detection.** Id `04` was refused with `01` (*t310*), and there is no in-ear sensor. A bud out of the case reads as "out" in every state.
- **Hi-Res** `18`, **hearing profile** `0B`, **game sound** `27`, **hold volume** `35`: refused with `01` (*t310*).
- **Not in the T310 bitmap** ([§5](#bitmap-t310)): alert volume, tap sensitivity, spatial type, 3D-audio source, game EQ, head gestures, personalised ANC and batch query.

<a id="open"></a>
## 12. Open questions (need a capture or a test on the owner's buds)

1. ~~Which service UUID the T310 offers, and its RFCOMM channel.~~ Answered at HW1: `0000079a-d102-11e1-9b23-00025b00a5a5` on channel 6.
2. ~~The real bitmap bytes, the product-id reply and the offered events.~~ Answered at HW1; see [§5](#bitmap-t310) and [§6](#events-offered).
3. Partly answered at HW1: no `0x0500`, but an unanswered `0x0501` with no reset ([§7](#buds-requests)). `F2`/`F4` were offered but not subscribed, so none arrived. What `0x0501` asks for is unknown.
4. What "busy" looks like when the phone holds the channel: the connect is refused, the connect succeeds and the buds stay silent, or the buds drop the laptop's link. A local holder gives `EBUSY` (HW1). Until the phone case is tested, budctl also treats a silent handshake as "channel busy" (exit 3).
5. The width of the noise-control value: one byte so far, on reads and writes. `08` is the deep level: light, medium and deep read back as `04`, `10` and `08` (W2).
6. ~~Whether the T310 answers id `1A` (wind).~~ It does (HW1). What id `05` means is still open: it reads `00`.
7. How long after the game-mode ack the new value shows up in `0x010D`: at most 1 s on the T310 (W3). Whether it is sooner was not measured.
8. The shape of the T310 gesture table is answered at HW1: six entries, `<side> <group> <act> <fn>`, double tap, triple tap and hold for each bud. The names of the EQ presets in realme Link are still open.
9. Whether the minimum 200 ms spacing between sends is needed.
10. Why the T310 ignores `0x0124` although bit 38 is set ([§9.7](#bass-read)). Needs a realme Link capture before W6.
11. How to turn noise cancelling on at the level the buds saved, without forcing deep ([§10.1](#anc-write)). Needs a realme Link snoop.

<a id="roadmap"></a>
## 13. Roadmap and hardware checkpoints

The owner runs each checkpoint with `--debug` and pastes the output. Results go into the [verification log](#verification-log).

**HW1, read-only:**
- Run `budctl list`, `budctl --debug status`, `budctl --json status` and `budctl --debug touch`.
- Then run `status` again twice: once while the phone app is connected (the busy path), and once with the case lid closed (the not-connected path).

**Writes, one per checkpoint, in this order:**

| Step | Write |
|---|---|
| W1 | `anc off/transparency/on` |
| W2 | `anc light/medium/deep` |
| W3 | `game on/off` |
| W4 | `eq <preset>` |
| W5 | `set bass-boost` → `set spatial` → `set wind` → `set dual`, each one checkpoint |
| W6 | `bass <level>` |
| W7 | `find` |
| W8 | `touch <side> <gesture> <action>` (`--dry-run` first) |
| W9 | `touch cycle` |
| W10 | `fit-test` |
| W11 | custom EQ (create, then save, then delete) |

Ask for an Android HCI snoop of realme Link before W11, and for any write that does nothing.

<a id="verification-log"></a>
## 14. Verification log

| Date | Checkpoint | Result | Notes |
|---|---|---|---|
| 2026-10-27 | W5d `set dual on`, `off` | pass | Acked, the immediate read-back showed the change, and the laptop stayed connected. Only one host was connected. See [§10.2](#switch-write) |
| 2026-10-27 | W5c `set wind off`, `on` | pass (protocol) | Acked, and the immediate read-back showed the change. Not audible in music. See [§10.2](#switch-write) |
| 2026-10-27 | W5b `set spatial on`, `off` | pass | Acked, the immediate read-back showed the change, and the owner heard a difference. See [§10.2](#switch-write) |
| 2026-10-27 | W5a `set bass-boost on`, `off` | pass (protocol) | Acked, and the immediate read-back showed the change. The owner heard no clear difference with EQ pure-bass. See [§10.2](#switch-write) |
| 2026-10-27 | W4 `eq pure-raw`, `deep-bass`, `pure-vocals`, `pure-bass` | pass | Acked and read back on the first try; the presets sound clearly different. See [§10.3](#eq-write) |
| 2026-10-27 | W3 `game on`, `game off` | pass | Acked, and the 1 s read-back showed the change. No push or prompt. See [§10.2](#switch-write) |
| 2026-10-27 | W2 `anc light`, `anc medium`, `anc deep`; level memory | pass | Each level acked and read back on the first try. The buds keep the level across a hold-gesture cycle. See [§10.1](#anc-write) |
| 2026-10-27 | W1 `anc transparency`, `anc off`, `anc on` | pass | Acked, read back on the first try, and the buds' voice prompts matched. See [§10.1](#anc-write) |
| 2026-10-27 | HW1 `list`, `status`, `--json status`, `touch`, not-connected path, busy path (local holder) | pass, with fixes | See below. Busy with the phone app as the holder is still untested |

**HW1, 2026-10-27** (budctl 0.1.0, `--debug` output pasted by the owner):
- `list` finds `realme Buds T310` at `88:0E:85:46:FE:5C`, connected, recognized by `Name`.
- SDP: service `0000079a-d102-11e1-9b23-00025b00a5a5` on RFCOMM channel 6.
- Handshake: bitmap `00 BF 1F 5A 20 45 04 40 00`, product id `00 12 3C 06` (`063C12`), events offered `01 02 03 04 05 06 0D F1 F4`. The subscribe `04 01 02 03 04` is acked.
- Reads that answered, with what budctl showed:
  - battery `00 03 02 64 01 64 03 00`: left and right 100 %, case unknown;
  - noise control `00 01 01 08`: anc, deep;
  - switches `00 06 05 00 06 00 11 01 1A 01 1B 00 1D 00`: game off, dual on, wind on, 3D off, bass boost off;
  - EQ `00 03`: pure-bass;
  - codec `00 01`: SBC;
  - firmware `1,2,1.1.0.43,2,2,1.1.0.43`;
  - gestures: both buds double tap play/pause, triple tap next, hold noise cycle;
  - hold cycle `00 02 01 0A`: transparency and anc.
- Every reply came 50-90 ms after its request.
- `0x0124` (bass level) got no reply in 3 s.
- Lid closed: `status` reports "paired but not connected" and exits 2.
- Busy path, first try: with the phone app connected, BlueZ listed the buds as not connected to the laptop, so budctl stopped before RFCOMM (exit 2). This is inconclusive: the laptop's link was down, so the control channel was never tried.
- Busy path, local variant: a second socket on the laptop connected to channel 6 and held it, sending nothing. budctl's connect then failed with `EBUSY` (os error 16), and budctl reported "control channel busy" and exited 3. budctl sent nothing to the buds. This shows that a local client holding the channel gives exit 3. When the phone holds it, the buds decide what happens, and that is still untested.
- Fixes made from this checkpoint:
  - the parser accepts slice byte `08` ([§3](#frame-from-buds)); before, it silently dropped every frame the buds start;
  - `0x0124` is no longer sent;
  - firmware display ([§9.2](#firmware)).
- Still to confirm by the owner against the phone app: the noise-control mode and level, the EQ preset, the dual and wind switches, the gestures and the hold cycle.
