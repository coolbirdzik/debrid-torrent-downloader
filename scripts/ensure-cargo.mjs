// A newly installed Rust toolchain may be missing from an already-open terminal's PATH.
// Repair only this process and its children, keeping an existing Cargo on PATH first.
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { delimiter, join } from 'node:path';

export function ensureCargoOnPath() {
  if (spawnSync('cargo', ['--version'], { stdio: 'ignore' }).error?.code !== 'ENOENT') return;
  const cargoBin = join(process.env.CARGO_HOME || join(homedir(), '.cargo'), 'bin');
  const cargo = join(cargoBin, process.platform === 'win32' ? 'cargo.exe' : 'cargo');
  if (existsSync(cargo)) {
    process.env.PATH = [cargoBin, process.env.PATH].filter(Boolean).join(delimiter);
  }
}
