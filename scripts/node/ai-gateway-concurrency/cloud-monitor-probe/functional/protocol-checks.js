'use strict';
const assert=require('node:assert/strict');
// Established responsesToolEvents(final=true) emits two marker chunks, then the sentinel.
const FINAL_TOOL_TEXT='chunk-1 marker-1chunk-2 marker-2 1flowbase gateway tool sentinel ok';
function assertUniqueProjection(events,{terminal='response.completed',text}={}){
 const ends=events.filter(e=>['response.completed','response.failed','response.cancelled'].includes(e.type));assert.equal(ends.length,1,'Exactly one terminal');assert.equal(ends[0].type,terminal,'Expected terminal');
 const calls=events.filter(e=>e.type==='response.output_item.done'&&['function_call','custom_tool_call'].includes(e.item?.type)).map(e=>e.item.call_id);assert.ok(calls.every(Boolean));assert.equal(new Set(calls).size,calls.length,'No duplicate completed tool call');
 const deltas=events.filter(e=>e.type==='response.output_text.delta').map(e=>e.delta).join('');
 const full=(ends[0].response?.output||[]).flatMap(i=>i.content||[]).filter(x=>x.type==='output_text').map(x=>x.text).join('');
 if(full||deltas)assert.equal(deltas,full,'No missing or duplicate text relative to terminal output');if(text!==undefined)assert.equal(deltas,text,'Exact expected text');
 return {terminal:terminal,text:deltas,tool_call_ids:calls,completed_tool_calls:calls.length,event_types:events.map(e=>e.type)};
}
module.exports={assertUniqueProjection,FINAL_TOOL_TEXT};
