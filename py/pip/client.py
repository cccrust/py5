import os
import json
import sys

SERVER_URL = "http://localhost:8080"
STORAGE_DIR = "./py/pip/storage"
CACHE_FILE = "./py/pip/installed.json"

def exists(path):
    return os.system("test -e " + path + " && echo yes") == 0

def ensure_storage():
    if not exists(STORAGE_DIR):
        os.system("mkdir -p " + STORAGE_DIR)

def load_installed():
    if exists(CACHE_FILE):
        f = open(CACHE_FILE)
        data = f.read()
        f.close()
        return json.loads(data)
    return {}

def save_installed(installed):
    f = open(CACHE_FILE, "w")
    f.write(json.dumps(installed))
    f.close()

def cmd_list():
    installed = load_installed()
    keys = installed.keys()
    if len(keys) > 0:
        print("Installed packages:")
        for name in keys:
            info = installed[name]
            ver = ""
            ver_key = "version"
            if has_key(info, ver_key):
                ver = info[ver_key]
            print("  " + name + " " + ver)
    else:
        print("No packages installed")

def cmd_search(keyword):
    url = SERVER_URL + "/search?q=" + keyword
    cmd = "curl -s '" + url + "'"
    result = os.system(cmd)
    print("Search results from server")

def cmd_install(name):
    ensure_storage()
    url = SERVER_URL + "/download/" + name
    target = STORAGE_DIR + "/" + name + ".py"
    tmpfile = "/tmp/py5_pip_install.json"
    cmd = "curl -s -o " + tmpfile + " '" + url + "'"
    print("Installing " + name + "...")
    res = os.system(cmd)
    if res == 0:
        f = open(tmpfile)
        data = f.read()
        f.close()
        pkg_data = json.loads(data)
        src = pkg_data["source"]
        f = open(target, "w")
        f.write(src)
        f.close()
        installed = load_installed()
        installed[name] = {"version": "1.0.0", "path": target}
        save_installed(installed)
        print(name + " installed successfully")
    else:
        print("Failed to install " + name)

def cmd_upload(name):
    pkg_dir = "./packages/" + name
    if not exists(pkg_dir):
        print("Package not found: " + pkg_dir)
        return
    meta_file = pkg_dir + "/meta.json"
    if not exists(meta_file):
        print("Package must have meta.json")
        return
    f = open(meta_file)
    meta = json.loads(f.read())
    f.close()
    src_file = pkg_dir + "/" + name + ".py"
    if not exists(src_file):
        print("Package must have " + name + ".py")
        return
    f = open(src_file)
    src = f.read()
    f.close()
    data = json.dumps({"name": name, "meta": meta, "source": src})
    tmpfile = "/tmp/py5_pip_upload.json"
    f = open(tmpfile, "w")
    f.write(data)
    f.close()
    cmd = "curl -s -X POST -H 'Content-Type: application/json' --data-binary @" + tmpfile + " '" + SERVER_URL + "/upload'"
    res = os.system(cmd)
    if res == 0:
        print(name + " uploaded successfully")
    else:
        print("Failed to upload " + name)

def has_key(d, key):
    keys = d.keys()
    for k in keys:
        if k == key:
            return True
    return False

def cmd_remove(name):
    installed = load_installed()
    if has_key(installed, name):
        target = STORAGE_DIR + "/" + name + ".py"
        if os.path.exists(target):
            os.system("rm " + target)
        new_installed = {}
        keys = installed.keys()
        for k in keys:
            if k != name:
                new_installed[k] = installed[k]
        save_installed(new_installed)
        print(name + " removed")
    else:
        print("Package not installed: " + name)

def cmd_server():
    print("Starting pip server...")
    os.system("python3 ./py/pip/server.py")

def main():
    if len(sys.argv) < 2:
        print("Usage: py5 pip <command>")
        print("Commands: install, upload, list, search, remove, server")
        return
    cmd = sys.argv[1]
    if cmd == "list":
        cmd_list()
    elif cmd == "install":
        if len(sys.argv) < 3:
            print("Usage: py5 pip install <package>")
        else:
            cmd_install(sys.argv[2])
    elif cmd == "upload":
        if len(sys.argv) < 3:
            print("Usage: py5 pip upload <package>")
        else:
            cmd_upload(sys.argv[2])
    elif cmd == "search":
        if len(sys.argv) < 3:
            print("Usage: py5 pip search <keyword>")
        else:
            cmd_search(sys.argv[2])
    elif cmd == "remove":
        if len(sys.argv) < 3:
            print("Usage: py5 pip remove <package>")
        else:
            cmd_remove(sys.argv[2])
    elif cmd == "server":
        cmd_server()
    else:
        print("Unknown command: " + cmd)

main()