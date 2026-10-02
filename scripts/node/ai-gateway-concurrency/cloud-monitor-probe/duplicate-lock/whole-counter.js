'use strict';
const {sqlDelta}=require('../observation');
const {sqlGuard}=require('../lock-scope/scope-contract');
function wholeCounterEvidence(before,after,rawDatabaseDelta){
 if(!before||!after)return {guard:{valid:false,reason:'missing whole-window snapshots'},raw_database_delta:rawDatabaseDelta,validated_database_delta:null};
 const delta=sqlDelta(before.rows,after.rows),guard=sqlGuard(before.info,after.info,delta);
 return {guard,statement_delta:delta,raw_database_delta:rawDatabaseDelta,validated_database_delta:guard.valid?rawDatabaseDelta:null,scope:'before whole CPU boundary to after whole CPU boundary; includes warmup/background/observers; no flush receipt'};
}
module.exports={wholeCounterEvidence};
