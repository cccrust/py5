import os
import json
from http.server import HTTPServer, BaseHTTPRequestHandler

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
STORAGE_DIR = os.path.join(SCRIPT_DIR, "storage")
INDEX_FILE = os.path.join(SCRIPT_DIR, "index.json")

def ensure_storage():
    if not os.path.exists(STORAGE_DIR):
        os.system("mkdir -p " + STORAGE_DIR)

def load_index():
    if os.path.exists(INDEX_FILE):
        with open(INDEX_FILE) as f:
            return json.loads(f.read())
    return {}

def save_index(index):
    with open(INDEX_FILE, "w") as f:
        f.write(json.dumps(index))

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/" or self.path == "/list":
            index = load_index()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(index).encode())
        elif self.path.startswith("/download/"):
            name = self.path[10:]
            pkg_file = STORAGE_DIR + "/" + name + ".py"
            if os.path.exists(pkg_file):
                with open(pkg_file) as f:
                    src = f.read()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({"source": src}).encode())
            else:
                self.send_response(404)
                self.end_headers()
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        if self.path == "/upload":
            length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(length).decode()
            print("[pip-server] Upload request, body length:", len(body))
            try:
                data = json.loads(body)
                name = data.get("name")
                source = data.get("source")
                print("[pip-server] Saving to:", STORAGE_DIR + "/" + name + ".py")
                ensure_storage()
                out_file = open(STORAGE_DIR + "/" + name + ".py", "w")
                out_file.write(source)
                out_file.close()
                index = load_index()
                index[name] = data.get("meta", {})
                save_index(index)
                print("[pip-server] Saved successfully")
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({"status": "ok"}).encode())
            except Exception as e:
                print("[pip-server] Error:", str(e))
                self.send_response(400)
                self.end_headers()
        else:
            self.send_response(404)
            self.end_headers()

    def do_DELETE(self):
        self.send_response(404)
        self.end_headers()

    def log_message(self, format, *args):
        print("[pip-server] " + format % args)

def main():
    ensure_storage()
    port = 8080
    print("Starting pip server on http://localhost:" + str(port))
    server = HTTPServer(("localhost", port), Handler)
    server.serve_forever()

if __name__ == "__main__":
    main()