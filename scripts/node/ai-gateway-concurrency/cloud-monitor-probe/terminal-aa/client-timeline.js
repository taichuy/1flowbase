'use strict';
class ClientTimeline {
  constructor({expectedDeltas=256,clock=()=>process.hrtime.bigint()}={}) {
    if(!Number.isSafeInteger(expectedDeltas)||expectedDeltas<1)throw Error('expected controlled fixture delta count');
    this.expected=expectedDeltas;this.clock=clock;this.reads=0;this.deltas=0;this.completed=0;this.fields={};this.ordinals={};this.ordinal=0;this.issues=[];
    this.mark('request_start_ns');
  }
  mark(field) {
    if(this.fields[field]!==undefined){this.issues.push('duplicate_'+field);return;}
    const value=this.clock();this.reads++;
    if(!/^[0-9]+$/.test(String(value)))throw Error('invalid monotonic clock');
    this.fields[field]=String(value);this.ordinals[field]=++this.ordinal;
  }
  headers(){this.mark('headers_ns');}
  delta(){this.deltas++;if(this.deltas===1)this.mark('first_delta_ns');if(this.deltas===this.expected)this.mark('last_expected_delta_ns');}
  terminal(){this.completed++;if(this.completed===1)this.mark('protocol_completed_ns');}
  end(){this.mark('body_end_ns');}
  snapshot() {
    const fields=['request_start_ns','headers_ns','first_delta_ns','last_expected_delta_ns','protocol_completed_ns','body_end_ns'];
    const issues=[...this.issues];if(this.deltas!==this.expected)issues.push('delta_count');if(this.completed!==1)issues.push('terminal_count');
    for(const field of fields)if(this.fields[field]===undefined)issues.push('missing_'+field);
    if(fields.every(f=>this.fields[f]!==undefined)&&fields.some((f,i)=>i>0&&BigInt(this.fields[f])<BigInt(this.fields[fields[i-1]])))issues.push('monotonic_order');
    if(fields.every(f=>this.ordinals[f]!==undefined)&&fields.some((f,i)=>i>0&&this.ordinals[f]<=this.ordinals[fields[i-1]]))issues.push('event_order');
    const duration=(a,b)=>this.fields[a]!==undefined&&this.fields[b]!==undefined?Number(BigInt(this.fields[b])-BigInt(this.fields[a]))/1e6:null;
    return {...this.fields,mark_ordinals:{...this.ordinals},valid:issues.length===0,issues,delta_events:this.deltas,terminal_events:this.completed,clock_reads:this.reads,
      first_to_last_delta_ms:duration('first_delta_ns','last_expected_delta_ns'),last_delta_to_terminal_ms:duration('last_expected_delta_ns','protocol_completed_ns'),terminal_to_body_end_ms:duration('protocol_completed_ns','body_end_ns'),request_to_body_end_ms:duration('request_start_ns','body_end_ns'),
      boundary:'client parse/callback monotonic timestamps; body_end is HTTP end callback, not server inner EOF/archive completion/socket flush'};
  }
}
module.exports={ClientTimeline};
