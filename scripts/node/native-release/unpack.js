'use strict';
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { spawnSync } = require('node:child_process');
function unpack(stage, destination) {
  stage = path.resolve(stage); destination = path.resolve(destination);
  if (fs.existsSync(destination)) throw new Error('Use a fresh archive verification directory');
  fs.mkdirSync(destination, { recursive: true });
  const archive = stage + (process.platform === 'win32' ? '.zip' : '.tar.gz');
  const result = process.platform === 'win32'
    ? spawnSync('powershell.exe',['-NoProfile','-NonInteractive','-Command','Add-Type -AssemblyName System.IO.Compression.FileSystem; [System.IO.Compression.ZipFile]::ExtractToDirectory($env:FLOWBASE_ARCHIVE, $env:FLOWBASE_EXTRACT)'],{env:{...process.env,FLOWBASE_ARCHIVE:archive,FLOWBASE_EXTRACT:destination},stdio:'inherit'})
    : spawnSync('tar',['-xzf',archive,'-C',destination],{stdio:'inherit'});
  if(result.status!==0)throw result.error || new Error('Native archive extraction failed');
  const root = path.join(destination,path.basename(stage));
  const manifest = JSON.parse(fs.readFileSync(path.join(root,'native-distribution.json'),'utf8'));
  for(const [name,digest] of Object.entries(manifest.files)) {
    if(!fs.lstatSync(path.join(root,name)).isFile())throw new Error(`Extracted package entry must be a regular file: ${name}`);
    if(crypto.createHash('sha256').update(fs.readFileSync(path.join(root,name))).digest('hex')!==digest)throw new Error(`Extracted file checksum mismatch: ${name}`);
  }
  if(fs.readFileSync(path.join(root,'.env'),'utf8')!==fs.readFileSync(path.join(root,'.env.example'),'utf8'))throw new Error('Archive must contain the initial configuration template without generated credentials');
  console.log(root);
  return root;
}
if(require.main===module)try{unpack(process.argv[2],process.argv[3]);}catch(error){console.error(error.message);process.exitCode=1;}
module.exports={unpack};
