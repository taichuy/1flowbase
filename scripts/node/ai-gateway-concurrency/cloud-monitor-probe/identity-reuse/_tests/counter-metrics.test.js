'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const {diagnosticMetrics} = require('../../observer-diagnostic/contract');
function sample(ns, cpu, available=true) {
 return {monotonic_ns:ns,processes:[{identity:'20:100',pid:20,group:'postgres',cpu_ticks:cpu,cpu_user_ticks:cpu,cpu_system_ticks:0,rss_bytes:8192,pss_bytes:available?4096:null,io:available?{rchar:0,wchar:0,syscr:0,syscw:0,read_bytes:0,write_bytes:0,cancelled_write_bytes:0}:null}]};
}
test('ordinary unreadable PG PSS and IO stay null while allowed CPU remains measurable', () => {
 const g=diagnosticMetrics([sample(0,10,false),sample(1e9,30,false)],100).groups.postgres;
 assert.equal(g.cpu_seconds,.2);assert.equal(g.cpu_average_single_core_percent,20);
 assert.equal(g.identity_complete,true);assert.equal(g.pss_peak_bytes,null);
 assert.ok(Object.values(g.io_delta).every(v=>v===null));
});
test('whole-window missing identity stays null with explicit lower-bound scope', () => {
 const g=diagnosticMetrics([sample(0,10),{monotonic_ns:1e9,processes:[]}],100).groups.postgres;
 assert.equal(g.cpu_seconds,null);assert.equal(g.cpu_average_single_core_percent,null);
 assert.equal(g.identity_complete,false);assert.equal(g.captured_cpu_seconds_lower_bound,0);
 assert.equal(g.unobserved_short_lived_processes_possible,true);
});
test('different PID generation cannot be subtracted or averaged as a complete identity', () => {
 const b=sample(1e9,1);b.processes[0].identity='20:200';
 const g=diagnosticMetrics([sample(0,90),b],100).groups.postgres;
 assert.equal(g.cpu_seconds,null);assert.equal(g.cpu_average_single_core_percent,null);
 assert.equal(g.identity_complete,false);
});
test('reported peak retains the actual sample interval and single-core denominator', () => {
 const g=diagnosticMetrics([sample(0,10),sample(5e8,50)],100).groups.postgres;
 assert.equal(g.sampled_cpu_peak.single_core_percent,80);
 assert.equal(g.sampled_cpu_peak.interval_seconds,.5);
});
