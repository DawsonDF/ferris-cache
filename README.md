# ferris-cache

A minimal, in-memory key-value cache server written in Rust. No persistence, no replication, no modules, no configuration files. Just a fast cache over a plain TCP socket — deployed as a single static binary in a distroless container.

---

## Why ferris-cache?

Sometimes you don't need Redis. You need something that holds keys in memory, answers fast, and stays out of your way.

Redis is battle-hardened and feature-rich — and that's exactly the problem in some contexts. It brings a client ecosystem, a configuration surface, persistence semantics, and operational overhead you may not need or want. If your use case is "cache some values during the lifetime of a pod", that's a lot of ceremony.

ferris-cache is the answer to: **what is the absolute minimum viable cache?**

---

## What it is

- An in-memory key-value store over a raw TCP socket
- Four commands: `GET`, `SET`, `DELETE`, `FLUSH`
- Line-oriented plain text protocol — connect with `nc`, any language, no client library required
- Async I/O via Tokio — one lightweight task per connection, shared lock-free store via DashMap
- A single static binary (~400–600 KB) in a distroless image
- Zero runtime dependencies, zero configuration

---

## What it is not

- **Not persistent.** Data lives in memory. Pod restart means cache gone. This is intentional.
- **Not a Redis replacement.** No pub/sub, no sorted sets, no Lua scripting, no AUTH, no clustering.
- **Not production-hardened for untrusted networks.** There is no authentication. Run it inside your cluster, not on a public interface.
- **Minimal configuration.** One optional environment variable: `FERRIS_BIND`. No config files. Defaults to `0.0.0.0:7878`.

If you need any of those things, use Redis.

---

## Protocol

Plain text over TCP, one command per line, `\r\n` terminated. Multiple commands can be pipelined over a single connection.

| Command | Response |
|---|---|
| `SET <key> <value>` | `OK` (no expiry) |
| `SET <key> <value> <ttl_seconds>` | `OK` (expires after N seconds) |
| `SET <key> <value> 0` | `ERR TTL must be a positive integer` |
| `SET <key>` | `ERR wrong number of arguments` |
| `GET <key>` | `<value>` or `NIL` |
| `DELETE <key>` | `OK` or `NIL` |
| `FLUSH` | `OK` |
| anything else | `ERR unknown command` |

Values may contain spaces — everything after the key is treated as the value. When a TTL is provided, it must be the last token and a positive integer:

```
SET msg hello world      →  OK           (value: "hello world", no expiry)
SET msg hello world 60   →  OK           (value: "hello world", expires in 60s)
GET msg                  →  hello world
```

---

## Configuration

| Variable | Default | Description |
|---|---|---|
| `FERRIS_BIND` | `0.0.0.0:7878` | Address and port to listen on |

```bash
FERRIS_BIND=127.0.0.1:7878 cargo run        # local dev, loopback only
FERRIS_BIND=0.0.0.0:9000 ./ferris-cache     # custom port
docker run -e FERRIS_BIND=0.0.0.0:9000 -p 9000:9000 ferris-cache:latest
```

---

## Quickstart

**Run locally:**

```bash
cargo run
```

**Connect and test:**

```bash
printf "SET name ferris\r\nGET name\r\nDELETE name\r\nGET name\r\n" | nc 127.0.0.1 7878
# OK
# ferris
# OK
# NIL
```

**Flush the entire cache:**

```bash
printf "FLUSH\r\n" | nc 127.0.0.1 7878
# OK
```

---

## Docker

**Build:**

```bash
docker build -t ferris-cache:latest .
```

**Run:**

```bash
docker run -p 7878:7878 ferris-cache:latest
```

**Test:**

```bash
printf "SET docker works\r\nGET docker\r\n" | nc 127.0.0.1 7878
# OK
# works
```

The image uses a two-stage build: the binary is compiled against `musl` for a fully static executable, then copied into `gcr.io/distroless/static-debian12` — no shell, no package manager, no OS surface to attack.

---

## Connecting from your application

No client library needed. Open a TCP socket and write lines. Examples:

**Python:**

```python
import socket

def cache_cmd(cmd: str) -> str:
    with socket.create_connection(("127.0.0.1", 7878)) as s:
        s.sendall((cmd + "\r\n").encode())
        return s.recv(4096).decode().strip()

cache_cmd("SET user:1 alice")
print(cache_cmd("GET user:1"))  # alice
```

**Node.js:**

```js
const net = require("net");

function cacheCmd(cmd) {
  return new Promise((resolve) => {
    const client = net.createConnection(7878, "127.0.0.1", () => {
      client.write(cmd + "\r\n");
    });
    client.once("data", (data) => {
      resolve(data.toString().trim());
      client.destroy();
    });
  });
}

await cacheCmd("SET session:abc xyz");
console.log(await cacheCmd("GET session:abc")); // xyz
```

---

## Stack

| Concern | Choice | Reason |
|---|---|---|
| Async runtime | `tokio` | Industry standard, excellent performance |
| Concurrent store | `dashmap` | Lock-free HashMap, no `Mutex` contention |
| Runtime image | `distroless/static` | Minimal attack surface, tiny layer |
| Binary target | `musl` | Fully static, no glibc dependency |

---

## Development

```bash
cargo check      # fast compile check
cargo build      # debug build
cargo test       # run tests
cargo clippy     # lint
cargo fmt        # format
```

---

## License

MIT
