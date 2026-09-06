"""Launch the packaged app and require its real React/WebKit readiness handshake."""
import json
import atexit
import os
from pathlib import Path
import subprocess
import sqlite3
import sys
import tempfile
import time
import uuid

binary = Path(sys.argv[1]).resolve()
if os.environ.get("GITHUB_ACTIONS") == "true":
    # Hosted runner images disable graphic effects by default. Exercise actual
    # blur in screenshots; never change accessibility preferences on a user's Mac.
    subprocess.run(["defaults", "write", "com.apple.universalaccess", "reduceTransparency", "-bool", "false"], check=True)
    fixture_binary = Path(tempfile.gettempdir()) / "clipmo-vibrancy-backdrop"
    subprocess.run(["clang", "-fobjc-arc", "-framework", "AppKit", "scripts/macos-vibrancy-backdrop.m", "-o", str(fixture_binary)], check=True)
    fixture = subprocess.Popen([str(fixture_binary)])
    atexit.register(fixture.terminate)
    time.sleep(1)
    if fixture.poll() is not None:
        raise RuntimeError("Vibrancy background fixture could not start")

for mode, args in [("main", []), ("quick", ["--show-quick"])]:
    with tempfile.TemporaryDirectory(prefix="clipmo-smoke-") as directory:
        ready = Path(directory) / "ready.json"
        env = dict(os.environ, CLIPDECK_READY_FILE=str(ready))
        with (Path(directory) / "app.log").open("w+") as log:
            app = subprocess.Popen([str(binary), *args], env=env, stdout=log, stderr=log)
            try:
                deadline = time.monotonic() + 90
                handshake = ready if mode == "main" else ready.with_suffix(".quick-focus.json")
                while time.monotonic() < deadline:
                    if app.poll() is not None:
                        raise RuntimeError(f"{mode} app exited early: {app.returncode}")
                    if handshake.exists():
                        try:
                            value = json.loads(handshake.read_text())
                        except json.JSONDecodeError:
                            time.sleep(0.2)
                            continue
                        assert value["processId"] == app.pid
                        if mode == "main":
                            assert all(value[key] for key in ["frontendReady", "windowCreated", "windowVisible", "searchVisible", "layoutVisible"])
                        else:
                            assert value["searchFocused"]
                        print(f"PASS: packaged {mode} window rendered and ready")
                        native_path = ready.with_suffix(f".vibrancy.{mode}.json")
                        native = json.loads(native_path.read_text())
                        assert native["processId"] == app.pid
                        assert not native["windowOpaque"], native
                        assert native["webviews"] and all(not view["opaque"] for view in native["webviews"]), native
                        assert len(native["effects"]) == 1, native
                        effect = native["effects"][0]
                        assert effect["material"] == (6 if mode == "quick" else 7), native
                        assert effect["state"] == 1 and effect["blendingMode"] == 0, native
                        print(f"PASS: packaged {mode} has active behind-window vibrancy and transparent WebKit")
                        if mode == "main" and os.environ.get("CLIPDECK_PDF_SMOKE_ID"):
                            pdf_path = ready.with_suffix(".pdf.json")
                            pdf_deadline = time.monotonic() + 45
                            while time.monotonic() < pdf_deadline:
                                if pdf_path.exists():
                                    try:
                                        pdf = json.loads(pdf_path.read_text())
                                    except json.JSONDecodeError:
                                        time.sleep(0.1)
                                        continue
                                    assert pdf["processId"] == app.pid, pdf
                                    result = pdf["result"]
                                    assert result["itemId"] == int(os.environ["CLIPDECK_PDF_SMOKE_ID"]), pdf
                                    assert result["success"] and result["ink"] > 0, pdf
                                    print(f"PASS: packaged file preview rendered {result['width']} x {result['height']} with {result['ink']} visible pixels")
                                    break
                                time.sleep(0.2)
                            else:
                                raise RuntimeError("Packaged PDF preview did not finish")
                        if os.environ.get("GITHUB_ACTIONS") == "true":
                            screenshots = Path("artifacts/smoke")
                            screenshots.mkdir(parents=True, exist_ok=True)
                            (screenshots / f"{mode}-vibrancy.json").write_text(json.dumps(native, indent=2))
                            time.sleep(2)
                            subprocess.run(["screencapture", "-x", str(screenshots / f"{mode}.png")], check=True)
                            if mode == "main":
                                # Hosted runner only: exercise the running listener and SQLite sink.
                                # Activate another app so Clipmo's own-source exclusion does not apply.
                                subprocess.run(["open", "-a", "TextEdit"], check=True)
                                time.sleep(3)
                                token = f"Clipmo macOS capture smoke {uuid.uuid4()}"
                                subprocess.run(["pbcopy"], input=token.encode(), check=True)
                                db = Path.home() / "Library/Application Support/app.clipdeck.desktop/clipdeck.db"
                                capture_deadline = time.monotonic() + 15
                                while time.monotonic() < capture_deadline:
                                    with sqlite3.connect(f"file:{db}?mode=ro", uri=True) as connection:
                                        captured = connection.execute("SELECT content FROM items WHERE content = ?", (token,)).fetchone()
                                    if captured:
                                        print("PASS: running macOS listener persisted the clipboard change")
                                        break
                                    time.sleep(0.25)
                                else:
                                    raise RuntimeError("running listener did not persist the clipboard change")
                        break
                    time.sleep(0.5)
                else:
                    raise RuntimeError(f"{mode} window did not report readiness")
            except Exception:
                log.seek(0)
                print(log.read(), file=sys.stderr)
                raise
            finally:
                app.terminate()
                try:
                    app.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    app.kill()
                    app.wait()
