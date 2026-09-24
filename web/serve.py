#!/usr/bin/env python3
import http.server, os, sys, urllib.request

DEM = "https://copernicus-dem-30m.s3.amazonaws.com"
ROOT = os.path.dirname(os.path.abspath(__file__))


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm", ".js": "text/javascript"}

    def __init__(self, *a, **k):
        super().__init__(*a, directory=ROOT, **k)

    def end_headers(self):
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        super().end_headers()

    def do_GET(self):
        if not self.path.startswith("/dem/"):
            return super().do_GET()
        try:
            with urllib.request.urlopen(DEM + self.path[4:]) as r:
                body = r.read()
                self.send_response(200)
                self.send_header("Content-Type", "image/tiff")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
        except urllib.error.HTTPError as e:
            self.send_error(e.code)


port = int(sys.argv[1]) if len(sys.argv) > 1 else 8080
http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
