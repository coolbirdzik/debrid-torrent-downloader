import { ensureCargoOnPath } from './ensure-cargo.mjs';

ensureCargoOnPath();

const { run } = await import('@tauri-apps/cli');
try {
  await run(process.argv.slice(2), 'pnpm tauri');
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
