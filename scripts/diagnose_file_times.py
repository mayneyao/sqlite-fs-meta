"""Disposable-file diagnostics; no timestamp assumptions or production writes."""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import tempfile
import time


NODE_PROBE = r"""
const fs = require('node:fs');
const {DatabaseSync} = require('node:sqlite');
const [extension, database, file, sql] = process.argv.slice(1);
const db = new DatabaseSync(database, {allowExtension:true});
db.loadExtension(extension);
try {
  if(sql) db.exec(sql);
  const row=db.prepare('SELECT _created_at, _updated_at, mtime, rating FROM files WHERE _id=?').get('probe.txt');
  const stat=fs.statSync(file,{bigint:true});
  console.log(JSON.stringify({row,node:Object.fromEntries(['birthtimeNs','mtimeNs','ctimeNs','atimeNs','ino','size'].map(k=>[k,String(stat[k])]))}));
} finally { db.close(); }
"""


def native_times(file):
    if os.name != 'nt':
        stat = file.stat()
        return {'mtime_ns': str(stat.st_mtime_ns), 'ctime_ns': str(stat.st_ctime_ns),
                'birthtime_ns': str(getattr(stat, 'st_birthtime_ns', 'unavailable'))}
    from ctypes import wintypes

    class BasicInfo(ctypes.Structure):
        _fields_ = [('CreationTime', ctypes.c_longlong),
                    ('LastAccessTime', ctypes.c_longlong),
                    ('LastWriteTime', ctypes.c_longlong),
                    ('ChangeTime', ctypes.c_longlong),
                    ('FileAttributes', wintypes.DWORD)]

    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    kernel.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD,
                                  ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE]
    kernel.CreateFileW.restype = wintypes.HANDLE
    kernel.GetFileInformationByHandleEx.argtypes = [wintypes.HANDLE, ctypes.c_int,
                                                   ctypes.c_void_p, wintypes.DWORD]
    kernel.GetFileInformationByHandleEx.restype = wintypes.BOOL
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.CloseHandle.restype = wintypes.BOOL
    handle = kernel.CreateFileW(str(file), 0, 7, None, 3, 0, None)
    if handle == ctypes.c_void_p(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        info = BasicInfo()
        if not kernel.GetFileInformationByHandleEx(handle, 0, ctypes.byref(info), ctypes.sizeof(info)):
            raise ctypes.WinError(ctypes.get_last_error())
        return {name: str(getattr(info, name)) for name, _ in info._fields_}
    finally:
        kernel.CloseHandle(handle)


def raw_write(file, value):
    if os.name == 'nt':
        Path(str(file) + ':space.eidos.meta').write_bytes(value)
    elif platform.system() == 'Darwin':
        subprocess.run(['xattr', '-w', 'space.eidos.meta', value.decode(), str(file)], check=True)
    else:
        os.setxattr(file, 'user.space.eidos.meta', value)


def raw_remove(file):
    if os.name == 'nt':
        Path(str(file) + ':space.eidos.meta').unlink()
    elif platform.system() == 'Darwin':
        subprocess.run(['xattr', '-d', 'space.eidos.meta', str(file)], check=True)
    else:
        os.removexattr(file, 'user.space.eidos.meta')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--extension', required=True)
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    extension = Path(args.extension).resolve()
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    report = {'platform': platform.platform(), 'python': platform.python_version(),
              'node': subprocess.check_output(['node', '--version'], text=True).strip(),
              'extension_sha256': hashlib.sha256(extension.read_bytes()).hexdigest(),
              'release': 'v0.2.3', 'steps': []}
    try:
        with tempfile.TemporaryDirectory(prefix='fs-meta-times-') as temporary:
            root = Path(temporary)
            file = root / 'probe.txt'
            original = b'File content must remain unchanged.\n'
            file.write_bytes(original)
            # Deliberately distinguish true birth time from content modification time.
            os.utime(file, (946684800, 946684800))
            database = root / '.diagnostic.sqlite'
            setup = "CREATE VIRTUAL TABLE files USING fs_meta(root='.', namespace='space.eidos.meta', fields='rating INTEGER');"
            previous = None

            def record(name, sql='', raw=None, expected=None):
                nonlocal previous
                if previous is not None:
                    time.sleep(1.1)
                if raw:
                    raw()
                value = json.loads(subprocess.check_output(
                    ['node', '-e', NODE_PROBE, str(extension), str(database), str(file), sql], text=True))
                value['native'] = native_times(file)
                value['name'] = name
                value['content_sha256'] = hashlib.sha256(file.read_bytes()).hexdigest()
                assert file.read_bytes() == original, 'Metadata operation changed main stream'
                assert value['row']['rating'] == expected, (name, value['row'], expected)
                value['changed'] = {} if previous is None else {
                    group: [key for key in value[group] if value[group][key] != previous[group][key]]
                    for group in ['native', 'node', 'row']}
                report['steps'].append(value)
                previous = value
                print(json.dumps(value), flush=True)

            record('baseline (mtime set to 2000; birth time remains real)', setup)
            record('SQL: add rating', 'UPDATE files SET rating=1;', expected=1)
            record('SQL: update rating', 'UPDATE files SET rating=2;', expected=2)
            record('SQL: write same value', 'UPDATE files SET rating=2;', expected=2)
            record('SQL: clear field', 'UPDATE files SET rating=NULL;')
            record('SQL: add rating again', 'UPDATE files SET rating=3;', expected=3)
            record('SQL: rollback update', 'BEGIN; UPDATE files SET rating=4; ROLLBACK;', expected=3)
            record('SQL: remove key', "UPDATE files SET __fs_meta_remove_key='rating';")
            record('SQL: delete metadata envelope', 'DELETE FROM files;')
            record('Raw OS: create attribute/ADS', raw=lambda: raw_write(file, b'{"rating":5}'), expected=5)
            record('Raw OS: overwrite attribute/ADS', raw=lambda: raw_write(file, b'{"rating":6}'), expected=6)
            record('Raw OS: remove attribute/ADS', raw=lambda: raw_remove(file))
            record('SQL: rollback first attribute creation', 'BEGIN; UPDATE files SET rating=7; ROLLBACK;')
        report['ok'] = True
    except Exception as error:
        report['error'] = repr(error)
        raise
    finally:
        (output / 'timestamps.json').write_text(json.dumps(report, indent=2), encoding='utf8')
        lines = ['# File timestamp observations', '', f"Platform: {report['platform']}",
                 '', '| Operation | Native changes | Node changes | Virtual table changes |',
                 '| --- | --- | --- | --- |']
        for step in report['steps']:
            changes = step['changed']
            lines.append('| ' + ' | '.join([step['name']] + [', '.join(changes.get(group, [])) or 'none'
                         for group in ['native', 'node', 'row']]) + ' |')
        lines += ['', 'Raw timestamps and values are recorded in timestamps.json.',
                  'A successful run means observations and metadata/content checks completed; it does not mean timestamps stayed unchanged.']
        summary = '\n'.join(lines) + '\n'
        (output / 'summary.md').write_text(summary, encoding='utf8')
        if os.environ.get('GITHUB_STEP_SUMMARY'):
            with open(os.environ['GITHUB_STEP_SUMMARY'], 'a', encoding='utf8') as target:
                target.write(summary)


if __name__ == '__main__':
    main()
