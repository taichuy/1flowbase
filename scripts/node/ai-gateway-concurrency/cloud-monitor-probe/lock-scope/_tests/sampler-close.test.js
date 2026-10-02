'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),{spawn}=require('node:child_process'),readline=require('node:readline');const{closeOwnedSampler}=require('../sampler-close');
test('Failed child stderr and final JSONL persist before close rejects, after stdio drains',async()=>{
 const root=path.join(process.cwd(),'tmp/test-governance/observer-close-pure');fs.mkdirSync(root,{recursive:true});const file=path.join(root,'samples-'+process.pid+'.jsonl');const log=fs.createWriteStream(file);
 const child=spawn(process.execPath,['-e',`process.stdin.resume();process.stdin.once('end',()=>{process.stdout.write(JSON.stringify({terminal:'final',escaped:'line\\nquote"',value:null})+'\\n');process.stderr.write('synthetic failure');process.exitCode=1;});`],{stdio:['pipe','pipe','pipe']});const closed=new Promise(resolve=>child.once('close',resolve));let stderr='';child.stderr.on('data',c=>stderr+=c);readline.createInterface({input:child.stdout}).on('line',line=>log.write(line+'\n'));
 try{await assert.rejects(closeOwnedSampler(child,closed,log,()=>stderr),/sampler exit 1 synthetic failure/);assert.deepEqual(JSON.parse(fs.readFileSync(file,'utf8')),{terminal:'final',escaped:'line\nquote"',value:null});assert.equal(log.writableFinished,true);}finally{fs.rmSync(file,{force:true});}
});
