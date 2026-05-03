#!/bin/bash
cd "$(dirname "$0")"

echo "=== Starting pip server ==="
python3 py/pip/server.py &
SERVER_PID=$!
sleep 2

echo "=== Testing upload ==="
./py5 pip upload mypackage

echo "=== Testing list ==="
./py5 pip list

echo "=== Testing install ==="
./py5 pip install mypackage

echo "=== Testing list after install ==="
./py5 pip list

echo "=== Testing run installed package ==="
./py5 run py/test_installed_pkg.py

echo "=== Cleaning up ==="
kill $SERVER_PID 2>/dev/null

echo "=== Done ==="