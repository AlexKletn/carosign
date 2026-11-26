# Reference tables

Numbers, addresses and names — no narrative. Everything is ✅BIN unless marked
otherwise.

| document | subject |
|---|---|
| [commands.md](commands.md) | HAL commands: numbers, bodies, senders |
| [names.md](names.md) | modes, operations, navigation, errors, chips |
| [weights.md](weights.md) | weight symbols: addresses, sizes, GOT slots |
| [addresses.md](addresses.md) | `libgf_hal` function addresses and how to read them |
| [service-contracts.md](service-contracts.md) | FS, GPFS, RPMB contracts |
| [kernel-and-daemon.md](kernel-and-daemon.md) | modules and daemon, briefly |
| [frame-header.md](frame-header.md) | command header, reply and event-body offsets |

⚠️ How to read addresses:

* `nm -D` gives **virtual** addresses; `objdump -d <file>
  --start-address=<address>` works with them;
* tables in `.data` hold **file offsets** as pointers; the string address is
  the value `+ 0x10000`;
* jump tables: `target = table_base + offset` (not `×4`).
