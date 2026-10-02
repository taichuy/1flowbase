'use strict';
async function closeOwnedSampler(child,closed,log,errorTail){
 let code;
 try{if(child.exitCode===null&&!child.stdin.destroyed)child.stdin.end(JSON.stringify({stop:true})+'\n');code=await closed;}
 finally{await new Promise((resolve,reject)=>log.end(error=>error?reject(error):resolve()));}
 if(code!==0)throw Error('sampler exit '+code+' '+errorTail());
}
module.exports={closeOwnedSampler};
