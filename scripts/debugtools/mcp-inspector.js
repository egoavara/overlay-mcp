const { spawn } = require('child_process');

const command = 'npx';
const args = ['@modelcontextprotocol/inspector', 'node', 'build/index.js'];

console.log(`Executing: ${command} ${args.join(' ')}`);

const inspectorProcess = spawn(command, args, { stdio: 'inherit' });

inspectorProcess.on('error', (error) => {
  console.error(`Failed to start subprocess: ${error.message}`);
});

inspectorProcess.on('exit', (code, signal) => {
  if (code !== null) {
    console.log(`Subprocess exited with code ${code}`);
  } else if (signal !== null) {
    console.log(`Subprocess was killed with signal ${signal}`);
  }
});
