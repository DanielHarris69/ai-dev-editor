"""Run from the repository root. Uses isolated data and never calls a provider."""
import json
import os
from pathlib import Path
import socket
import sqlite3
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

binary = Path('desktop/frontend/src-tauri/target/release/bundle/macos/Astra Dev Editor.app/Contents/MacOS/lpm-desktop').resolve()
original = Path('/usr/local/bin/lpm').readlink()
results = {}

def launch(data, log):
    return subprocess.Popen([str(binary)], env={**os.environ, 'LPM_DIR': str(data)}, stdout=log, stderr=log)

def stop(process):
    process.terminate()
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()

with tempfile.TemporaryDirectory(prefix='astra-native-') as root:
    root = Path(root)
    with (root / 'native.log').open('w') as log:
        (root / 'ready').mkdir()
        legacy = sqlite3.connect(root / 'ready' / 'api-spend.db')
        legacy.executescript("""CREATE TABLE api_spend (
            id INTEGER PRIMARY KEY, request_id TEXT NOT NULL, provider TEXT NOT NULL,
            model TEXT NOT NULL, input_tokens INTEGER NOT NULL, output_tokens INTEGER NOT NULL,
            cost REAL NOT NULL, timestamp TEXT NOT NULL, source TEXT NOT NULL,
            UNIQUE(source,request_id));
            INSERT INTO api_spend VALUES(1,'fixture-demo','vertex','fixture',1,1,0,'2026-10-15','demo');""")
        legacy.close()
        process = launch(root / 'ready', log)
        try:
            for _ in range(100):
                try:
                    health = json.load(urllib.request.urlopen('http://127.0.0.1:4000/health', timeout=1))
                    break
                except (urllib.error.URLError, ConnectionError):
                    if process.poll() is not None:
                        raise RuntimeError('App exited during startup')
                    time.sleep(0.1)
            else:
                raise RuntimeError('Proxy did not start')
            assert health['service'] == 'astra-dev-editor'
            connection = sqlite3.connect(root / 'ready' / 'api-spend.db')
            assert connection.execute('SELECT COUNT(*) FROM api_spend').fetchone()[0] == 0
            columns = [r[1] for r in connection.execute('PRAGMA table_info(api_spend)')]
            assert {'provider','model','input_tokens','output_tokens','cost','timestamp'} <= set(columns)
            request = urllib.request.Request('http://127.0.0.1:4000/v1/usage', data=b'{"request_id":"unauthorized","provider":"hermes","model":"test","input_tokens":1,"output_tokens":1,"cost":0.0}', headers={'Content-Type':'application/json'})
            try:
                urllib.request.urlopen(request)
                raise AssertionError('Unauthenticated write accepted')
            except urllib.error.HTTPError as error:
                assert error.code == 401
            connection.close()
            assert list((root / 'ready').glob('api-spend.pre-providers-*.sqlite3'))
            results['legacy_demo_upgrade_and_backup'] = 'passed'
            results['startup'] = 'passed'
            results['schema'] = 'passed'
            results['unauthenticated_write'] = 'rejected'
        finally:
            stop(process)
        with socket.socket() as occupied:
            occupied.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            occupied.bind(('127.0.0.1', 4000))
            occupied.listen(1)
            process = launch(root / 'busy', log)
            try:
                for _ in range(100):
                    if (root / 'busy' / 'api-spend.db').exists():
                        break
                    if process.poll() is not None:
                        raise RuntimeError('App exited with occupied proxy port')
                    time.sleep(0.1)
                else:
                    raise RuntimeError('Busy-port app did not initialize')
                time.sleep(1)
                assert process.poll() is None
                results['occupied_port_app_survives'] = 'passed'
            finally:
                stop(process)
assert Path('/usr/local/bin/lpm').readlink() == original
results['existing_lpm_cli_unchanged'] = True
print(json.dumps(results, indent=2))
