import { spawn } from 'node:child_process';

// Windows libuv adds USERPROFILE from the parent when it is absent in child env.
// Remove it for the synchronous spawn call, then restore this test process only.
// https://github.com/libuv/libuv/blob/v1.x/src/win/process.c (required_vars)
export function spawnIsolated(executable, args, options) {
  const saved = new Map(['USERPROFILE', 'HOME'].map(key => [key, process.env[key]]));
  try {
    for (const key of saved.keys()) delete process.env[key];
    return spawn(executable, args, options);
  } finally {
    for (const [key, value] of saved) {
      if (value !== undefined) process.env[key] = value;
    }
  }
}
