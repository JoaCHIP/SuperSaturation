#!/usr/bin/env python3
"""Talks to a PhotoCraft started with tools/launch-photocraft.sh (its JSON-lines control channel).

    tools/pc.py install                  install supersaturation.wasm into the running app
    tools/pc.py dialog                   open Filter › Plug-ins › SuperSaturation…
    tools/pc.py screenshot [FILE.png]    capture the main window (default target/photocraft-test/screen.png)
    tools/pc.py call METHOD [JSON]       any control method, e.g. call ui.inspect

Also a module: the other scripts here use Client, WORK and the helpers below.
"""
import base64, json, os, socket, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WORK = os.path.join(ROOT, "target", "photocraft-test")
WASM = os.path.join(ROOT, "supersaturation.wasm")
PLUGIN_ID = "io.github.joachip.supersaturation"


class Client:
    """One authenticated connection. Token authentication sends the token to whoever holds the
    port, so this is only for the loopback app that launch-photocraft.sh started."""

    def __init__(self, port=None, token_file=None):
        port = int(port or os.environ.get("PC_PORT", "7878"))
        token = open(token_file or os.path.join(WORK, "control.token")).read().strip()
        self.sock = socket.create_connection(("127.0.0.1", port), timeout=120)
        self.f = self.sock.makefile("rwb")
        self.n = 0
        r = self.raw("auth", {"token": token})
        if not r.get("ok"):
            raise SystemExit(f"auth failed: {r}")

    def raw(self, method, params=None):
        """The reply as is: {"id", "ok", "result" | "error"}."""
        self.n += 1
        self.f.write((json.dumps({"id": self.n, "method": method, "params": params or {}}) + "\n").encode())
        self.f.flush()
        while True:
            line = self.f.readline()
            if not line:
                raise SystemExit("PhotoCraft closed the connection")
            r = json.loads(line)
            if r.get("id") in (self.n, "auth") or method == "auth":
                return r

    def call(self, method, params=None):
        """The result; a failed call raises RuntimeError."""
        r = self.raw(method, params)
        if not r.get("ok"):
            raise RuntimeError(f"{method}: {r.get('error')}")
        return r.get("result")

    def execute(self, command, params=None):
        return self.call("engine.execute", {"command": command, "params": params or {}})

    def install(self, path=WASM):
        # plugin.install by path is refused over automation (ambient file access); bytes are fine.
        data = base64.b64encode(open(path, "rb").read()).decode()
        return self.execute("plugin.install", {"data": data})

    def run_filter(self, params):
        return self.execute("plugin.run", {"id": PLUGIN_ID, "params": params})

    def open(self, rel):
        """Opens a file by a path relative to the read roots (target/photocraft-test, then the project)."""
        return self.call("app.open", {"path": rel})

    def save(self, name):
        """Saves the active document under target/photocraft-test (format from the extension)."""
        path = os.path.join(WORK, name)
        if os.path.exists(path):
            os.remove(path)
        self.call("app.save", {"path": name})
        if not os.path.exists(path):
            raise RuntimeError(f"app.save did not write {path}")
        return path

    def screenshot(self, path):
        return self.call("ui.screenshot", {"path": os.path.relpath(os.path.abspath(path), WORK)})


def main(argv):
    if not argv:
        print(__doc__.strip())
        return 2
    c = Client()
    cmd, args = argv[0], argv[1:]
    if cmd == "install":
        m = c.install()
        print(f"installed {m['name']} {m['version']} by {m['author']} ({m['size']} bytes)")
    elif cmd == "dialog":
        print(c.call("ui.menu.invoke", {"id": "plugin.filter." + PLUGIN_ID}))
    elif cmd == "screenshot":
        path = args[0] if args else os.path.join(WORK, "screen.png")
        r = c.screenshot(path)
        print(f"{os.path.join(WORK, r['path']) if not os.path.isabs(r['path']) else r['path']} ({r['width']}×{r['height']})")
    elif cmd == "call":
        print(json.dumps(c.raw(args[0], json.loads(args[1]) if len(args) > 1 else {}), indent=1))
    else:
        print(__doc__.strip())
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
